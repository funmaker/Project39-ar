use std::time::Duration;
use simba::scalar::SupersetOf;
use openvr::TrackedDeviceIndex;

use crate::application::{Entity, Application};
use crate::math::{AMat4, Similarity3, VRSlice};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::component::vr::VrRoot;

#[derive(ComponentBase)]
pub struct VrTracked {
	#[inner] inner: ComponentInner,
	pub device_id: TrackedDeviceIndex,
	pub root: ComponentRef<VrRoot>,
}

impl VrTracked {
	pub fn new(device_id: TrackedDeviceIndex, root: ComponentRef<VrRoot>) -> Self {
		VrTracked {
			inner: ComponentInner::new_norender(),
			device_id,
			root,
		}
	}
}

impl Component for VrTracked {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if !application.vr.as_ref().unwrap().lock().unwrap().system.is_tracked_device_connected(self.device_id) {
			println!("Removing {}", entity.name);
			entity.remove();
		}
		
		let root_pos = match self.root.entity().get(application) {
			Some(root) => *root.state().position,
			None => {
				entity.remove();
				return Ok(());
			}
		};
		
		let pose = application.vr_poses.render[self.device_id as usize];
		
		if !pose.pose_is_valid() {
			return Ok(());
		}
		
		let orientation = AMat4::from_slice34(pose.device_to_absolute_tracking());
		let orientation: Similarity3 = orientation.to_subset().unwrap();
		
		let mut state = entity.state_mut();
		*state.position = root_pos * orientation.isometry;
		*state.velocity = root_pos.transform_vector(&pose.velocity().clone().into());
		*state.angular_velocity = root_pos.transform_vector(&pose.angular_velocity().clone().into());
		
		Ok(())
	}
}
