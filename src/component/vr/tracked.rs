use std::time::Duration;
use simba::scalar::SupersetOf;
use openvr::TrackedDeviceIndex;

use crate::application::{Entity, Application};
use crate::math::{AMat4, Similarity3, VRSlice};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};

#[derive(ComponentBase)]
pub struct VrTracked {
	#[inner] inner: ComponentInner,
	device_id: TrackedDeviceIndex,
}

impl VrTracked {
	pub fn new(device_id: TrackedDeviceIndex) -> Self {
		VrTracked {
			inner: ComponentInner::new(),
			device_id
		}
	}
}

impl Component for VrTracked {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if !application.vr.as_ref().unwrap().lock().unwrap().system.is_tracked_device_connected(self.device_id) {
			println!("Removing {}", entity.name);
			entity.remove();
		}
		
		let pose = application.vr_poses.render[self.device_id as usize];
		
		if !pose.pose_is_valid() {
			return Ok(());
		}
		
		let orientation = AMat4::from_slice34(pose.device_to_absolute_tracking());
		let orientation: Similarity3 = orientation.to_subset().unwrap();
		
		let mut state = entity.state_mut();
		state.position = orientation.isometry;
		state.velocity = pose.velocity().clone().into();
		state.angular_velocity = pose.angular_velocity().clone().into();
		
		Ok(())
	}
}
