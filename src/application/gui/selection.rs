use std::process::id;
use egui::{Button, Color32, RichText, Ui};
use rapier3d::dynamics::{ImpulseJointHandle, RigidBodyHandle};
use rapier3d::geometry::ColliderHandle;
use rapier3d::pipeline::PhysicsWorld;
use crate::component::{Component, ComponentBase, ComponentRef};
use crate::component::model::MMDModel;
use crate::component::model::mmd::MMDBone;
use crate::utils::{id_fmt, PhysicsComponentEx, PhysicsEntityEx, ExUi};
use super::super::{Application, Entity, EntityRef};


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

impl Default for GuiTab {
	fn default() -> Self {
		GuiTab::Main
	}
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GuiSelections {
	pub hover: GuiSelection,
	pub selection: GuiSelection,
	pub focus: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GuiSelection {
	None,
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
			GuiSelection::None |
			GuiSelection::Entity(_) |
			GuiSelection::Component(_) |
			GuiSelection::RigidBody(_) |
			GuiSelection::ImpulseJoint(_) |
			GuiSelection::Collider(_) => GuiTab::Main,
			GuiSelection::MMDBone(_, _) => GuiTab::Miku,
		}
	}
	
	pub fn entity(&self, physics: &PhysicsWorld) -> EntityRef {
		match self {
			GuiSelection::Entity(eref) => eref.clone(),
			GuiSelection::Component(cref) => cref.entity(),
			GuiSelection::RigidBody(rbref) if let Some(rb) = physics.bodies.get(*rbref) => rb.entity_ref(),
			GuiSelection::Collider(cref) if let Some(rb) = physics.colliders.get(*cref) => rb.entity_ref(),
			GuiSelection::ImpulseJoint(jref) if let Some(rb) = physics.impulse_joints.get(*jref) => rb.entity_ref(),
			GuiSelection::MMDBone(cref, _) => cref.entity(),
			_ => EntityRef::null(),
		}
	}
	
	pub fn component(&self, physics: &PhysicsWorld) -> ComponentRef<dyn Component> {
		match self {
			GuiSelection::Component(cref) => cref.clone(),
			GuiSelection::Collider(cref) if let Some(rb) = physics.colliders.get(*cref) => rb.component_ref().into(),
			GuiSelection::ImpulseJoint(jref) if let Some(rb) = physics.impulse_joints.get(*jref) => rb.component_ref().into(),
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
	
	pub fn mmd_model(&self) -> ComponentRef<MMDModel> {
		match self {
			GuiSelection::MMDBone(model, _) => model.clone(),
			_ => ComponentRef::null(),
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
		GuiSelection::None
	}
}

pub fn select_button(ui: &mut Ui, target: impl Into<GuiSelection>, application: &Application) {
	let target = target.into();
	
	let label = match &target {
		GuiSelection::Entity(eref)        if let Some(entity) = eref.get(application)                                                         => Some(id_fmt(entity.id, "")),
		GuiSelection::Component(cref)     if let Some(component) = cref.get_dyn(application)                                                  => Some(id_fmt(component.id(), "")),
		GuiSelection::RigidBody(rb)       if application.physics.try_borrow().ok().is_none_or(|physics| physics.bodies.contains(*rb))         => Some(id_fmt(rb.into_raw_parts().0, "RB ")),
		GuiSelection::Collider(col)       if application.physics.try_borrow().ok().is_none_or(|physics| physics.colliders.contains(*col))     => Some(id_fmt(col.into_raw_parts().0, "CO ")),
		GuiSelection::ImpulseJoint(ij)    if application.physics.try_borrow().ok().is_none_or(|physics| physics.impulse_joints.contains(*ij)) => Some(id_fmt(ij.into_raw_parts().0, "IJ ")),
		GuiSelection::MMDBone(cref, bone) if let Some(component) = cref.get(application) && component.state().bones.len() < *bone             => Some(id_fmt(bone, "MB ")),
		_ => None,
	};
	
	if let Some(mut label) = label {
		let selected = application.gui_selections().selection == target;
		
		if selected {
			label = label.color(Color32::LIGHT_BLUE);
		}
		
		let response = ui.button(label);
		if response.double_clicked() {
			application.select(target.clone(), true);
		} else if response.clicked() {
			application.select(target.clone(), false);
		}
		
		let hovered = ui.track_hover(&response);
		if response.hovered() && !hovered {
			application.gui_selections_mut().hover = target;
		} else if !response.hovered() && hovered {
			application.gui_selections_mut().hover = GuiSelection::None;
		}
	} else {
		ui.label(RichText::new("NULL").monospace().italics());
	}
}
