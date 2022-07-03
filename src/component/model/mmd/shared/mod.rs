use std::sync::Arc;
use vulkano::buffer::{CpuBufferPool, ImmutableBuffer};
use vulkano::descriptor_set::layout::DescriptorSetLayout;
use vulkano::pipeline::{ComputePipeline, Pipeline};

mod builder;
mod rigid_body;
mod joint;
mod sub_mesh;

use crate::component::model::{ModelError, VertexIndex};
use crate::renderer::pipelines::mmd::MMDPipelineOpaque;
use crate::renderer::Renderer;
use crate::utils::{FenceCheck, ImmutableIndexBuffer, NgPod};
use crate::math::{AMat4, IVec4};
use super::{MMDBone, Vertex};
pub use builder::MMDModelSharedBuilder;
pub use sub_mesh::{MaterialInfo, SubMesh, SubMeshDesc};
pub use rigid_body::ColliderDesc;
pub use joint::JointDesc;

pub struct MMDModelShared {
	pub vertices: Arc<ImmutableBuffer<[Vertex]>>,
	pub indices: ImmutableIndexBuffer,
	pub sub_meshes: Vec<SubMesh>,
	pub default_bones: Vec<MMDBone>,
	pub bones_pool: CpuBufferPool<NgPod<AMat4>>,
	pub morphs_offsets: Arc<ImmutableBuffer<[NgPod<IVec4>]>>,
	pub morphs_sizes: Vec<usize>,
	pub morphs_max_size: usize,
	pub morphs_pool: CpuBufferPool<NgPod<IVec4>>,
	pub morphs_pipeline: Arc<ComputePipeline>,
	pub fence: FenceCheck,
	pub colliders: Vec<ColliderDesc>,
	pub joints: Vec<JointDesc>,
}

impl MMDModelShared {
	pub fn new<VI: VertexIndex>(vertices: Vec<Vertex>, indices: Vec<VI>) -> MMDModelSharedBuilder<VI> {
		MMDModelSharedBuilder::new(vertices, indices)
	}
	
	pub fn commons_layout(&self, renderer: &mut Renderer) -> Result<Arc<DescriptorSetLayout>, ModelError> {
		self.sub_meshes.first()
		               .map(|mesh| mesh.main.0.clone())
		               .ok_or(ModelError::NoLayout)
		               .or_else(|_| renderer.pipelines.get::<MMDPipelineOpaque>().map_err(Into::into).map(Into::into))
		               .and_then(|pipeline| pipeline.layout().set_layouts().get(0).cloned().ok_or(ModelError::NoLayout))
	}
}

