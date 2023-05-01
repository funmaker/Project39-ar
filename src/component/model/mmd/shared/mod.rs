use std::sync::Arc;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::{ComputePipeline, Pipeline};
use vulkano::buffer::Subbuffer;

mod builder;
mod rigid_body;
mod joint;
mod sub_mesh;

use crate::component::model::{ModelError, VertexIndex};
use crate::utils::{FenceCheck, IndexSubbuffer};
use crate::math::IVec4;
use super::{MMDBone, Vertex};
pub use builder::MMDModelSharedBuilder;
pub use sub_mesh::{MaterialInfo, SubMesh, SubMeshDesc};
pub use rigid_body::ColliderDesc;
pub use joint::JointDesc;

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

