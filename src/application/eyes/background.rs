use std::sync::{Arc, mpsc};
use bytemuck::{Zeroable, Pod};
use anyhow::Result;
use thiserror::Error;
use vulkano::{sync, command_buffer, sampler, memory, descriptor_set, buffer};
use vulkano::buffer::{Buffer, Subbuffer, BufferUsage, BufferContents};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract};
use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
use vulkano::device::Queue;
use vulkano::image::view::ImageView;
use vulkano::memory::allocator::{AllocationCreateInfo, MemoryUsage};
use vulkano::pipeline::{Pipeline, GraphicsPipeline, PipelineBindPoint};
use vulkano::sampler::{Sampler, Filter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode};
use vulkano::sync::GpuFuture;

use crate::config;
use crate::math::{Vec4, Vec2, Mat3, Isometry3};
use crate::renderer::Renderer;
use crate::renderer::pipelines::PipelineNoLayoutError;
use crate::utils::{FenceCheck, IntoInfo};
use super::camera::Camera;
use super::pipeline::{BackgroundPipeline, Vertex, Pc};


#[allow(dead_code)]
#[repr(C)]
#[derive(Default, Copy, Clone, BufferContents)]
struct Intrinsics {
	rawproj: [Vec4; 2],
	focal: [Vec2; 2],
	coeffs: [Vec4; 2],
	scale: [Vec2; 2],
	center: [Vec2; 2],
}

pub struct Background {
	queue: Arc<Queue>,
	pipeline: Arc<GraphicsPipeline>,
	vertices: Subbuffer<[Vertex]>,
	// intrinsics: Arc<CpuAccessibleBuffer<Intrinsics>>,
	set: Arc<PersistentDescriptorSet>,
	extrinsics: (Mat3, Mat3),
	last_frame_pose: Isometry3,
	camera_loads: mpsc::Receiver<(PrimaryAutoCommandBuffer, Option<Isometry3>)>,
}

impl Background {
	pub fn new(camera: Box<dyn Camera>, raw_projection: (Vec4, Vec4), renderer: &mut Renderer) -> Result<Background> {
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
		let vertices = Buffer::from_iter(&renderer.memory_allocator,
		                                 BufferUsage::VERTEX_BUFFER.into_info(),
		                                 MemoryUsage::Upload.into_info(),
		                                 square.iter().copied())?; // TODO: remove copied (array_into_iter)
		
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
		
		let intrinsics = Buffer::from_data(&renderer.memory_allocator,
		                                   BufferUsage::UNIFORM_BUFFER.into_info(),
		                                   MemoryUsage::Upload.into_info(),
		                                   intrinsics)?;
		
		let view = ImageView::new_default(camera_image.clone())?;
		let sampler = Sampler::new(queue.device().clone(), SamplerCreateInfo {
			mag_filter: Filter::Linear,
			min_filter: Filter::Linear,
			mipmap_mode: SamplerMipmapMode::Nearest,
			address_mode: [SamplerAddressMode::ClampToEdge; 3],
			..SamplerCreateInfo::default()
		})?;
		
		let set = PersistentDescriptorSet::new(
			&renderer.descriptor_set_allocator,
			pipeline.layout().set_layouts().get(0).ok_or(PipelineNoLayoutError)?.clone(), [
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
		
		Ok(Background {
			queue,
			pipeline,
			vertices,
			// intrinsics,
			set,
			extrinsics: (left_extrinsics, right_extrinsics),
			last_frame_pose: Isometry3::identity(),
			camera_loads: camera_rx,
		})
	}
	
	pub fn load_camera(&mut self, camera_pos: Isometry3, mut future: Box<dyn GpuFuture>) -> Result<Box<dyn GpuFuture>> {
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
	
	pub fn render(&mut self, camera_pos: Isometry3, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) -> Result<()> {
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
		// 	debug::draw_point(point!(0.0, 0.0, 0.0), 32.0, Color::RED);
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + rotation * Vec3::x() / 20.0, 4.0, Color::D_RED);
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + rotation * Vec3::y() / 20.0, 4.0, Color::D_GREEN);
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + rotation * Vec3::z() / 20.0, 4.0, Color::D_BLUE);
		//
		// 	debug::draw_point(point!(0.0, 0.0, 0.0), 32.0, Color::RED);
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + Vec3::x() / 20.0, 4.0, Color::RED);
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + Vec3::y() / 20.0, 4.0, Color::GREEN);
		// 	debug::draw_line(point!(0.0, 0.0, 0.0), point!(0.0, 0.0, 0.0) + Vec3::z() / 20.0, 4.0, Color::BLUE);
		//
		// 	let flip_xz = vector!(-1.0, 1.0, -1.0);
		//
		// 	let mut left_cam: Point3 = config.camera.left.position.component_mul(&flip_xz).into();
		// 	let left_ex_inv = self.extrinsics.0.try_inverse().unwrap();
		// 	debug::draw_point(&left_cam, 32.0, Color::GREEN);
		// 	debug::draw_line(&left_cam, left_cam + config.camera.left.right.component_mul(&flip_xz) / 15.0, 2.0, Color::CYAN);
		// 	debug::draw_line(&left_cam, left_cam + config.camera.left.back.component_mul(&flip_xz) / 15.0, 2.0, Color::D_CYAN);
		//
		// 	debug::draw_line(&left_cam, &left_cam + left_ex_inv * Vec3::x() / 20.0, 4.0, Color::RED);
		// 	debug::draw_line(&left_cam, &left_cam + left_ex_inv * Vec3::y() / 20.0, 4.0, Color::GREEN);
		// 	debug::draw_line(&left_cam, &left_cam + left_ex_inv * Vec3::z() / 20.0, 4.0, Color::BLUE);
		//
		// 	let mut right_cam: Point3 = config.camera.right.position.component_mul(&flip_xz).into();
		// 	let right_ex_inv = self.extrinsics.1.try_inverse().unwrap();
		// 	debug::draw_point(&right_cam, 32.0, Color::BLUE);
		// 	debug::draw_line(&right_cam, right_cam + config.camera.right.right.component_mul(&flip_xz) / 15.0, 2.0, Color::MAGENTA);
		// 	debug::draw_line(&right_cam, right_cam + config.camera.right.back.component_mul(&flip_xz) / 15.0, 2.0, Color::D_MAGENTA);
		//
		// 	debug::draw_line(&right_cam, &right_cam + right_ex_inv * Vec3::x() / 20.0, 4.0, Color::RED);
		// 	debug::draw_line(&right_cam, &right_cam + right_ex_inv * Vec3::y() / 20.0, 4.0, Color::GREEN);
		// 	debug::draw_line(&right_cam, &right_cam + right_ex_inv * Vec3::z() / 20.0, 4.0, Color::BLUE);
		// }
		
		let constants = Pc {
			shift: [
				(rotation * self.extrinsics.0).to_homogeneous().into(),
				(rotation * self.extrinsics.1).to_homogeneous().into()
			],
		};
		
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


