use rapier3d::prelude::Collider;
use rapier3d::dynamics::RigidBody;

use crate::component::physics::collider::ColliderComponent;
use crate::application::{Application, Entity, EntityRef};
use crate::component::ComponentRef;

pub trait ColliderEx {
	fn component_ref(&self) -> ComponentRef<ColliderComponent>;
	
	fn entity_ref(&self) -> EntityRef {
		self.component_ref().entity()
	}
	
	fn component<'a>(&self, application: &'a Application) -> &'a ColliderComponent {
		self.component_ref().get(application).unwrap()
	}
	
	fn entity<'a>(&self, application: &'a Application) -> &'a Entity {
		self.component_ref().entity().get(application).unwrap()
	}
}

impl ColliderEx for Collider {
	fn component_ref(&self) -> ComponentRef<ColliderComponent> {
		let eid = self.user_data & 0xFFFF_FFFF_FFFF_FFFF;
		let cid = self.user_data >> 64;
		
		ComponentRef::new(eid as u64, cid as u64)
	}
}

pub trait RigidBodyEx {
	fn entity_ref(&self) -> EntityRef;
	
	fn entity<'a>(&self, application: &'a Application) -> &'a Entity {
		self.entity_ref().get(application).unwrap()
	}
}

impl RigidBodyEx for RigidBody {
	fn entity_ref(&self) -> EntityRef {
		let eid = self.user_data & 0xFFFF_FFFF_FFFF_FFFF;
		
		EntityRef::new(eid as u64)
	}
}

pub fn get_userdata(eid: u64, cid: u64) -> u128 {
	eid as u128 + ((cid as u128) << 64)
}
