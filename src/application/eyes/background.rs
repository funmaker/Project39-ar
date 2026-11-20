use std::cell::Cell;
use std::sync::{Arc, mpsc};
use anyhow::Result;
use vulkano::buffer::{Buffer, Subbuffer, BufferUsage};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::device::Queue;
use vulkano::image::Image;
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::MemoryTypeFilter;
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};
use vulkano::image::sampler::{Sampler, Filter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode};
use vulkano::sync::GpuFuture;

use crate::config::{self, CameraEyeConfig};
use crate::math::{AMat4, Isometry3, Mat3, Mat4, Rot3, Translation3, Vec4, VRSlice};
use crate::renderer::Renderer;
use crate::renderer::pipelines::PipelineNoLayoutError;
use crate::utils::IntoInfo;
use super::super::vr::{VR, DepthMesh, DepthMeshFrame};
use super::camera::Camera;
use super::pipeline::{BackgroundPipeline, BackgroundMeshPipeline, Vertex, MeshVertex, Params};
use super::dump::{BackgroundDump, ParamsDump};


/// Radius of the dome the camera image is projected on (same as SteamVR's room view).
const DOME_RADIUS: f32 = 15.0;
/// Floor height in tracking space.
const FLOOR_HEIGHT: f32 = 0.0;

/// Static per camera data.
#[derive(Copy, Clone)]
struct CameraData {
	/// Camera to head transform.
	to_head: Isometry3,
	coeffs: Vec4,
	focal_center: Vec4,
	bounds: Vec4,
}

impl CameraData {
	fn new(config: &CameraEyeConfig) -> Self {
		let back = config.back.normalize();
		let right = config.right.normalize();
		let rotation = Mat3::from_columns(&[right, back.cross(&right), back]);
		let center = config.center + config.offset.cast();
		let min = config.offset.cast::<f32>() - vector!(0.5, 0.5);
		let max = min + config.size.cast::<f32>();

		CameraData {
			to_head: Isometry3::from_parts(Translation3::from(config.position), Rot3::from_matrix(&rotation)),
			coeffs: config.coeffs,
			focal_center: vector!(config.focal_length.x, config.focal_length.y, center.x, center.y),
			bounds: vector!(min.x, min.y, max.x, max.y),
		}
	}
}

/// Values used by the last background render.
#[derive(Copy, Clone)]
struct LastRender {
	head_pose: Isometry3,
	params: Params,
}

pub struct Background {
	queue: Arc<Queue>,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Subbuffer<[Vertex]>,
	set_layout: Arc<DescriptorSetLayout>,
	mesh_pipeline: Arc<GraphicsPipeline>,
	mesh_set_layout: Arc<DescriptorSetLayout>,
	mesh_allocator: SubbufferAllocator,
	/// Latest depth mesh (vertices, indices).
	mesh: Option<(Subbuffer<[MeshVertex]>, Subbuffer<[u32]>)>,
	/// SteamVR's depth mesh reader, created on demand. Has to be dropped before `vr`.
	depth_mesh: Option<DepthMesh>,
	depth_passthrough: Cell<bool>,
	vr: Option<Arc<VR>>,
	descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
	params_allocator: SubbufferAllocator,
	camera_view: Arc<ImageView>,
	sampler: Arc<Sampler>,
	camera_image: Arc<Image>,
	cameras: [CameraData; 2],
	/// Camera space x/z shift reproducing SteamVR's right camera undistortion map lookup (see `Eyes::steamvr_quirk_shift`).
	quirk_shift: [f32; 2],
	steamvr_quirks: Cell<bool>,
	/// Eye to head transforms.
	eyes: [AMat4; 2],
	raw_projection: [Vec4; 2],
	frame_size: Vec4,
	last_frame_pose: Isometry3,
	last_render: Option<LastRender>,
	camera_loads: mpsc::Receiver<(Arc<PrimaryAutoCommandBuffer>, Option<Isometry3>)>,
}

impl Background {
	/// `view` - head to eye transforms. `quirk_shift` - see `steamvr_quirks`.
	pub fn new(camera: Box<dyn Camera>, raw_projection: (Vec4, Vec4), view: (AMat4, AMat4), quirk_shift: [f32; 2], vr: Option<Arc<VR>>, renderer: &mut Renderer) -> Result<Background> {
		let config = config::get();
		let pipeline = renderer.pipelines.get::<BackgroundPipeline>()?;
		let mesh_pipeline = renderer.pipelines.get::<BackgroundMeshPipeline>()?;
		let queue = renderer.load_queue.clone();

		let (camera_image, camera_rx) = camera.start(renderer.load_queue.clone(), renderer.memory_allocator.clone(), renderer.command_buffer_allocator.clone())?;

		let square = [
			Vertex::new([-1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0,  1.0]),
		];

		// TODO: Upload
		let vertices = Buffer::from_iter(renderer.memory_allocator.clone(),
		                                 BufferUsage::VERTEX_BUFFER.into_info(),
		                                 (MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE).into_info(),
		                                 square.iter().copied())?; // TODO: remove copied (array_into_iter)

		let params_allocator = SubbufferAllocator::new(renderer.memory_allocator.clone(), SubbufferAllocatorCreateInfo {
			buffer_usage: BufferUsage::UNIFORM_BUFFER,
			memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
			..SubbufferAllocatorCreateInfo::default()
		});

		let camera_view = ImageView::new_default(camera_image.clone())?;
		let sampler = Sampler::new(queue.device().clone(), SamplerCreateInfo {
			mag_filter: Filter::Linear,
			min_filter: Filter::Linear,
			mipmap_mode: SamplerMipmapMode::Nearest,
			address_mode: [SamplerAddressMode::ClampToEdge; 3],
			..SamplerCreateInfo::default()
		})?;

		let set_layout = pipeline.layout().set_layouts().get(0).ok_or(PipelineNoLayoutError)?.clone();
		let mesh_set_layout = mesh_pipeline.layout().set_layouts().get(0).ok_or(PipelineNoLayoutError)?.clone();

		let mesh_allocator = SubbufferAllocator::new(renderer.memory_allocator.clone(), SubbufferAllocatorCreateInfo {
			buffer_usage: BufferUsage::VERTEX_BUFFER | BufferUsage::INDEX_BUFFER,
			memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
			..SubbufferAllocatorCreateInfo::default()
		});
		let frame_size = config.camera.frame_buffer_size.cast::<f32>();

		Ok(Background {
			queue,
			pipeline,
			vertices,
			set_layout,
			mesh_pipeline,
			mesh_set_layout,
			mesh_allocator,
			mesh: None,
			depth_mesh: None,
			depth_passthrough: Cell::new(false),
			vr,
			descriptor_set_allocator: renderer.descriptor_set_allocator.clone(),
			params_allocator,
			camera_view,
			sampler,
			camera_image,
			cameras: [CameraData::new(&config.camera.left), CameraData::new(&config.camera.right)],
			quirk_shift,
			steamvr_quirks: Cell::new(false),
			eyes: [view.0.inverse(), view.1.inverse()],
			raw_projection: [raw_projection.0, raw_projection.1],
			frame_size: vector!(frame_size.x, frame_size.y, 0.0, 0.0),
			last_frame_pose: Isometry3::identity(),
			last_render: None,
			camera_loads: camera_rx,
		})
	}

	pub fn camera_image(&self) -> &Arc<Image> {
		&self.camera_image
	}

	/// Emulate SteamVR's passthrough bugs (right camera is shifted by half of its undistortion map texel, ~0.4 deg).
	pub fn steamvr_quirks(&self) -> &Cell<bool> {
		&self.steamvr_quirks
	}

	/// 3D passthrough using the depth mesh of SteamVR's 3D room view (VR only). The dome is still used where the mesh has no data.
	pub fn depth_passthrough(&self) -> &Cell<bool> {
		&self.depth_passthrough
	}

	/// Turns the depth mesh processing on/off according to `depth_passthrough` and uploads new meshes.
	fn update_depth_mesh(&mut self) -> Result<()> {
		let enabled = self.depth_passthrough.get() && self.vr.is_some();

		if !enabled {
			if self.depth_mesh.take().is_some() {
				self.mesh = None;
			}
			return Ok(());
		}

		if self.depth_mesh.is_none() {
			let mut depth_mesh = DepthMesh::new()?;
			depth_mesh.set_enabled(true)?;
			self.depth_mesh = Some(depth_mesh);
		}

		if let Some(frame) = self.depth_mesh.as_mut().unwrap().poll()? {
			self.upload_mesh(&frame)?;
		}

		Ok(())
	}

	fn upload_mesh(&mut self, frame: &DepthMeshFrame) -> Result<()> {
		let (width, height) = (frame.width as usize, frame.height as usize);
		// The Index only has a mesh for the left camera, SteamVR uses it for both eyes.
		let points = &frame.left;
		if width < 2 || height < 2 || points.len() < width * height {
			return Ok(());
		}

		let valid = |index: usize| points[index].iter().all(|v| v.is_finite());
		let mut indices = Vec::with_capacity((width - 1) * (height - 1) * 6);
		for row in 0..height - 1 {
			for col in 0..width - 1 {
				let i = row * width + col;
				// Same triangulation as vrcompositor, triangles touching invalid (NaN) points are skipped.
				for tri in [[i, i + 1, i + width + 1], [i, i + width + 1, i + width]] {
					if tri.iter().all(|&v| valid(v)) {
						indices.extend(tri.map(|v| v as u32));
					}
				}
			}
		}

		if indices.is_empty() {
			self.mesh = None;
			return Ok(());
		}

		let vertices = self.mesh_allocator.allocate_slice::<MeshVertex>(points.len() as u64)?;
		{
			let mut write = vertices.write()?;
			for (dst, src) in write.iter_mut().zip(points) {
				*dst = MeshVertex::new(src.map(|v| if v.is_finite() { v } else { 0.0 }));
			}
		}

		let index_buffer = self.mesh_allocator.allocate_slice::<u32>(indices.len() as u64)?;
		index_buffer.write()?.copy_from_slice(&indices);

		self.mesh = Some((vertices, index_buffer));

		Ok(())
	}

	fn raw_to_tracking(&self) -> Mat4 {
		match &self.vr {
			Some(vr) => Mat4::from_slice34(&vr.lock().unwrap().system.raw_zero_pose_to_standing_absolute_tracking_pose()),
			None => Mat4::identity(),
		}
	}

	pub fn dump(&self) -> BackgroundDump {
		BackgroundDump {
			camera_frame_pose: self.last_frame_pose.into(),
			render_head_pose: self.last_render.map(|last| last.head_pose.into()),
			params: self.last_render.map(|last| ParamsDump {
				eye_to_tracking: last.params.eye_to_tracking,
				tracking_to_camera: last.params.tracking_to_camera,
				rawproj: last.params.rawproj,
				coeffs: last.params.coeffs,
				focal_center: last.params.focal_center,
				bounds: last.params.bounds,
				tan_shift: last.params.tan_shift,
				raw_to_tracking: last.params.raw_to_tracking,
dome: last.params.dome,
				frame_size: last.params.frame_size,
			}),
		}
	}

	/// `head_pose` - current head pose in tracking space, used when the camera doesn't provide frame poses.
	pub fn load_camera(&mut self, head_pose: Isometry3, mut future: Box<dyn GpuFuture>) -> Result<Box<dyn GpuFuture>> {
		while let Ok((command, cam_pose)) = self.camera_loads.try_recv() {
			if !future.queue_change_allowed() && &future.queue().unwrap() != &self.queue {
				future = Box::new(future.then_signal_semaphore()
				                        .then_execute(self.queue.clone(), command)?);
			} else {
				future = Box::new(future.then_execute(self.queue.clone(), command)?);
			}
			
			self.last_frame_pose = cam_pose.unwrap_or(head_pose);
		}
		
		Ok(future)
	}
	
	/// `head_pose` - head pose in tracking space at render time.
	pub fn render(&mut self, head_pose: Isometry3, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<()> {
		if let Err(err) = self.update_depth_mesh() {
			eprintln!("3D passthrough unavailable: {err}");
			self.depth_passthrough.set(false);
			self.depth_mesh = None;
			self.mesh = None;
		}
		
let eye_to_tracking = |eye: usize| -> [[f32; 4]; 4] {
			(head_pose.to_homogeneous() * self.eyes[eye].to_homogeneous()).into()
		};
		let tracking_to_camera = |eye: usize| -> [[f32; 4]; 4] {
			(self.last_frame_pose * self.cameras[eye].to_head).inverse().to_homogeneous().into()
		};
		let tracking_to_eye = |eye: usize| -> [[f32; 4]; 4] {
			(self.eyes[eye].inverse().to_homogeneous() * head_pose.inverse().to_homogeneous()).into()
		};

		let params = Params {
			eye_to_tracking: [eye_to_tracking(0), eye_to_tracking(1)],
			tracking_to_camera: [tracking_to_camera(0), tracking_to_camera(1)],
			tracking_to_eye: [tracking_to_eye(0), tracking_to_eye(1)],
			raw_to_tracking: self.raw_to_tracking().into(),
rawproj: self.raw_projection.map(Into::into),
			coeffs: self.cameras.map(|camera| camera.coeffs.into()),
			focal_center: self.cameras.map(|camera| camera.focal_center.into()),
			bounds: self.cameras.map(|camera| camera.bounds.into()),
			tan_shift: if self.steamvr_quirks.get() { self.quirk_shift.map(|shift| [shift, 0.0, 0.0, 0.0]) } else { [[0.0; 4]; 2] },
			dome: [head_pose.translation.x, FLOOR_HEIGHT, head_pose.translation.z, DOME_RADIUS],
			frame_size: self.frame_size.into(),
		};

		self.last_render = Some(LastRender { head_pose, params });

		let params_buffer = self.params_allocator.allocate_sized::<Params>()?;
		*params_buffer.write()? = params;

		let set = DescriptorSet::new(
			self.descriptor_set_allocator.clone(),
			self.set_layout.clone(),
			[
				WriteDescriptorSet::buffer(0, params_buffer.clone()),
				WriteDescriptorSet::image_view_sampler(1, self.camera_view.clone(), self.sampler.clone()),
			],
			[])?;

		let mesh_set = self.mesh.is_some().then(|| DescriptorSet::new(
			self.descriptor_set_allocator.clone(),
			self.mesh_set_layout.clone(),
			[
				WriteDescriptorSet::buffer(0, params_buffer),
				WriteDescriptorSet::image_view_sampler(1, self.camera_view.clone(), self.sampler.clone()),
			],
			[])).transpose()?;

		unsafe {
			builder.bind_pipeline_graphics(self.pipeline.clone())?
			       .bind_vertex_buffers(0, self.vertices.clone())?
			       .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                             self.pipeline.layout().clone(),
			                             0,
			                             set)?
			       .draw(self.vertices.len() as u32,
			             1,
			             0,
			             0)?;
		}
		
		if let (Some((vertices, indices)), Some(mesh_set)) = (&self.mesh, mesh_set) {
			unsafe {
				builder.bind_pipeline_graphics(self.mesh_pipeline.clone())?
				       .bind_vertex_buffers(0, vertices.clone())?
				       .bind_index_buffer(indices.clone())?
				       .bind_descriptor_sets(PipelineBindPoint::Graphics,
				                             self.mesh_pipeline.layout().clone(),
				                             0,
				                             mesh_set)?
				       .draw_indexed(indices.len() as u32, 1, 0, 0, 0)?;
			}
		}
		
		Ok(())
	}
}


