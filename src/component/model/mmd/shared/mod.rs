use std::sync::Arc;
use anyhow::Result;
use vulkano::buffer::Subbuffer;
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::{ComputePipeline, Pipeline};

mod bone;
mod builder;
mod collider;
mod joint;
mod sub_mesh;

use crate::renderer::pipelines::PipelineNoLayoutError;
use crate::math::IVec4;
use crate::utils::{FenceCheck, IndexSubbuffer};
use super::super::VertexIndex;
use super::Vertex;
pub use bone::{BoneDesc, BoneConnection};
pub use builder::MMDModelSharedBuilder;
pub use collider::ColliderDesc;
pub use joint::{JointDesc, ConeTwistDesc};
pub use sub_mesh::{MaterialInfo, SubMesh, SubMeshDesc};


/// GPU data of an MMD model shared by all its instances: mesh, materials, default bones,
/// morph offsets and physics descriptions.
pub struct MMDModelShared {
	pub vertices: Subbuffer<[Vertex]>,
	pub indices: IndexSubbuffer,
	pub sub_meshes: Vec<SubMesh>,
	pub default_bones: Vec<BoneDesc>,
	/// Vertex offsets of all morphs, `morphs_max_size` entries per morph. See `MMDModelSharedBuilder::build`.
	pub morphs_offsets: Subbuffer<[IVec4]>,
	/// Number of vertex offsets of each morph.
	pub morphs_sizes: Vec<usize>,
	/// Size of the largest morph, rounded up to `MORPH_GROUP_SIZE`.
	pub morphs_max_size: usize,
	pub morphs_pipeline: Arc<ComputePipeline>,
	/// Signaled once all model data has been uploaded.
	pub fence: FenceCheck,
	pub colliders: Vec<ColliderDesc>,
	pub joints: Vec<JointDesc>,
}

impl MMDModelShared {
	/// Starts building a model from its vertices and indices.
	pub fn new<VI: VertexIndex>(vertices: Vec<Vertex>, indices: Vec<VI>) -> MMDModelSharedBuilder<VI> {
		MMDModelSharedBuilder::new(vertices, indices)
	}
	
	/// Returns the per-model descriptor set layouts of the main and edge pipelines.
	pub fn layouts(&self) -> Result<(Arc<DescriptorSetLayout>, Option<Arc<DescriptorSetLayout>>)> {
		let main = self.sub_meshes.first()
		                          .map(|mesh| mesh.main.0.clone())
		                          .ok_or(PipelineNoLayoutError)
		                          .and_then(|pipeline| pipeline.layout().set_layouts().get(0).cloned().ok_or(PipelineNoLayoutError))?;
		
		let edge = self.sub_meshes.iter()
		                          .find(|mesh| mesh.edge.is_some())
		                          .map(|mesh| mesh.edge.clone().unwrap().0)
		                          .map(|pipeline| pipeline.layout().set_layouts().get(0).cloned().ok_or(PipelineNoLayoutError))
		                          .transpose()?;
		
		Ok((main, edge))
	}
}

