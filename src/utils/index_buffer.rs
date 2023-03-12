use std::sync::Arc;
use std::any::{Any, TypeId};
use vulkano::buffer::{DeviceLocalBuffer, TypedBufferAccess};
use vulkano::command_buffer::allocator::CommandBufferAllocator;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::DeviceSize;

pub use crate::component::model::VertexIndex;

#[derive(Clone)]
pub enum DeviceLocalIndexBuffer {
	U8(Arc<DeviceLocalBuffer<[u8]>>),
	U16(Arc<DeviceLocalBuffer<[u16]>>),
	U32(Arc<DeviceLocalBuffer<[u32]>>),
}

impl DeviceLocalIndexBuffer {
	pub fn len(&self) -> DeviceSize {
		match self {
			DeviceLocalIndexBuffer::U8(buffer) => buffer.len(),
			DeviceLocalIndexBuffer::U16(buffer) => buffer.len(),
			DeviceLocalIndexBuffer::U32(buffer) => buffer.len(),
		}
	}
}

// TODO: Use specialization feature instead instead once it's ready
impl<VI: VertexIndex> Into<DeviceLocalIndexBuffer> for Arc<DeviceLocalBuffer<[VI]>> {
	fn into(self) -> DeviceLocalIndexBuffer {
		let type_id = TypeId::of::<VI>();
		
		let wrapped = &mut Some(self) as &mut dyn Any;
		
		if      type_id == TypeId::of::<u8 >() { DeviceLocalIndexBuffer::U8 (wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else if type_id == TypeId::of::<u16>() { DeviceLocalIndexBuffer::U16(wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else if type_id == TypeId::of::<u32>() { DeviceLocalIndexBuffer::U32(wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else { panic!("Only u8. u16 and u32 are allowed.") }
	}
}


pub trait AutoCommandBufferBuilderEx {
	fn bind_any_index_buffer(&mut self, index_buffer: DeviceLocalIndexBuffer) -> &mut Self;
}

impl<L, A> AutoCommandBufferBuilderEx for AutoCommandBufferBuilder<L, A>
where A: CommandBufferAllocator {
	fn bind_any_index_buffer(&mut self, index_buffer: DeviceLocalIndexBuffer) -> &mut Self {
		match index_buffer {
			DeviceLocalIndexBuffer::U8(buffer) => self.bind_index_buffer(buffer),
			DeviceLocalIndexBuffer::U16(buffer) => self.bind_index_buffer(buffer),
			DeviceLocalIndexBuffer::U32(buffer) => self.bind_index_buffer(buffer),
		};
		
		self
	}
}
