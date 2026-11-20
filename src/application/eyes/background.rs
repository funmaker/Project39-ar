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
use crate::math::{AMat4, Isometry3, Mat3, Rot3, Translation3, Vec4};
use crate::renderer::Renderer;
use crate::renderer::pipelines::PipelineNoLayoutError;
use crate::utils::IntoInfo;
use super::camera::Camera;
use super::pipeline::{BackgroundPipeline, Vertex, Params};
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
		let center = config.center + config.center_offset + config.offset.cast();
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
	descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
	params_allocator: SubbufferAllocator,
	camera_view: Arc<ImageView>,
	sampler: Arc<Sampler>,
	camera_image: Arc<Image>,
	cameras: [CameraData; 2],
	/// Eye to head transforms.
	eyes: [AMat4; 2],
	raw_projection: [Vec4; 2],
	frame_size: Vec4,
	last_frame_pose: Isometry3,
	last_render: Option<LastRender>,
	camera_loads: mpsc::Receiver<(Arc<PrimaryAutoCommandBuffer>, Option<Isometry3>)>,
}

impl Background {
	/// `view` - head to eye transforms.
	pub fn new(camera: Box<dyn Camera>, raw_projection: (Vec4, Vec4), view: (AMat4, AMat4), renderer: &mut Renderer) -> Result<Background> {
		let config = config::get();
		let pipeline = renderer.pipelines.get::<BackgroundPipeline>()?;
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
		let frame_size = config.camera.frame_buffer_size.cast::<f32>();

		Ok(Background {
			queue,
			pipeline,
			vertices,
			set_layout,
			descriptor_set_allocator: renderer.descriptor_set_allocator.clone(),
			params_allocator,
			camera_view,
			sampler,
			camera_image,
			cameras: [CameraData::new(&config.camera.left), CameraData::new(&config.camera.right)],
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
		let eye_to_tracking = |eye: usize| -> [[f32; 4]; 4] {
			(head_pose.to_homogeneous() * self.eyes[eye].to_homogeneous()).into()
		};
		let tracking_to_camera = |eye: usize| -> [[f32; 4]; 4] {
			(self.last_frame_pose * self.cameras[eye].to_head).inverse().to_homogeneous().into()
		};

		let params = Params {
			eye_to_tracking: [eye_to_tracking(0), eye_to_tracking(1)],
			tracking_to_camera: [tracking_to_camera(0), tracking_to_camera(1)],
			rawproj: self.raw_projection.map(Into::into),
			coeffs: self.cameras.map(|camera| camera.coeffs.into()),
			focal_center: self.cameras.map(|camera| camera.focal_center.into()),
			bounds: self.cameras.map(|camera| camera.bounds.into()),
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
				WriteDescriptorSet::buffer(0, params_buffer),
				WriteDescriptorSet::image_view_sampler(1, self.camera_view.clone(), self.sampler.clone()),
			],
			[])?;

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
		
		Ok(())
	}
}


