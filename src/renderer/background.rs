use std::sync::Arc;
use err_derive::Error;
use vulkano::image::AttachmentImage;
use vulkano::device::Queue;
use vulkano::buffer::{ImmutableBuffer, BufferUsage, CpuAccessibleBuffer};
use vulkano::descriptor::{DescriptorSet, descriptor_set};
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::image::view::ImageView;
use vulkano::sampler::{Sampler, Filter, MipmapMode, SamplerAddressMode};
use vulkano::command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer, DynamicState};
use vulkano::sync::GpuFuture;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::{memory, sync, command_buffer, sampler};

use super::pipelines::background::{BackgroundPipeline, Vertex};
use super::pipelines::{PipelineError, Pipelines};
use super::model::FenceCheck;
use crate::math::{Vec4, Vec2, Vec3, Rot3, Isometry3, Point3, Color};
use crate::{config, debug};
use crate::renderer::eyes::Eyes;

#[allow(dead_code)]
#[derive(Copy, Clone)]
struct Intrinsics {
	focal: [Vec2; 2],
	proj: [Vec4; 2],
	coeffs: [Vec4; 2],
	scale: [Vec2; 2],
	center: [Vec2; 2],
}

pub struct Background {
	pipeline: Arc<BackgroundPipeline>,
	vertices: Arc<ImmutableBuffer<[Vertex]>>,
	intrinsics: Arc<CpuAccessibleBuffer<Intrinsics>>,
	set: Arc<dyn DescriptorSet + Send + Sync>,
	fence: Arc<FenceCheck>,
	shift: Vec3,
	extrinsics: (Rot3, Rot3),
}

impl Background {
	pub fn new(camera_image: Arc<AttachmentImage>, eyes: &Eyes, queue: &Arc<Queue>, pipelines: &mut Pipelines) -> Result<Background, BackgroundError> {
		let config = config::get();
		let pipeline: Arc<BackgroundPipeline> = pipelines.get()?;
		
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
			focal: [
				config.camera.left.focal_length.component_div(&config.camera.left.size.cast()),
				config.camera.right.focal_length.component_div(&config.camera.right.size.cast()),
			],
			proj: [
				eyes.raw_projection.0,
				eyes.raw_projection.1,
			],
			coeffs: [
				config.camera.left.coeffs.clone(),
				config.camera.right.coeffs.clone(),
			],
			scale: [
				config.camera.left.size.cast::<f32>().component_div(&config.camera.frame_buffer_size.cast()),
				config.camera.right.size.cast::<f32>().component_div(&config.camera.frame_buffer_size.cast()),
			],
			center: [
				(config.camera.left.center + config.camera.left.offset.cast()).component_div(&config.camera.frame_buffer_size.cast()),
				(config.camera.right.center + config.camera.right.offset.cast()).component_div(&config.camera.frame_buffer_size.cast()),
			],
		};
		
		let intrinsics = CpuAccessibleBuffer::from_data(queue.device().clone(),
		                                                BufferUsage{ uniform_buffer: true, ..BufferUsage::none() },
		                                                true,
		                                                intrinsics)?;
		
		let view = ImageView::new(camera_image)?;
		let sampler = Sampler::new(
			queue.device().clone(),
			Filter::Linear,
			Filter::Linear,
			MipmapMode::Nearest,
			SamplerAddressMode::ClampToEdge,
			SamplerAddressMode::ClampToEdge,
			SamplerAddressMode::ClampToEdge,
			0.0,
			1.0,
			0.0,
			1.0,
		)?;
		
		let set = Arc::new(
			PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layout(0).ok_or(BackgroundError::NoLayout)?.clone())
				.add_buffer(intrinsics.clone())?
				.add_sampled_image(view, sampler)?
				.build()?
		);
		
		let hmd_forwards = Rot3::face_towards(&-Vec3::z_axis(), &Vec3::y_axis());
		
		let left_forward = -config.camera.left.back;
		let left_down = -config.camera.left.right.cross(&left_forward);
		let left_extrinsics = hmd_forwards.rotation_to(&Rot3::face_towards(&left_forward, &left_down));
		
		let right_forward = -config.camera.right.back;
		let right_down = -config.camera.right.right.cross(&right_forward);
		let right_extrinsics = hmd_forwards.rotation_to(&Rot3::face_towards(&right_forward, &right_down));
		
		println!("{:?}", left_extrinsics.euler_angles());
		println!("{:?}", right_extrinsics.euler_angles());
		
		let fence = Arc::new(FenceCheck::new(vertices_promise)?);
		
		Ok(Background {
			pipeline,
			vertices,
			intrinsics,
			set,
			fence,
			shift: Vec3::zeros(),
			extrinsics: (left_extrinsics.inverse(), right_extrinsics.inverse()),
		})
	}
	
	pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, hmd_pose: &Isometry3) -> Result<(), BackgroundRenderError> {
		if !self.fence.check() { return Ok(()); }
		
		if let Ok(mut intrinsics) = self.intrinsics.write() {
			if debug::get_flag_or_default("KeyA") {
				intrinsics.focal[0].x -= 0.0001;
				intrinsics.focal[0].y -= 0.0001;
				intrinsics.focal[1].x -= 0.0001;
				intrinsics.focal[1].y -= 0.0001;
			} else if debug::get_flag_or_default("KeyD") {
				intrinsics.focal[0].x += 0.0001;
				intrinsics.focal[0].y += 0.0001;
				intrinsics.focal[1].x += 0.0001;
				intrinsics.focal[1].y += 0.0001;
			}
		}
		
		{
			let config = config::get();
		
			let hmd_pose = Isometry3::identity();
		
			debug::draw_point(hmd_pose.transform_point(&Point3::from(Point3::new(0.0, 0.0, -0.5).coords)), 32.0, Color::red());
		
			let left_cam = hmd_pose.transform_point(&Point3::from(Point3::new(0.0, 0.0, -0.5).coords + config.camera.left.position));
			debug::draw_point(&left_cam, 32.0, Color::green());
			debug::draw_line(&left_cam, left_cam + hmd_pose.transform_vector(&config.camera.left.right) / 20.0, 4.0, Color::cyan());
			debug::draw_line(&left_cam, left_cam + hmd_pose.transform_vector(&config.camera.left.back) / 20.0, 4.0, Color::dcyan());
		
			let right_cam = hmd_pose.transform_point(&Point3::from(Point3::new(0.0, 0.0, -0.5).coords + config.camera.right.position));
			debug::draw_point(&right_cam, 32.0, Color::blue());
			debug::draw_line(&right_cam, right_cam + hmd_pose.transform_vector(&config.camera.right.right) / 20.0, 4.0, Color::magenta());
			debug::draw_line(&right_cam, right_cam + hmd_pose.transform_vector(&config.camera.right.back) / 20.0, 4.0, Color::dmagenta());
		}
		
		let rotation = Rot3::from_euler_angles(-self.shift.y, self.shift.x, self.shift.z);
		
		let constants = (
			(rotation * self.extrinsics.0).to_homogeneous(),
			(rotation * self.extrinsics.1).to_homogeneous(),
		);
		
		builder.draw(self.pipeline.clone(),
		             &DynamicState::none(),
		             self.vertices.clone(),
		             self.set.clone(),
		             constants,
		             None)?;
		
		Ok(())
	}
}



#[derive(Debug, Error)]
pub enum BackgroundError {
	#[error(display = "Pipeline doesn't have specified layout")] NoLayout,
	#[error(display = "{}", _0)] PipelineError(#[error(source)] PipelineError),
	#[error(display = "{}", _0)] DeviceMemoryAllocError(#[error(source)] memory::DeviceMemoryAllocError),
	#[error(display = "{}", _0)] FlushError(#[error(source)] sync::FlushError),
	#[error(display = "{}", _0)] ImageCreationError(#[error(source)] vulkano::image::ImageCreationError),
	#[error(display = "{}", _0)] ImageViewCreationError(#[error(source)] vulkano::image::view::ImageViewCreationError),
	#[error(display = "{}", _0)] PersistentDescriptorSetError(#[error(source)] descriptor_set::PersistentDescriptorSetError),
	#[error(display = "{}", _0)] PersistentDescriptorSetBuildError(#[error(source)] descriptor_set::PersistentDescriptorSetBuildError),
	#[error(display = "{}", _0)] SamplerCreationError(#[error(source)] sampler::SamplerCreationError),
}

#[derive(Debug, Error)]
pub enum BackgroundRenderError {
	#[error(display = "{}", _0)] DrawError(#[error(source)] command_buffer::DrawError),
}
