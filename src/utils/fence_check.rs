use std::sync::Arc;
use anyhow::Result;
use vulkano::sync::future::{FenceSignalFuture, GpuFuture};

/// Shared fence of a submitted GPU future, used to check whether the work has finished.
#[derive(Clone)]
pub struct FenceCheck(Arc<FenceSignalFuture<Box<dyn GpuFuture>>>);

impl FenceCheck {
	/// Flushes `future` and signals a fence once it completes.
	pub fn new<GF>(future: GF) -> Result<FenceCheck>
		where GF: GpuFuture + 'static {
		Ok(FenceCheck(Arc::new(future.boxed().then_signal_fence_and_flush()?)))
	}
	
	/// Returns `true` if the GPU work has finished. Doesn't block.
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
	
	/// Returns a future that can be used to wait for the GPU work.
	pub fn future(&self) -> impl GpuFuture + use<> {
		self.0.clone()
	}
}
