use std::sync::{Arc, mpsc};
use err_derive::Error;
use bytemuck::{Zeroable, Pod};
use vulkano::{sync, command_buffer, sampler, memory, descriptor_set};
use vulkano::device::Queue;
use vulkano::buffer::{ImmutableBuffer, BufferUsage, CpuAccessibleBuffer, TypedBufferAccess};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::image::view::ImageView;
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer};
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};
use vulkano::sync::GpuFuture;

use crate::math::{Vec4, Vec2, Mat3, Isometry3};
use crate::utils::{FenceCheck, NgPod};
use crate::config;
use crate::application::eyes::camera::CameraStartError;
use crate::application::eyes::camera::Camera;
use crate::renderer::Renderer;
use crate::renderer::pipelines::PipelineError;
use super::pipeline::{BackgroundPipeline, Vertex};

#[allow(dead_code)]
#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
struct Intrinsics {
	rawproj: [NgPod<Vec4>; 2],
	focal: [NgPod<Vec2>; 2],
	coeffs: [NgPod<Vec4>; 2],
	scale: [NgPod<Vec2>; 2],
	center: [NgPod<Vec2>; 2],
}

pub struct Background {
	queue: Arc<Queue>,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	// intrinsics: Arc<CpuAccessibleBuffer<Intrinsics>>,
	set: Arc<PersistentDescriptorSet>,
	fence: FenceCheck,
	extrinsics: (Mat3, Mat3),
	last_frame_pose: Isometry3,
	camera_loads: mpsc::Receiver<(PrimaryAutoCommandBuffer, Option<Isometry3>)>,
}

impl Background {
	pub fn new(camera: Box<dyn Camera>, raw_projection: (Vec4, Vec4), renderer: &mut Renderer) -> Result<Background, BackgroundError> {
		let config = config::get();
		let pipeline = renderer.pipelines.get::<BackgroundPipeline>()?;
		let queue = renderer.load_queue.clone();
		
		let (camera_image, camera_rx) = camera.start(queue.clone())?;
		
		let square = [
			Vertex::new([-1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([ 1.0, -1.0]),
			Vertex::new([-1.0,  1.0]),
			Vertex::new([ 1.0,  1.0]),
		];
		
		let (vertices, vertices_promise) = ImmutableBuffer::from_iter(square.iter().cloned(),
		                                                              BufferUsage{ vertex_buffer: true, ..BufferUsage::none() },
		                                                              queue.clone())?;
		
		let intrinsics = Intrinsics {
			rawproj: [
				raw_projection.0.into(),
				raw_projection.1.into(),
			],
			focal: [
				config.camera.left.focal_length.component_div(&config.camera.left.size.cast()).into(),
				config.camera.right.focal_length.component_div(&config.camera.right.size.cast()).into(),
			],
			coeffs: [
				config.camera.left.coeffs.clone().into(),
				config.camera.right.coeffs.clone().into(),
			],
			scale: [
				config.camera.left.size.cast::<f32>().component_div(&config.camera.frame_buffer_size.cast()).into(),
				config.camera.right.size.cast::<f32>().component_div(&config.camera.frame_buffer_size.cast()).into(),
			],
			center: [
				(config.camera.left.center + config.camera.left.offset.cast()).component_div(&config.camera.frame_buffer_size.cast()).into(),
				(config.camera.right.center + config.camera.right.offset.cast()).component_div(&config.camera.frame_buffer_size.cast()).into(),
			],
		};
		
		let intrinsics = CpuAccessibleBuffer::from_data(queue.device().clone(),
		                                                BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
		                                                true,
		                                                intrinsics)?;
		
		let view = ImageView::new_default(camera_image.clone())?;
		let sampler = Sampler::new(queue.device().clone(), SamplerCreateInfo {
			mag_filter: Filter::Linear,
			min_filter: Filter::Linear,
			mipmap_mode: SamplerMipmapMode::Nearest,
			address_mode: [SamplerAddressMode::ClampToEdge; 3],
			..SamplerCreateInfo::default()
		})?;
		
		let set = PersistentDescriptorSet::new(pipeline.layout().set_layouts().get(0).ok_or(BackgroundError::NoLayout)?.clone(), [
			WriteDescriptorSet::buffer(0, intrinsics.clone()),
			WriteDescriptorSet::image_view_sampler(1, view, sampler),
		])?;
		
		let flip_xz = vector!(-1.0, 1.0, -1.0);
		let flip_xz_m = Mat3::from_columns(&[flip_xz, flip_xz, flip_xz]);
		
		let left_extrinsics = Mat3::from_columns(&[
			config.camera.left.right,
			config.camera.left.back.cross(&config.camera.left.right),
			config.camera.left.back,
		]).component_mul(&flip_xz_m)
		  .try_inverse()
		  .expect("Unable to inverse left camera extrinsics");
		
		let right_extrinsics = Mat3::from_columns(&[
			config.camera.right.right,
			config.camera.right.back.cross(&config.camera.right.right),
			config.camera.right.back,
		]).component_mul(&flip_xz_m)
		  .try_inverse()
		  .expect("Unable to inverse right camera extrinsics");
		
		let fence = FenceCheck::new(vertices_promise)?;
		
		Ok(Background {
			queue,
			pipeline,
			vertices,
			// intrinsics,
			set,
			fence,
			extrinsics: (left_extrinsics, right_extrinsics),
			last_frame_pose: Isometry3::identity(),
			camera_loads: camera_rx,
		})
	}
	
	pub fn load_camera(&mut self, camera_pos: Isometry3, mut future: Box<dyn GpuFuture>) -> Result<Box<dyn GpuFuture>, BackgroundLoadError> {
		while let Ok((command, cam_pose)) = self.camera_loads.try_recv() {
			if !future.queue_change_allowed() && &future.queue().unwrap() != &self.queue {
				future = Box::new(future.then_signal_semaphore()
				                        .then_execute(self.queue.clone(), command)?);
			} else {
				future = Box::new(future.then_execute(self.queue.clone(), command)?);
			}
			
			self.last_frame_pose = cam_pose.unwrap_or(camera_pos);
		}
		
		Ok(future)
	}
	
	pub fn render(&mut self, camera_pos: Isometry3, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<(), BackgroundRenderError> {
		if !self.fence.check() { return Ok(()); }
		
		// if let Ok(mut intrinsics) = self.intrinsics.write() {
		// 	if debug::get_flag_or_default("KeyA") {
		// 		intrinsics.center[0].x -= 0.0001;
		// 	} else if debug::get_flag_or_default("KeyD") {
		// 		intrinsics.center[0].x += 0.0001;
		// 	}
		//
		// 	if debug::get_flag_or_default("KeyW") {
		// 		intrinsics.center[0].y -= 0.0001;
		// 	} else if debug::get_flag_or_default("KeyS") {
		// 		intrinsics.center[0].y += 0.0001;
		// 	}
		//
		// 	if debug::get_flag_or_default("KeyH") {
		// 		intrinsics.center[1].x -= 0.0001;
		// 	} else if debug::get_flag_or_default("KeyK") {
		// 		intrinsics.center[1].x += 0.0001;
		// 	}
		//
		// 	if debug::get_flag_or_default("KeyU") {
		// 		intrinsics.center[1].y -= 0.0001;
		// 	} else if debug::get_flag_or_default("KeyJ") {
		// 		intrinsics.center[1].y += 0.0001;
		// 	}
		// }
		
		let rotation = (camera_pos.rotation.inverse() * camera_pos.rotation / self.last_frame_pose.rotation * camera_pos.rotation).to_rotation_matrix();
		
		// {
		// 	let config = config::get();
		//
		// 	debug::draw_point(point!(0.0, 0.0, 0.0), 32.0, Color::red());
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + rotation * Vec3::x() / 20.0, 4.0, Color::dred());
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + rotation * Vec3::y() / 20.0, 4.0, Color::dgreen());
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + rotation * Vec3::z() / 20.0, 4.0, Color::dblue());
		//
		// 	debug::draw_point(point!(0.0, 0.0, 0.0), 32.0, Color::red());
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + Vec3::x() / 20.0, 4.0, Color::red());
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + Vec3::y() / 20.0, 4.0, Color::green());
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + Vec3::z() / 20.0, 4.0, Color::blue());
		//
		// 	let flip_xz = vector!(-1.0, 1.0, -1.0);
		//
		// 	let mut left_cam: Point3 = config.camera.left.position.component_mul(&flip_xz).into();
		// 	let left_ex_inv = self.extrinsics.0.try_inverse().unwrap();
		// 	debug::draw_point(&left_cam, 32.0, Color::green());
		// 	debug::draw_line(&left_cam, left_cam + config.camera.left.right.component_mul(&flip_xz) / 15.0, 2.0, Color::cyan());
		// 	debug::draw_line(&left_cam, left_cam + config.camera.left.back.component_mul(&flip_xz) / 15.0, 2.0, Color::dcyan());
		//
		// 	debug::draw_line(&left_cam, &left_cam + left_ex_inv * Vec3::x() / 20.0, 4.0, Color::red());
		// 	debug::draw_line(&left_cam, &left_cam + left_ex_inv * Vec3::y() / 20.0, 4.0, Color::green());
		// 	debug::draw_line(&left_cam, &left_cam + left_ex_inv * Vec3::z() / 20.0, 4.0, Color::blue());
		//
		// 	let mut right_cam: Point3 = config.camera.right.position.component_mul(&flip_xz).into();
		// 	let right_ex_inv = self.extrinsics.1.try_inverse().unwrap();
		// 	debug::draw_point(&right_cam, 32.0, Color::blue());
		// 	debug::draw_line(&right_cam, right_cam + config.camera.right.right.component_mul(&flip_xz) / 15.0, 2.0, Color::magenta());
		// 	debug::draw_line(&right_cam, right_cam + config.camera.right.back.component_mul(&flip_xz) / 15.0, 2.0, Color::dmagenta());
		//
		// 	debug::draw_line(&right_cam, &right_cam + right_ex_inv * Vec3::x() / 20.0, 4.0, Color::red());
		// 	debug::draw_line(&right_cam, &right_cam + right_ex_inv * Vec3::y() / 20.0, 4.0, Color::green());
		// 	debug::draw_line(&right_cam, &right_cam + right_ex_inv * Vec3::z() / 20.0, 4.0, Color::blue());
		// }
		
		let constants = (
			(rotation * self.extrinsics.0).to_homogeneous(),
			(rotation * self.extrinsics.1).to_homogeneous(),
		);
		
		builder.bind_pipeline_graphics(self.pipeline.clone())
		       .bind_vertex_buffers(0, self.vertices.clone())
		       .bind_descriptor_sets(PipelineBindPoint::Graphics,
		                             self.pipeline.layout().clone(),
		                             0,
		                             self.set.clone())
		       .push_constants(self.pipeline.layout().clone(),
		                       0,
		                       constants)
		       .draw(self.vertices.len() as u32,
		             1,
		             0,
		             0)?;
		
		Ok(())
	}
}



#[derive(Debug, Error)]
pub enum BackgroundError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] CameraStartError(#[error(source)] CameraStartError),
	#[error(display = "{}", _0)] DeviceMemoryAllocationError(#[error(source)] memory::DeviceMemoryAllocationError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] DescriptorSetCreationError(#[error(source)] descriptor_set::DescriptorSetCreationError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
}

#[derive(Debug, Error)]
pub enum BackgroundRenderError {
	#[error(display = "{}", _0)] DrawError(#[error(source)] command_buffer::DrawError),
}

#[derive(Debug, Error)]
pub enum BackgroundLoadError {
	#[error(display = "{}", _0)] DrawError(#[error(source)] command_buffer::CommandBufferExecError),
}
