
macro_rules! mmd_shaders {
	( $( $type:literal $name:ident = $source:literal; )* ) => { $(
		mod $name {
			
			vulkano_shaders::shader! {
				ty: $type,
				path: $source,
				include: [ "src/component/model/mmd/pipeline" ],
				spirv_version: "1.3"
			}
		}
	)* }
}

macro_rules! mmd_pipelines {
	( $(
		$pub:vis pipeline $name:ident {
			shader vs = $vertex_shader:path $( { $( $vertex_const_name:ident: $vertex_const_value:expr ),* } )*;
			shader fs = $fragment_shader:path $( { $( $fragment_const_name:ident: $fragment_const_value:expr ),* } )*;
			config $builder:ident $code:block
		}
	)* ) => {
		use std::sync::Arc;
		use vulkano::render_pass::RenderPass;
		use vulkano::device::DeviceOwned;
		use vulkano::pipeline::GraphicsPipeline;
				use vulkano::pipeline::graphics::depth_stencil::DepthStencilState;
		use vulkano::pipeline::graphics::rasterization::{CullMode, RasterizationState, FrontFace};
		use vulkano::pipeline::graphics::viewport::ViewportState;
		use vulkano::pipeline::graphics::multisample::MultisampleState;
		use vulkano::pipeline::graphics::vertex_input::Vertex as VertexTy;
		use vulkano::image::SampleCount;
		
		use $crate::renderer::pipelines::{PipelineConstructor, PipelineError, pre_mul_alpha_blending};
		
		$(
			$pub struct $name;
			
			impl PipelineConstructor for $name {
				type PipeType = GraphicsPipeline;
				
				fn new(render_pass: &Arc<RenderPass>) -> Result<Arc<Self::PipeType>, PipelineError> {
					use $vertex_shader as vertex_shader;
					use $fragment_shader as fragment_shader;
					
					let device = render_pass.device();
					let vs = vertex_shader::load(device.clone()).unwrap();
					let fs = fragment_shader::load(device.clone()).unwrap();
					
					#[allow(unused_variables)]
					let vs_consts = ();
					$(
						let vs_consts = vertex_shader::SpecializationConstants{
							$( $vertex_const_name: $vertex_const_value ),*
						};
					)*
					
					#[allow(unused_variables)]
					let fs_consts = ();
					$(
						let fs_consts = fragment_shader::SpecializationConstants{
							$( $fragment_const_name: $fragment_const_value ),*
						};
					)*
					
					let $builder = GraphicsPipeline::start()
						.vertex_input_state(Vertex::per_vertex())
						.vertex_shader(vs.entry_point("main").unwrap(), vs_consts)
						.fragment_shader(fs.entry_point("main").unwrap(), fs_consts)
						.render_pass(render_pass.clone().first_subpass())
						.depth_stencil_state(DepthStencilState::simple_depth_test())
						.rasterization_state(RasterizationState::new().cull_mode(CullMode::Back).front_face(FrontFace::Clockwise))
						.viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
						.multisample_state(MultisampleState {
							rasterization_samples: render_pass.clone().first_subpass().num_samples().unwrap_or(SampleCount::Sample1),
							..MultisampleState::new()
						});
					
					Ok(
						$code.build(device.clone())?
					)
				}
			}
		)*
	}
}
