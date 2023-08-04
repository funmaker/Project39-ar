use std::any::Any;
use std::cell::{RefCell, Cell};
use std::collections::HashMap;
use rapier3d::dynamics::{RigidBody, RigidBodyHandle, RigidBodyType};
use rapier3d::geometry::{Collider, ColliderBuilder};
use rapier3d::prelude::RigidBodyBuilder;

use crate::application::EntityRef;
use crate::application::entity::EntityState;
use crate::component::Component;
use crate::component::model::SimpleModel;
use crate::component::physics::collider::ColliderComponent;
use crate::math::{Isometry3, Vec3, Point3, Rot3};
use crate::utils::{MutMark, next_uid};
use super::Entity;


pub struct EntityBuilder {
	pub name: String,
	pub parent: EntityRef,
	pub parent_follow: bool,
	pub persist: bool,
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
			parent: EntityRef::null(),
			parent_follow: false,
			persist: false,
			rigid_body: RigidBodyBuilder::fixed().build(),
			position: Isometry3::identity(),
			velocity: Vec3::zeros(),
			angular_velocity: Vec3::zeros(),
			hidden: false,
			components: vec![],
			tags: HashMap::new(),
		}
	}
	
	pub fn parent(mut self, parent: EntityRef, follow: bool) -> Self {
		self.parent = parent;
		self.parent_follow = follow;
		self
	}
	
	pub fn persist(mut self) -> Self {
		self.persist = true;
		self
	}
	
	pub fn rigid_body(mut self, rigid_body: RigidBody) -> Self {
		self.rigid_body = rigid_body;
		self
	}
	
	pub fn rigid_body_type(mut self, rb_type: RigidBodyType) -> Self {
		self.rigid_body.set_body_type(rb_type, false);
		self
	}
	
	pub fn gravity_scale(mut self, scale: f32) -> Self {
		self.rigid_body.set_gravity_scale(scale, false);
		self
	}
	
	pub fn damping(mut self, linear: f32, angular: f32) -> Self {
		self.rigid_body.set_linear_damping(linear);
		self.rigid_body.set_angular_damping(angular);
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
	
	pub fn collider_from_aabb(self, density: f32) -> Self {
		for component in &self.components {
			if let Some(model) = component.as_any().downcast_ref::<SimpleModel>() {
				let aabb = model.aabb();
				let hsize = aabb.half_extents();
				
				return self.collider(ColliderBuilder::cuboid(hsize.x, hsize.y, hsize.z)
				           .translation(aabb.center().coords)
				           .density(density)
				           .build());
			}
		}
		
		eprintln!("Unable to create collider from aabb without SimpleModel component! ({})", self.name);
		
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
			parent: self.parent,
			parent_offset: Cell::new(self.parent_follow.then_some(Isometry3::identity())), // will be set on Entity::initialize
			children: RefCell::new(vec![]),
			tags: RefCell::new(self.tags),
			state: RefCell::new(EntityState {
				position: MutMark::new(self.position),
				velocity: MutMark::new(self.velocity),
				angular_velocity: MutMark::new(self.angular_velocity),
				hidden: self.hidden,
			}),
			initialized: Cell::new(false),
			removed: Cell::new(false),
			persist: Cell::new(self.persist),
			frozen: Cell::new(false),
			components: HashMap::new(),
			new_components: RefCell::new(self.components),
			rigid_body: RigidBodyHandle::invalid(),
			rigid_body_template: self.rigid_body,
		};
		
		entity
	}
}
