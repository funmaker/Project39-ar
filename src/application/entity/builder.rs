use std::cell::{RefCell, Cell};
use std::collections::{BTreeMap, HashMap};
use std::any::Any;
use rapier3d::dynamics::{RigidBody, RigidBodyHandle, RigidBodyType};
use rapier3d::prelude::RigidBodyBuilder;
use rapier3d::geometry::Collider;

use crate::math::{Isometry3, Vec3, Point3, Rot3};
use crate::component::Component;
use super::Entity;
use crate::utils::next_uid;
use crate::application::entity::EntityState;
use crate::component::physics::collider::ColliderComponent;

pub struct EntityBuilder {
	pub name: String,
	pub rigid_body: RigidBody,
	pub position: Isometry3,
	pub velocity: Vec3,
	pub angular_velocity: Vec3,
	pub hidden: bool,
	pub components: Vec<Box<dyn Component>>,
	pub tags: HashMap<String, Box<dyn Any>>,
}

impl EntityBuilder {
	pub fn new(name: impl Into<String>) -> Self {
		EntityBuilder {
			name: name.into(),
			rigid_body: RigidBodyBuilder::new_static().build(),
			position: Isometry3::identity(),
			velocity: Vec3::zeros(),
			angular_velocity: Vec3::zeros(),
			hidden: false,
			components: vec![],
			tags: HashMap::new(),
		}
	}
	
	pub fn rigid_body(mut self, rigid_body: RigidBody) -> Self {
		self.rigid_body = rigid_body;
		self
	}
	
	pub fn rigid_body_type(mut self, rb_type: RigidBodyType) -> Self {
		self.rigid_body.set_body_type(rb_type);
		self
	}
	
	pub fn position(mut self, position: Isometry3) -> Self {
		self.position = position;
		self
	}
	
	pub fn translation(mut self, point: Point3) -> Self {
		self.position.translation = point.into();
		self
	}
	
	pub fn rotation(mut self, angle: Rot3) -> Self {
		self.position.rotation = angle;
		self
	}
	
	pub fn velocity(mut self, velocity: Vec3) -> Self {
		self.velocity = velocity;
		self
	}
	
	pub fn angular_velocity(mut self, angular_velocity: Vec3) -> Self {
		self.angular_velocity = angular_velocity;
		self
	}
	
	pub fn hidden(mut self, hidden: bool) -> Self {
		self.hidden = hidden;
		self
	}
	
	pub fn component<C: Component>(mut self, component: C) -> Self {
		self.components.push(component.boxed());
		self
	}
	
	pub fn collider(mut self, collider: Collider) -> Self {
		self.components.push(ColliderComponent::new(collider).boxed());
		self
	}
	
	pub fn tag<T: 'static>(mut self, key: impl Into<String>, val: T) -> Self {
		self.tags.insert(key.into(), Box::new(val));
		self
	}
	
	pub fn build(self) -> Entity {
		let entity = Entity {
			id: next_uid(),
			name: self.name,
			tags: RefCell::new(self.tags),
			state: RefCell::new(EntityState {
				position: self.position,
				velocity: self.velocity,
				angular_velocity: self.angular_velocity,
				hidden: self.hidden,
			}),
			removed: Cell::new(false),
			components: BTreeMap::new(),
			new_components: RefCell::new(self.components),
			rigid_body: RigidBodyHandle::invalid(),
			rigid_body_template: self.rigid_body,
		};
		
		entity
	}
}
