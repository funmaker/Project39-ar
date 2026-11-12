use std::time::Duration;
use anyhow::Result;

use crate::application::{Entity, Application, Hand};
use super::{Component, ComponentBase, ComponentInner};


/// Maximum thruster force.
const FORCE: f32 = 1000.0;

/// Which right controller touchpad direction fires the thruster.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ThrusterDirection {
	Forward,
	Back,
	Left,
	Right,
}

/// Vehicle thruster controlled with the right controller's touchpad while the player is seated
/// (see `Seat`). Pushes the entity along its local Y axis.
#[derive(ComponentBase)]
pub struct Thruster {
	#[inner] inner: ComponentInner,
	direction: ThrusterDirection,
}

impl Thruster {
	pub fn new(direction: ThrusterDirection) -> Self {
		Thruster {
			inner: ComponentInner::new_norender(),
			direction,
		}
	}
}

impl Component for Thruster {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<()> {
		if let Some(root) = application.find_entity(|e| e.name == "VR Root") {
			if root.has_tag("Seat") {
				let (x, y) = application.input.controller(Hand::Right)
				                        .map(|input| (input.axis(0), input.axis(1)))
				                        .unwrap_or_default();
				
				let thrust = FORCE * match self.direction {
					ThrusterDirection::Forward => -y,
					ThrusterDirection::Back => y,
					ThrusterDirection::Left => x,
					ThrusterDirection::Right => -x,
				};
			
				let force = entity.state().position.transform_vector(&vector!(0.0, thrust, 0.0));
				
				application.physics.borrow_mut().bodies.get_mut(entity.rigid_body).unwrap().add_force(force.into(), true);
			}
		}
		
		Ok(())
	}
}
