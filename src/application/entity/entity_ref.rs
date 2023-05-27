use std::cell::Cell;
use std::ops::{Deref, DerefMut};

use crate::application::Application;
use super::Entity;


#[derive(Debug, Eq, PartialEq, Clone)]
pub struct EntityRef {
	inner: Cell<Option<u64>>,
}

impl EntityRef {
	pub fn new(eid: u64) -> Self {
		EntityRef {
			inner: Cell::new(Some(eid)),
		}
	}
	
	pub fn null() -> Self {
		EntityRef {
			inner: Cell::new(None),
		}
	}
	
	pub fn set(&self, other: Self) {
		self.inner.swap(&other.inner);
	}
	
	pub fn set_null(&self) {
		self.inner.set(None);
	}
	
	pub fn get<'a>(&self, application: &'a Application) -> Option<&'a Entity> {
		if let Some(eid) = self.inner.get() {
			if let Some(entity) = application.entity(eid) {
				return Some(entity);
			} else if !application.pending_entity(eid) {
				self.inner.set(None);
			}
		}
		
		None
	}
	
	pub fn raw(&self) -> Option<u64> {
		self.inner.get()
	}
}

impl PartialEq<EntityRef> for Entity {
	fn eq(&self, other: &EntityRef) -> bool {
		if let Some(id) = other.inner.get() {
			self.id == id
		} else {
			false
		}
	}
}

impl PartialEq<EntityRef> for &Entity {
	fn eq(&self, other: &EntityRef) -> bool {
		(*self).eq(other)
	}
}

impl PartialEq<Entity> for EntityRef {
	fn eq(&self, other: &Entity) -> bool {
		other.eq(self)
	}
}

impl PartialEq<&Entity> for EntityRef {
	fn eq(&self, other: &&Entity) -> bool {
		(*other).eq(self)
	}
}

impl From<&Entity> for EntityRef {
	fn from(entity: &Entity) -> Self {
		EntityRef::new(entity.id)
	}
}

impl From<&EntityRef> for EntityRef {
	fn from(entity_ref: &EntityRef) -> Self {
		entity_ref.clone()
	}
}
