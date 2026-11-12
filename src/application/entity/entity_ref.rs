use std::cell::Cell;
use std::ops::{Deref, DerefMut};

use super::super::Application;
use super::Entity;


/// Weak, nullable reference to an entity by id.
///
/// Resolve it with `get`. Once the referenced entity no longer exists, the reference
/// nulls itself.
#[derive(Debug, Eq, PartialEq, Clone, Default)]
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
	
	/// Replaces the referenced entity.
	pub fn set(&self, other: Self) {
		self.inner.swap(&other.inner);
	}
	
	pub fn set_null(&self) {
		self.inner.set(None);
	}
	
	/// Resolves the reference. Returns `None` (and clears the reference) if the entity was removed.
	/// Entities that are still pending are not returned, but the reference stays valid.
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
	
	/// Returns the referenced id without checking whether the entity exists.
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
