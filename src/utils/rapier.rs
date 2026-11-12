use rapier3d::dynamics::{ImpulseJoint, RigidBody};
use rapier3d::geometry::Collider;

use crate::application::{Application, Entity, EntityRef};
use crate::component::ComponentRef;
use crate::component::physics::collider::ColliderComponent;


/// Access to the component that owns a Rapier object (collider or joint), through its user data.
#[allow(dead_code)]
pub trait PhysicsComponentEx {
	/// Returns a reference to the owning component.
	fn component_ref(&self) -> ComponentRef<ColliderComponent>;
	
	/// Returns a reference to the entity of the owning component.
	fn entity_ref(&self) -> EntityRef {
		self.component_ref().entity()
	}
	
	/// Returns the owning component. Panics if it doesn't exist.
	fn component<'a>(&self, application: &'a Application) -> &'a ColliderComponent {
		self.component_ref().get(application).unwrap()
	}
	
	/// Returns the entity of the owning component. Panics if it doesn't exist.
	fn entity<'a>(&self, application: &'a Application) -> &'a Entity {
		self.component_ref().entity().get(application).unwrap()
	}
}

impl PhysicsComponentEx for Collider {
	fn component_ref(&self) -> ComponentRef<ColliderComponent> {
		let (eid, cid) = from_user_data(self.user_data);
		
		ComponentRef::new(eid, cid)
	}
}

impl PhysicsComponentEx for ImpulseJoint {
	fn component_ref(&self) -> ComponentRef<ColliderComponent> {
		let (eid, cid) = from_user_data(self.data.user_data);
		
		ComponentRef::new(eid, cid)
	}
}

/// Access to the entity that owns a Rapier rigid body, through its user data.
#[allow(dead_code)]
pub trait PhysicsEntityEx {
	fn entity_ref(&self) -> EntityRef;
	
	/// Returns the owning entity. Panics if it doesn't exist.
	fn entity<'a>(&self, application: &'a Application) -> &'a Entity {
		self.entity_ref().get(application).unwrap()
	}
}

impl PhysicsEntityEx for RigidBody {
	fn entity_ref(&self) -> EntityRef {
		let (eid, _) = from_user_data(self.user_data);
		
		EntityRef::new(eid)
	}
}

/// Packs entity and component ids into Rapier user data.
pub fn get_user_data(eid: u64, cid: u64) -> u128 {
	eid as u128 + ((cid as u128) << 64)
}

/// Unpacks `(entity id, component id)` from Rapier user data.
pub fn from_user_data(userdata: u128) -> (u64, u64) {
	let eid = (userdata % (1 << 64)) as u64;
	let cid = (userdata >> 64) as u64;
	(eid, cid)
}
