use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::ComponentRef;
use crate::component::physics::joint::JointComponent;
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};


#[derive(ComponentBase)]
pub struct MMDRigidBody {
	#[inner] inner: ComponentInner,
	pub bone: usize,
	pub joint: ComponentRef<JointComponent>,
}

impl MMDRigidBody {
	pub fn new(bone: usize, joint: ComponentRef<JointComponent>) -> Self {
		MMDRigidBody {
			inner: ComponentInner::new_norender(),
			bone,
			joint,
		}
	}
}

impl Component for MMDRigidBody {
	fn tick(&self, _entity: &Entity, _application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		
		Ok(())
	}
}
