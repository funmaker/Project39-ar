use std::fmt::Debug;
use std::hash::Hash;
use bytemuck::Pod;
use vulkano::pipeline::graphics::input_assembly::Index;

pub mod billboard;
pub mod gimp;
pub mod mmd;
pub mod simple;

pub use self::mmd::MMDModel;
pub use simple::SimpleModel;


pub trait VertexIndex: Index + Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}
impl<T> VertexIndex for T where T: Index + Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}
