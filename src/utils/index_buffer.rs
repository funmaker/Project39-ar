use std::sync::Arc;
use std::any::{Any, TypeId};
use vulkano::buffer::{ImmutableBuffer, TypedBufferAccess};
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::DeviceSize;

pub use crate::component::model::VertexIndex;

#[derive(Clone)]
pub enum ImmutableIndexBuffer {
	U8(Arc<ImmutableBuffer<[u8]>>),
	U16(Arc<ImmutableBuffer<[u16]>>),
	U32(Arc<ImmutableBuffer<[u32]>>),
}

impl ImmutableIndexBuffer {
	pub fn len(&self) -> DeviceSize {
		match self {
			ImmutableIndexBuffer::U8(buffer) => buffer.len(),
			ImmutableIndexBuffer::U16(buffer) => buffer.len(),
			ImmutableIndexBuffer::U32(buffer) => buffer.len(),
		}
	}
}

// TODO: Use specialization feature instead instead once it's ready
impl<VI: VertexIndex> Into<ImmutableIndexBuffer> for Arc<ImmutableBuffer<[VI]>> {
	fn into(self) -> ImmutableIndexBuffer {
		let type_id = TypeId::of::<VI>();
		
		let wrapped = &mut Some(self) as &mut dyn Any;
		
		if      type_id == TypeId::of::<u8 >() { ImmutableIndexBuffer::U8 (wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else if type_id == TypeId::of::<u16>() { ImmutableIndexBuffer::U16(wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else if type_id == TypeId::of::<u32>() { ImmutableIndexBuffer::U32(wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else { panic!("Only u8. u16 and u32 are allowed.") }
	}
}


pub trait AutoCommandBufferBuilderEx {
	fn bind_any_index_buffer(&mut self, index_buffer: ImmutableIndexBuffer) -> &mut Self;
}

impl<L, P> AutoCommandBufferBuilderEx for AutoCommandBufferBuilder<L, P> {
	fn bind_any_index_buffer(&mut self, index_buffer: ImmutableIndexBuffer) -> &mut Self {
		match index_buffer {
			ImmutableIndexBuffer::U8(buffer) => self.bind_index_buffer(buffer),
			ImmutableIndexBuffer::U16(buffer) => self.bind_index_buffer(buffer),
			ImmutableIndexBuffer::U32(buffer) => self.bind_index_buffer(buffer),
		};
		
		self
	}
}
