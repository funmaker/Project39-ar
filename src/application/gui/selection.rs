use rapier3d::dynamics::{ImpulseJointHandle, RigidBodyHandle};
use rapier3d::geometry::ColliderHandle;

use crate::component::{Component, ComponentBase, ComponentRef};
use crate::component::model::MMDModel;
use crate::component::model::mmd::MMDBone;
use super::super::{Entity, EntityRef};


#[derive(Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Copy, Clone)]
pub enum GuiTab {
	Main,
	Miku,
	Physics,
	Benchmark,
	Settings,
	Inspector,
	Memory,
}

impl GuiTab {
	pub fn label(self) -> &'static str {
		match self {
			GuiTab::Main => "Project 39",
			GuiTab::Miku => "Miku",
			GuiTab::Physics => "Physics",
			GuiTab::Benchmark => "Benchmark",
			GuiTab::Settings => "UI Settings",
			GuiTab::Inspector => "UI Inspector",
			GuiTab::Memory => "UI Memory",
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuiSelection {
	Tab(GuiTab),
	Entity(EntityRef),
	Component(ComponentRef<dyn Component>),
	RigidBody(RigidBodyHandle),
	Collider(ColliderHandle),
	ImpulseJoint(ImpulseJointHandle),
	MMDBone(ComponentRef<MMDModel>, usize),
}

impl GuiSelection {
	pub fn tab(&self) -> GuiTab {
		match self {
			GuiSelection::Tab(tab) => *tab,
			GuiSelection::Entity(_) |
			GuiSelection::Component(_) => GuiTab::Main,
			GuiSelection::RigidBody(_) |
			GuiSelection::Collider(_) |
			GuiSelection::ImpulseJoint(_) => GuiTab::Physics,
			GuiSelection::MMDBone(_, _) => GuiTab::Miku,
		}
	}
	
	pub fn entity(&self) -> EntityRef {
		match self {
			GuiSelection::Entity(eref) => eref.clone(),
			_ => EntityRef::null(),
		}
	}
	
	pub fn entity_or_component(&self) -> EntityRef {
		match self {
			GuiSelection::Entity(eref) => eref.clone(),
			GuiSelection::Component(cref) => cref.entity(),
			GuiSelection::MMDBone(cref, _) => cref.entity(),
			_ => EntityRef::null(),
		}
	}
	
	pub fn component(&self) -> ComponentRef<dyn Component> {
		match self {
			GuiSelection::Component(cref) => cref.clone(),
			GuiSelection::MMDBone(cref, _) => cref.clone().into(),
			_ => ComponentRef::null(),
		}
	}
	
	pub fn rigid_body(&self) -> RigidBodyHandle {
		match self {
			GuiSelection::RigidBody(rb) => rb.clone(),
			_ => RigidBodyHandle::invalid(),
		}
	}
	
	pub fn collider(&self) -> ColliderHandle {
		match self {
			GuiSelection::Collider(col) => col.clone(),
			_ => ColliderHandle::invalid(),
		}
	}
	
	pub fn joint(&self) -> ImpulseJointHandle {
		match self {
			GuiSelection::ImpulseJoint(joint) => joint.clone(),
			_ => ImpulseJointHandle::invalid(),
		}
	}
	
	pub fn mmd_bone(&self) -> Option<usize> {
		match self {
			GuiSelection::MMDBone(_, bone) => Some(*bone),
			_ => None,
		}
	}
}

macro_rules! impl_from {
	( $(
		$variant:ident from $type:ty;
	)* ) => { $(
		impl From<$type> for GuiSelection {
			fn from(value: $type) -> Self {
				Self::$variant(value)
			}
		}
	)*}
}

impl_from!{
	Tab from GuiTab;
	Entity from EntityRef;
	RigidBody from RigidBodyHandle;
	Collider from ColliderHandle;
	ImpulseJoint from ImpulseJointHandle;
}

impl<C: ?Sized + 'static> From<&ComponentRef<C>> for GuiSelection {
	fn from(cref: &ComponentRef<C>) -> Self {
		if let Some((eid, cid)) = cref.inner() {
			Self::Component(ComponentRef::new(eid, cid))
		} else {
			Self::Component(ComponentRef::null())
		}
	}
}

impl From<&Entity> for GuiSelection {
	fn from(entity: &Entity) -> Self {
		entity.as_ref().into()
	}
}

impl From<&EntityRef> for GuiSelection {
	fn from(eref: &EntityRef) -> Self {
		Self::Entity(eref.clone())
	}
}

impl From<&MMDBone> for GuiSelection {
	fn from(bone: &MMDBone) -> Self {
		Self::MMDBone(bone.model.clone(), bone.id)
	}
}

impl From<(ComponentRef<MMDModel>, usize)> for GuiSelection {
	fn from((model, bone): (ComponentRef<MMDModel>, usize)) -> Self {
		Self::MMDBone(model, bone)
	}
}

impl<C: ComponentBase> From<&C> for GuiSelection {
	fn from(component: &C) -> Self {
		(&component.as_cref()).into()
	}
}

impl Default for GuiSelection {
	fn default() -> Self {
		Self::Tab(GuiTab::Main)
	}
}

