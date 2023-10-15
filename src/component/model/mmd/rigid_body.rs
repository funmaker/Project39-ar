use std::time::Duration;

use crate::application::{Entity, Application};
use super::super::super::{ComponentRef, Component, ComponentBase, ComponentInner, ComponentError};
use super::super::super::physics::joint::JointComponent;
use super::BodyPart;


#[derive(ComponentBase)]
pub struct MMDRigidBody {
	#[inner] inner: ComponentInner,
	pub bone: usize,
	pub body_part: Option<BodyPart>,
	pub joint: ComponentRef<JointComponent>,
}

impl MMDRigidBody {
	pub fn new(bone: usize, body_part: Option<BodyPart>, joint: ComponentRef<JointComponent>) -> Self {
		MMDRigidBody {
			inner: ComponentInner::new_norender(),
			bone,
			body_part,
			joint,
		}
	}
}

impl Component for MMDRigidBody {
	fn tick(&self, _entity: &Entity, _application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		
		Ok(())
	}
}
