use std::sync::Arc;
use std::time::Duration;
use arc_swap::ArcSwap;
use vulkano::sync::{FenceSignalFuture, GpuFuture, FlushError};

enum FenceState {
	Done(bool),
	Pending(FenceSignalFuture<Box<dyn GpuFuture>>)
}

impl FenceState {
	fn new<GF>(future: GF)
	           -> Result<FenceState, FlushError>
		where GF: GpuFuture + 'static {
		Ok(FenceState::Pending((Box::new(future) as Box<dyn GpuFuture>).then_signal_fence_and_flush()?))
	}
}

pub struct FenceCheck(ArcSwap<FenceState>);

impl FenceCheck {
	pub fn new<GF>(future: GF)
	           -> Result<FenceCheck, FlushError>
	           where GF: GpuFuture + 'static {
		Ok(FenceCheck(ArcSwap::from_pointee(FenceState::new(future)?)))
	}
	
	pub fn check(&self) -> bool {
		match &**self.0.load() {
			FenceState::Done(result) => *result,
			FenceState::Pending(fence) => {
				match fence.wait(Some(Duration::new(0, 0))) {
					Err(FlushError::Timeout) => false,
					Ok(()) => {
						self.0.swap(Arc::new(FenceState::Done(true)));
						true
					}
					Err(err) => {
						eprintln!("Error while loading renderer.model: {:?}", err);
						self.0.swap(Arc::new(FenceState::Done(false)));
						false
					}
				}
			}
		}
	}
}
