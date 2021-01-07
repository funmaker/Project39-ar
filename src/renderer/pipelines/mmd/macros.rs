macro_rules! mmd_shaders {
	( $( $type:literal $name:ident = $source:literal; )* ) => { $(
		mod $name {
			#[allow(dead_code)]
			const SOURCE: &'static str = include_str!(concat!(stringify!($name), ".glsl")); // https://github.com/vulkano-rs/vulkano/issues/1349
			
			vulkano_shaders::shader! {
				ty: $type,
				path: $source
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
		use std::ops::Deref;
		use derive_deref::Deref;
		use vulkano::SafeDeref;
		use vulkano::pipeline::GraphicsPipeline;
		use vulkano::framebuffer::{RenderPassAbstract, Subpass};
		use vulkano::pipeline::viewport::Viewport;
		use vulkano::device::DeviceOwned;
		use vulkano::pipeline::vertex::SingleBufferDefinition;
		use vulkano::descriptor::PipelineLayoutAbstract;
		
		use $crate::renderer::pipelines::{Pipeline, PipelineError, pre_mul_alpha_blending};
		use $crate::renderer::{model, RenderPass};
		
		type MMDPipelineInner = GraphicsPipeline<
			SingleBufferDefinition<model::mmd::Vertex>,
			Box<dyn PipelineLayoutAbstract + Send + Sync>,
			Arc<dyn RenderPassAbstract + Send + Sync>
		>;
		
		#[derive(Clone)]
		pub enum MMDPipelineAny {
			$( $name(Arc<$name>), )*
		}
		
		impl Deref for MMDPipelineAny {
			type Target = MMDPipelineInner;
			
		    fn deref(&self) -> &Self::Target {
		        match self {
					$( MMDPipelineAny::$name(ref inner) => &inner.0, )*
		        }
		    }
		}
		
		unsafe impl SafeDeref for MMDPipelineAny {}
		
		$(
			#[derive(Debug, Deref)]
			$pub struct $name(MMDPipelineInner);
			
			unsafe impl SafeDeref for $name {}
			
			impl Pipeline for $name {
				fn new(render_pass: &Arc<RenderPass>, frame_buffer_size: (u32, u32)) -> Result<Arc<dyn Pipeline>, PipelineError> {
					use $vertex_shader as vertex_shader;
					use $fragment_shader as fragment_shader;
					
					let device = render_pass.device();
					let vs = vertex_shader::Shader::load(device.clone()).unwrap();
					let fs = fragment_shader::Shader::load(device.clone()).unwrap();
					
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
						.vertex_input_single_buffer()
						.vertex_shader(vs.main_entry_point(), vs_consts)
						.fragment_shader(fs.main_entry_point(), fs_consts)
						.render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
						.front_face_clockwise()
						.depth_stencil_simple_depth()
						.cull_mode_back()
						.viewports(Some(Viewport {
							origin: [0.0, 0.0],
							dimensions: [frame_buffer_size.0 as f32, frame_buffer_size.1 as f32],
							depth_range: 0.0..1.0,
						}));
					
					Ok(Arc::new($name(
						$code.build(device.clone())?
					)))
				}
			}
			
			impl From<Arc<$name>> for MMDPipelineAny {
			    fn from(pipeline: Arc<$name>) -> Self {
			        MMDPipelineAny::$name(pipeline)
			    }
			}
		)*
	}
}
