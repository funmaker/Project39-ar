// use std::sync::Arc;
// use cgmath::{Matrix4, Vector3};
// use vulkano::buffer::{DeviceLocalBuffer, CpuBufferPool};
//
// struct CommonsVBO {
// 	projection: [Matrix4<f32>; 2],
// 	view: [Matrix4<f32>; 2],
// 	light_direction: Vector3<f32>,
// 	ambient: f32,
// }
//
// pub struct Commons {
// 	pub buffer: Arc<DeviceLocalBuffer<CommonsVBO>>,
// 	pool: Arc<CpuBufferPool<CommonsVBO>>,
// }
//
// impl Commons {
// 	pub fn new(device: &Arc<Device>, queue_families: ) -> Self {
// 		let buffer = DeviceLocalBuffer::new();
//
// 		Commons {
// 			buffer,
// 			pool,
// 		}
// 	}
// }
