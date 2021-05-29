// I don't know what I'm doing
// Mostly rewrite from vulkano/sync/future/join.rs

use std::sync::Arc;

use vulkano::buffer::BufferAccess;
use vulkano::command_buffer::submit::{SubmitAnyBuilder, SubmitSemaphoresWaitBuilder, SubmitCommandBufferBuilder, SubmitBindSparseBuilder};
use vulkano::device::Device;
use vulkano::device::DeviceOwned;
use vulkano::device::Queue;
use vulkano::image::ImageAccess;
use vulkano::image::ImageLayout;
use vulkano::sync::AccessCheckError;
use vulkano::sync::AccessFlags;
use vulkano::sync::FlushError;
use vulkano::sync::GpuFuture;
use vulkano::sync::PipelineStages;

#[must_use]
pub struct VecFuture<GF> {
	device: Arc<Device>,
	vec: Vec<GF>,
}

impl<GF> VecFuture<GF> {
	pub fn new(device: Arc<Device>) -> Self {
		VecFuture {
			device,
			vec: vec![],
		}
	}
	
	pub fn push(&mut self, future: GF) {
		self.vec.push(future);
	}
}

unsafe impl<GF> DeviceOwned for VecFuture<GF>
	where GF: DeviceOwned {
	#[inline]
	fn device(&self) -> &Arc<Device> {
		&self.device
	}
}

unsafe impl<GF> GpuFuture for VecFuture<GF>
	where GF: GpuFuture {
	#[inline]
	fn cleanup_finished(&mut self) {
		for future in self.vec.iter_mut() {
			future.cleanup_finished();
		}
	}

	#[inline]
	fn flush(&self) -> Result<(), FlushError> {
		// Since each future remembers whether it has been flushed, there's no safety issue here
		// if we call this function multiple times.
		for future in self.vec.iter() {
			future.flush()?;
		}
		Ok(())
	}

	// TODO: Fix this shit
	#[inline]
	unsafe fn build_submission(&self) -> Result<SubmitAnyBuilder, FlushError> {
		let submissions = self.vec.iter()
		                          .map(|future| future.build_submission())
		                          .collect::<Result<Vec<_>, _>>()?;

		let semaphore = None::<SubmitSemaphoresWaitBuilder>;
		let mut command = None::<SubmitCommandBufferBuilder>;
		let bind_sparse = None::<SubmitBindSparseBuilder>;

		for (submission, _future) in submissions.into_iter().zip(self.vec.iter()) {
			match submission {
				SubmitAnyBuilder::Empty => {},
				SubmitAnyBuilder::SemaphoresWait(_s) => {
					// future.flush()?;
					// if let Some(merged) = &mut semaphore {
					// 	merged.merge(s);
					// } else {
					// 	semaphore = Some(s);
					// }
					unimplemented!()
				}
				SubmitAnyBuilder::CommandBuffer(c) => {
					if let Some(merged) = command.take() {
						command = Some(merged.merge(c));
					} else {
						command = Some(c);
					}
				}
				SubmitAnyBuilder::QueuePresent(_) => {
					// future.flush()?;
					unimplemented!()
				}
				SubmitAnyBuilder::BindSparse(_bs) => {
					// future.flush()?;
					// if let Some(merged) = &mut bind_sparse {
					// 	// TODO: this panics if both bind sparse have been given a fence already
					// 	//       annoying, but not impossible, to handle
					// 	merged.merge(bs).unwrap();
					// } else {
					// 	bind_sparse = Some(bs);
					// }
					unimplemented!()
				}
			}
		}
		
		Ok(match (semaphore, command, bind_sparse) {
			(None, None, None) => SubmitAnyBuilder::Empty,
			(Some(res), _, _) => SubmitAnyBuilder::SemaphoresWait(res),
			(None, Some(res), None) => SubmitAnyBuilder::CommandBuffer(res),
			(None, None, Some(res)) => SubmitAnyBuilder::BindSparse(res),
			(_, _, _) => unimplemented!(),
		})
	}

	#[inline]
	unsafe fn signal_finished(&self) {
		for future in self.vec.iter() {
			future.signal_finished();
		}
	}

	#[inline]
	fn queue_change_allowed(&self) -> bool {
		self.vec.iter()
		        .all(|future| future.queue_change_allowed())
	}

	#[inline]
	fn queue(&self) -> Option<Arc<Queue>> {
		let mut queue = None;
		let mut change_allowed = true;

		for future in self.vec.iter() {
			let cur_queue = match future.queue() {
				None => continue,
				Some(queue) => queue,
			};

			if change_allowed {
				queue = Some(cur_queue);
				change_allowed = future.queue_change_allowed();
			} else if queue.is_some() && queue.as_ref().unwrap().is_same(&cur_queue) {
				// Same queue
			} else if !future.queue_change_allowed() {
				return None;
			}
		}

		queue
	}

	#[inline]
	fn check_buffer_access(
		&self,
		buffer: &dyn BufferAccess,
		exclusive: bool,
		queue: &Queue,
	) -> Result<Option<(PipelineStages, AccessFlags)>, AccessCheckError> {
		let access = self.vec.iter()
		                     .map(|future| future.check_buffer_access(buffer, exclusive, queue))
		                     .filter(|access| match access {
			                     Err(AccessCheckError::Unknown) => false,
			                     _ => true
		                     })
		                     .collect::<Vec<_>>();

		if access.is_empty() {
			return Err(AccessCheckError::Unknown);
		}

		let mut flags = None;

		for result in access {
			flags = match (result?, flags) {
				(None, f) => f,
				(f, None) => f,
				(Some((a1, a2)), Some((b1, b2))) => Some((a1 | b1, a2 | b2)),
			}
		}

		return Ok(flags);
	}

	#[inline]
	fn check_image_access(
		&self,
		image: &dyn ImageAccess,
		layout: ImageLayout,
		exclusive: bool,
		queue: &Queue,
	) -> Result<Option<(PipelineStages, AccessFlags)>, AccessCheckError> {
		let access = self.vec.iter()
		                 .map(|future| future.check_image_access(image, layout, exclusive, queue))
		                 .filter(|access| match access {
			                 Err(AccessCheckError::Unknown) => false,
			                 _ => true
		                 })
		                 .collect::<Vec<_>>();

		if access.is_empty() {
			return Err(AccessCheckError::Unknown);
		}

		let mut flags = None;

		for result in access {
			flags = match (result?, flags) {
				(None, f) => f,
				(f, None) => f,
				(Some((a1, a2)), Some((b1, b2))) => Some((a1 | b1, a2 | b2)),
			}
		}

		return Ok(flags);
	}
}

