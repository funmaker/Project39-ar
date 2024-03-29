use std::any::{Any, TypeId};
use vulkano::DeviceSize;
use vulkano::buffer::Subbuffer;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::command_buffer::allocator::CommandBufferAllocator;

pub use crate::component::model::VertexIndex;


#[derive(Clone)]
pub enum IndexSubbuffer {
	U8(Subbuffer<[u8]>),
	U16(Subbuffer<[u16]>),
	U32(Subbuffer<[u32]>),
}

impl IndexSubbuffer {
	pub fn len(&self) -> DeviceSize {
		match self {
			IndexSubbuffer::U8(buffer) => buffer.len(),
			IndexSubbuffer::U16(buffer) => buffer.len(),
			IndexSubbuffer::U32(buffer) => buffer.len(),
		}
	}
}

// TODO: Use specialization feature instead instead once it's ready
impl<VI: VertexIndex> Into<IndexSubbuffer> for Subbuffer<[VI]> {
	fn into(self) -> IndexSubbuffer {
		let type_id = TypeId::of::<VI>();
		
		let wrapped = &mut Some(self) as &mut dyn Any;
		
		if      type_id == TypeId::of::<u8 >() { IndexSubbuffer::U8 (wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else if type_id == TypeId::of::<u16>() { IndexSubbuffer::U16(wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else if type_id == TypeId::of::<u32>() { IndexSubbuffer::U32(wrapped.downcast_mut::<Option<_>>().unwrap().take().unwrap()) }
		else { panic!("Only u8. u16 and u32 are allowed.") }
	}
}


pub trait AutoCommandBufferBuilderEx {
	fn bind_any_index_buffer(&mut self, index_buffer: IndexSubbuffer) -> &mut Self;
}

impl<L, A> AutoCommandBufferBuilderEx for AutoCommandBufferBuilder<L, A>
where A: CommandBufferAllocator {
	fn bind_any_index_buffer(&mut self, index_buffer: IndexSubbuffer) -> &mut Self {
		match index_buffer {
			IndexSubbuffer::U8(buffer) => self.bind_index_buffer(buffer),
			IndexSubbuffer::U16(buffer) => self.bind_index_buffer(buffer),
			IndexSubbuffer::U32(buffer) => self.bind_index_buffer(buffer),
		};
		
		self
	}
}
