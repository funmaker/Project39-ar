use std::sync::Arc;
use std::time::Duration;
use vulkano::sync::future::{FenceSignalFuture, GpuFuture, FlushError};


#[derive(Clone)]
pub struct FenceCheck(Arc<FenceSignalFuture<Box<dyn GpuFuture>>>);

impl FenceCheck {
	pub fn new<GF>(future: GF)
	               -> Result<FenceCheck, FlushError>
		where GF: GpuFuture + 'static {
		Ok(FenceCheck(Arc::new(future.boxed().then_signal_fence_and_flush()?)))
	}
	
	pub fn check(&self) -> bool {
		match self.0.wait(Some(Duration::new(0, 0))) {
			Err(FlushError::Timeout) => false,
			Ok(()) => true,
			Err(err) => panic!("Flushing Error: {}", err),
		}
	}
	
	pub fn future(&self) -> impl GpuFuture {
		self.0.clone()
	}
}
