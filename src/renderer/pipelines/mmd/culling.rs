use std::fmt::Debug;
use vulkano::pipeline::GraphicsPipelineBuilder;

pub trait MMDCullMode: Debug + Send + Sync + 'static {
	fn apply<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>
	        (pipeline: GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>)
	        -> GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>;
}

#[derive(Debug)]
pub struct NoCull;

impl MMDCullMode for NoCull {
	fn apply<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>
	        (pipeline: GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>)
	        -> GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp> {
		pipeline.cull_mode_disabled()
	}
}

#[derive(Debug)]
pub struct Cull;

impl MMDCullMode for Cull {
	fn apply<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>
	        (pipeline: GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp>)
	         -> GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp> {
		pipeline.cull_mode_back()
	}
}

pub trait MMDCullModeEx: Sized {
	fn cull_mode<CullMode>(self) -> Self where CullMode: MMDCullMode;
}

impl<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp> MMDCullModeEx for GraphicsPipelineBuilder<Vdef, Vs, Vss, Tcs, Tcss, Tes, Tess, Gs, Gss, Fs, Fss, Rp> {
	fn cull_mode<CullMode>(self) -> Self where CullMode: MMDCullMode {
		CullMode::apply(self)
	}
}
