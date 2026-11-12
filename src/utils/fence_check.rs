use std::sync::Arc;
use anyhow::Result;
use vulkano::sync::future::{FenceSignalFuture, GpuFuture};

#[derive(Clone)]
pub struct FenceCheck(Arc<FenceSignalFuture<Box<dyn GpuFuture>>>);

impl FenceCheck {
	pub fn new<GF>(future: GF) -> Result<FenceCheck>
		where GF: GpuFuture + 'static {
		Ok(FenceCheck(Arc::new(future.boxed().then_signal_fence_and_flush()?)))
	}
	
	pub fn check(&self) -> bool {
		// TODO: https://github.com/vulkano-rs/vulkano/issues/2854
		match self.0.is_signaled() {
			Ok(false) => false,
			Ok(true) => match self.0.wait(None) {
				Ok(()) => true,
				Err(err) => panic!("Flushing Error: {}", err),
			},
			Err(err) => panic!("Flushing Error: {}", err),
		}
	}
	
	pub fn future(&self) -> impl GpuFuture + use<> {
		self.0.clone()
	}
}
