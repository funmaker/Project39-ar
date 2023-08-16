use std::sync::Arc;
use vulkano::buffer::Subbuffer;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::{ComputePipeline, Pipeline};

mod builder;
mod joint;
mod rigid_body;
mod sub_mesh;

use crate::math::IVec4;
use crate::utils::{FenceCheck, IndexSubbuffer};
use super::super::{ModelError, VertexIndex};
use super::{MMDBone, Vertex};
pub use builder::MMDModelSharedBuilder;
pub use joint::JointDesc;
pub use rigid_body::ColliderDesc;
pub use sub_mesh::{MaterialInfo, SubMesh, SubMeshDesc};


pub struct MMDModelShared {
	pub vertices: Subbuffer<[Vertex]>,
	pub indices: IndexSubbuffer,
	pub sub_meshes: Vec<SubMesh>,
	pub default_bones: Vec<MMDBone>,
	pub morphs_offsets: Subbuffer<[IVec4]>,
	pub morphs_sizes: Vec<usize>,
	pub morphs_max_size: usize,
	pub morphs_pipeline: Arc<ComputePipeline>,
	pub fence: FenceCheck,
	pub colliders: Vec<ColliderDesc>,
	pub joints: Vec<JointDesc>,
}

impl MMDModelShared {
	pub fn new<VI: VertexIndex>(vertices: Vec<Vertex>, indices: Vec<VI>) -> MMDModelSharedBuilder<VI> {
		MMDModelSharedBuilder::new(vertices, indices)
	}
	
	pub fn layouts(&self) -> Result<(Arc<DescriptorSetLayout>, Option<Arc<DescriptorSetLayout>>), ModelError> {
		let main = self.sub_meshes.first()
		                          .map(|mesh| mesh.main.0.clone())
		                          .ok_or(ModelError::NoLayout)
		                          .and_then(|pipeline| pipeline.layout().set_layouts().get(0).cloned().ok_or(ModelError::NoLayout))?;
		
		let edge = self.sub_meshes.iter()
		                          .find(|mesh| mesh.edge.is_some())
		                          .map(|mesh| mesh.edge.clone().unwrap().0)
		                          .map(|pipeline| pipeline.layout().set_layouts().get(0).cloned().ok_or(ModelError::NoLayout))
		                          .transpose()?;
		
		Ok((main, edge))
	}
}

