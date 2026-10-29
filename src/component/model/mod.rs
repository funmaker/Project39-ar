use std::fmt::Debug;
use std::hash::Hash;
use bytemuck::Pod;

pub mod billboard;
pub mod gimp;
pub mod mmd;
pub mod simple;

pub use self::mmd::MMDModel;
pub use simple::SimpleModel;


pub trait VertexIndex: Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}
impl<T> VertexIndex for T where T: Pod + Copy + Send + Sync + Sized + Into<u32> + Hash + Debug + 'static {}
