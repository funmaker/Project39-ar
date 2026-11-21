use std::cell::Cell;
use std::sync::{Arc, mpsc};
use anyhow::Result;
use vulkano::buffer::{Buffer, Subbuffer, BufferUsage, BufferCreateInfo};
use vulkano::buffer::allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract, SecondaryAutoCommandBuffer};
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
use crate::debug;
use crate::math::{AMat4, Isometry3, Mat3, Rot3, Translation3, Vec2, Vec4};
use crate::renderer::{RenderContext, Renderer};
use crate::renderer::pipelines::PipelineNoLayoutError;
use crate::utils::{BufferEx, FenceCheck, IntoInfo};
use super::camera::Camera;
use super::pipeline::{BackgroundPipeline, Vertex, Params};


/// Radius of the dome the camera image is projected on (same as SteamVR's room view).
const DOME_RADIUS: f32 = 15.0;
/// Floor height in tracking space.
const FLOOR_HEIGHT: f32 = 0.0;

pub struct Background {
	pipeline: Arc<GraphicsPipeline>,
	vertices: Subbuffer<[Vertex]>,
	params: Subbuffer<Params>,
	set: Arc<DescriptorSet>,
	cameras: [CameraSpec; 2],
	eye_to_head: [AMat4; 2],
	raw_projection: [Vec4; 2],
	frame_size: Vec2,
	last_frame_pose: Isometry3,
	camera_loads: mpsc::Receiver<(Arc<SecondaryAutoCommandBuffer>, Option<Isometry3>)>,
	fence: FenceCheck,
}

impl Background {
	pub fn new(camera: Box<dyn Camera>, raw_projection: (Vec4, Vec4), eye_to_head: (AMat4, AMat4), renderer: &mut Renderer) -> Result<Background> {
		let config = config::get();
		let pipeline = renderer.pipelines.get::<BackgroundPipeline>()?;
		let queue = renderer.load_queue.clone();
		
		let mut upload_buffer = AutoCommandBufferBuilder::primary(renderer.command_buffer_allocator.clone(),
		                                                          renderer.load_queue.queue_family_index(),
		                                                          CommandBufferUsage::OneTimeSubmit)?;
		
		let (camera_image, camera_rx) = camera.start(renderer.load_queue.clone(), renderer.memory_allocator.clone(), renderer.command_buffer_allocator.clone())?;
		
		let square = [
			Vertex::new([-1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0,  1.0]),
		];
		
		let vertices = Buffer::upload_iter(&renderer.memory_allocator,
		                                   BufferUsage::VERTEX_BUFFER.into_info(),
		                                   square.into_iter(),
		                                   &mut upload_buffer)?;
		
		let params = Buffer::new_sized(renderer.memory_allocator.clone(),
		                               BufferCreateInfo {
			                               usage: BufferUsage::UNIFORM_BUFFER | BufferUsage::TRANSFER_DST,
			                               ..BufferCreateInfo::default()
		                               },
		                               MemoryTypeFilter::PREFER_DEVICE.into_info())?;
		
		let camera_view = ImageView::new_default(camera_image.clone())?;
		let sampler = Sampler::new(queue.device().clone(), SamplerCreateInfo {
			mag_filter: Filter::Linear,
			min_filter: Filter::Linear,
			mipmap_mode: SamplerMipmapMode::Nearest,
			address_mode: [SamplerAddressMode::ClampToEdge; 3],
			..SamplerCreateInfo::default()
		})?;
		
		let set = DescriptorSet::new(
			renderer.descriptor_set_allocator.clone(),
			pipeline.layout().set_layouts().get(0).ok_or(PipelineNoLayoutError)?.clone(),
			[
				WriteDescriptorSet::buffer(0, params.clone()),
				WriteDescriptorSet::image_view_sampler(1, camera_view.clone(), sampler.clone()),
			],
			[])?;
		
		let upload_future = upload_buffer.build()?
			.execute(renderer.load_queue.clone())?;
		
		let fence = FenceCheck::new(upload_future)?;
		
		Ok(Background {
			pipeline,
			vertices,
			params,
			set,
			cameras: [CameraSpec::new(&config.camera.left), CameraSpec::new(&config.camera.right)],
			eye_to_head: [eye_to_head.0.inverse(), eye_to_head.1.inverse()],
			raw_projection: [raw_projection.0, raw_projection.1],
			frame_size: config.camera.frame_buffer_size.cast(),
			last_frame_pose: Isometry3::identity(),
			camera_loads: camera_rx,
			fence,
		})
	}
	
	pub fn before_render(&mut self, hmd_position: Isometry3, context: &mut RenderContext) -> Result<()> {
		if !self.fence.check() { return Ok(()) }
		
		while let Ok((command, cam_pose)) = self.camera_loads.try_recv() {
			context.builder.execute_commands(command)?;
			self.last_frame_pose = cam_pose.unwrap_or(hmd_position);
		}
		
		context.builder.update_buffer(self.params.clone(), Arc::new(Params {
			eye_to_tracking: [
				(hmd_position.to_homogeneous() * self.eye_to_head[0].to_homogeneous()).into(),
				(hmd_position.to_homogeneous() * self.eye_to_head[1].to_homogeneous()).into(),
			],
			tracking_to_camera: [
				(self.last_frame_pose * self.cameras[0].to_head).inverse().to_homogeneous().into(),
				(self.last_frame_pose * self.cameras[1].to_head).inverse().to_homogeneous().into(),
			],
			rawproj: self.raw_projection.map(Into::into),
			coeffs: [
				self.cameras[0].coeffs.into(),
				self.cameras[1].coeffs.into(),
			],
			focal_center: [
				self.cameras[0].focal_center.into(),
				self.cameras[1].focal_center.into(),
			],
			bounds: [
				self.cameras[0].bounds.into(),
				self.cameras[1].bounds.into(),
			],
			tan_shift: [
				[0.0; 2].into(),
				if debug::get_flag_or_default("OpenVRPassthroughShift") { [0.006842548, 0.0].into() }
				else { [0.0, 0.0].into() },
			],
			dome: [hmd_position.translation.x, FLOOR_HEIGHT, hmd_position.translation.z, DOME_RADIUS],
			frame_size: self.frame_size.into(),
		}))?;
		
		Ok(())
	}
	
	/// `head_pose` - head pose in tracking space at render time.
	pub fn render(&mut self, context: &mut RenderContext) -> Result<()> {
		if !self.fence.check() { return Ok(()) }
		
		unsafe {
			context.builder.bind_pipeline_graphics(self.pipeline.clone())?
			               .bind_vertex_buffers(0, self.vertices.clone())?
			               .bind_descriptor_sets(PipelineBindPoint::Graphics,
			                                     self.pipeline.layout().clone(),
			                                     0,
			                                     self.set.clone())?
			               .draw(self.vertices.len() as u32,
			                     1,
			                     0,
			                     0)?;
		}
		
		Ok(())
	}
}

#[derive(Copy, Clone)]
struct CameraSpec {
	/// Camera to head transform.
	to_head: Isometry3,
	coeffs: Vec4,
	focal_center: Vec4,
	bounds: Vec4,
}

impl CameraSpec {
	fn new(config: &CameraEyeConfig) -> Self {
		let back = config.back.normalize();
		let right = config.right.normalize();
		let rotation = Mat3::from_columns(&[right, back.cross(&right), back]);
		let center = config.center + config.offset.cast();
		let min = config.offset.cast::<f32>() - vector!(0.5, 0.5);
		let max = min + config.size.cast::<f32>();
		
		CameraSpec {
			to_head: Isometry3::from_parts(Translation3::from(config.position), Rot3::from_matrix(&rotation)),
			coeffs: config.coeffs,
			focal_center: vector!(config.focal_length.x, config.focal_length.y, center.x, center.y),
			bounds: vector!(min.x, min.y, max.x, max.y),
		}
	}
}


