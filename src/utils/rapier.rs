use rapier3d::dynamics::RigidBody;
use rapier3d::prelude::Collider;

use crate::application::{Application, Entity, EntityRef};
use crate::component::ComponentRef;
use crate::component::physics::collider::ColliderComponent;


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
		let (eid, cid) = from_user_data(self.user_data);
		
		ComponentRef::new(eid, cid)
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
		let (eid, _) = from_user_data(self.user_data);
		
		EntityRef::new(eid)
	}
}

pub fn get_user_data(eid: u64, cid: u64) -> u128 {
	eid as u128 + ((cid as u128) << 64)
}

pub fn from_user_data(userdata: u128) -> (u64, u64) {
	let eid = (userdata % (1 << 64)) as u64;
	let cid = (userdata >> 64) as u64;
	(eid, cid)
}
