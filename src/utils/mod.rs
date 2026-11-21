pub mod from_args;
pub mod user_events;
mod fence_check;
mod fps_counter;
mod gui;
mod id_gen;
mod images;
mod index_buffer;
mod input;
mod mut_mark;
mod pattern;
mod rapier;
mod vulkan;
mod ref_cell_iter;
mod openvr;

pub use fence_check::*;
pub use fps_counter::*;
pub use gui::*;
pub use id_gen::*;
pub use images::*;
pub use index_buffer::*;
pub use input::*;
pub use mut_mark::*;
pub use pattern::*;
pub use rapier::*;
pub use vulkan::*;
pub use ref_cell_iter::*;
pub use openvr::*;


pub trait IntoBoxed<T: ?Sized>: 'static {
	fn into(self) -> Box<T>;
}

macro_rules! collection {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$(($k, $v),)*]))
    }};
    // set-like
    ($($v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$($v,)*]))
    }};
}
