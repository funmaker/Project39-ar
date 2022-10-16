use std::any::Any;
use rapier3d::dynamics::{ImpulseJoint, ImpulseJointHandle, ImpulseJointSet, RigidBody, RigidBodyHandle, RigidBodyType};
use rapier3d::geometry::{Collider, ColliderHandle, ColliderSet};
use rapier3d::parry::partitioning::IndexedData;
use egui::*;

use crate::Application;
use crate::utils::{from_user_data, InspectCollapsing, InspectObject};
use super::*;

impl SimpleInspect for RigidBodyType {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		
		ComboBox::from_id_source("Hand")
			.selected_text(match self {
				RigidBodyType::Fixed => "Fixed",
				RigidBodyType::Dynamic => "Dynamic",
				RigidBodyType::KinematicPositionBased => "Kinematic Pos",
				RigidBodyType::KinematicVelocityBased => "Kinematic Vel",
			})
			.show_ui(ui, |ui| {
				ui.selectable_value(self, RigidBodyType::Fixed, "Fixed");
				ui.selectable_value(self, RigidBodyType::Dynamic, "Dynamic");
				ui.selectable_value(self, RigidBodyType::KinematicPositionBased, "Kinematic Pos");
				ui.selectable_value(self, RigidBodyType::KinematicVelocityBased, "Kinematic Vel");
			});
	}
}

pub struct UserData(u128);

impl Inspect for UserData {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: Self::Options<'_>) {
		let (eid, cid) = from_user_data(self.0);
		
		if let Some(entity) = application.entity(eid) {
			if let Some(component) = entity.component_dyn(cid) {
				ui.inspect(&component.as_cref_dyn(), application);
			} else {
				ui.inspect(&entity.as_ref(), application);
			}
		} else {
			ui.label(RichText::new("Unknown").monospace().italics());
		}
	}
}


thread_local! {
	static JOINTS_CACHE: RefCell<Vec<ImpulseJointHandle>> = RefCell::new(Vec::new());
}

impl InspectMut for RigidBody {
	type Options<'a> = (RigidBodyHandle, &'a Application, &'a mut ColliderSet, &'a mut ImpulseJointSet);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (handle, application, collider_set, joints_set): Self::Options<'_>) {
		Grid::new("Rigid Body")
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				ui.inspect_row("ID", handle, application);
				ui.inspect_row("Owner", UserData(self.user_data), application);
				ui.inspect_row("Body Type", GetSet(|| (self.body_type(), |bt| self.set_body_type(bt))), ());
				ui.inspect_row("Sleeping", GetSet(|| (self.is_sleeping(), |sleep| if sleep { self.sleep() })), ());
			});
		
		if !self.colliders().is_empty() {
			CollapsingHeader::new(format!("Colliders ({})", self.colliders().len()))
				.id_source("Colliders")
				.show(ui, |ui| {
					for handle in self.colliders() {
						if let Some(col) = collider_set.get_mut(*handle) {
							ui.inspect_collapsing()
							  .show(ui, col, (*handle, application))
						} else {
							ui.label(RichText::new("Invalid").monospace().italics());
						}
					}
				});
		}
		
		JOINTS_CACHE.try_with(|joint_cache| {
			if let Ok(mut joint_cache) = joint_cache.try_borrow_mut() {
				joint_cache.clear();
				joint_cache.extend(joints_set.attached_joints(handle).map(|(_, _, handle, _)| handle));
				
				if !joint_cache.is_empty() {
					CollapsingHeader::new(format!("Joints ({})", joint_cache.len()))
						.id_source("Joints")
						.show(ui, |ui| {
							for handle in joint_cache.drain(..) {
								if let Some(col) = joints_set.get_mut(handle) {
									ui.inspect_collapsing()
									  .show(ui, col, (handle, application))
								} else {
									ui.label(RichText::new("Invalid").monospace().italics());
								}
							}
						});
				}
			} else {
				ui.label(RichText::new("Joint cache busy").italics());
			}
		}).expect("Joint cache failed");
	}
}

impl InspectObject for &mut RigidBody {
	fn is_selected(&self, (handle, application, _, _): &Self::Options<'_>) -> bool {
		application.get_selection().rigid_body() == *handle
	}

	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Rigid Body".into()
	}

	fn inspect_uid(&self, (handle, application, _, _): &Self::Options<'_>) -> u64 {
		handle.inspect_uid(application)
	}
}

impl Inspect for RigidBodyHandle {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		if ui.button(id_fmt(self.index(), "RB ")).clicked() {
			application.select(self);
		}
	}
}

impl InspectObject for RigidBodyHandle {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().rigid_body() == *self
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Rigid Body".into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		use std::hash::{Hash, Hasher};
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		self.type_id().hash(&mut s);
		self.hash(&mut s);
		s.finish()
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Ok(mut physics) = application.physics.try_borrow_mut() {
			let physics = &mut *physics;
			
			if let Some(rb) = physics.rigid_body_set.get_mut(self) {
				rb.show_collapsing((self, application, &mut physics.collider_set, &mut physics.impulse_joint_set), ui, collapsing);
				
				return;
			}
		}
		
		Grid::new(self.inspect_uid(&application))
			.min_col_width(100.0)
			.num_columns(2)
			.show(ui, |ui| {
				ui.inspect_row(collapsing.title.unwrap_or_else(|| self.inspect_header(&application)), self, application);
			});
	}
}



impl InspectMut for Collider {
	type Options<'a> = (ColliderHandle, &'a Application);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (handle, application): Self::Options<'_>) {
		Grid::new("Collider")
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				if let Some(parent) = self.parent() {
					ui.inspect_row("ID", handle, application);
					ui.inspect_row("Owner", UserData(self.user_data), application);
					ui.inspect_row("Rigid Body", parent, application);
					ui.inspect_row("Shape", format!("{:?}", self.shape().shape_type()), ());
				}
			});
	}
}

impl InspectObject for &mut Collider {
	fn is_selected(&self, (handle, application): &Self::Options<'_>) -> bool {
		application.get_selection().collider() == *handle
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Collider".into()
	}
	
	fn inspect_uid(&self, (handle, application): &Self::Options<'_>) -> u64 {
		handle.inspect_uid(application)
	}
}

impl Inspect for ColliderHandle {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		if ui.button(id_fmt(self.index(), "CO ")).clicked() {
			application.select(self);
		}
	}
}

impl InspectObject for ColliderHandle {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().collider() == *self
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Collider".into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		use std::hash::{Hash, Hasher};
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		self.type_id().hash(&mut s);
		self.hash(&mut s);
		s.finish()
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Ok(mut physics) = application.physics.try_borrow_mut() {
			let physics = &mut *physics;
			
			if let Some(col) = physics.collider_set.get_mut(self) {
				col.show_collapsing((self, application), ui, collapsing);
				
				return;
			}
		}
		
		Grid::new(self.inspect_uid(&application))
			.min_col_width(100.0)
			.num_columns(2)
			.show(ui, |ui| {
				ui.inspect_row(collapsing.title.unwrap_or_else(|| self.inspect_header(&application)), self, application);
			});
	}
}



impl InspectMut for ImpulseJoint {
	type Options<'a> = (ImpulseJointHandle, &'a Application);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (handle, application): Self::Options<'_>) {
		Grid::new("Impulse Joint")
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				ui.inspect_row("ID", handle, application);
				ui.inspect_row("Body 1", self.body1, application);
				ui.inspect_row("Frame 1", &mut self.data.local_frame1, ());
				ui.inspect_row("Body 2", self.body2, application);
				ui.inspect_row("Frame 2", &mut self.data.local_frame2, ());
			});
	}
}

impl InspectObject for &mut ImpulseJoint {
	fn is_selected(&self, (handle, application): &Self::Options<'_>) -> bool {
		application.get_selection().joint() == *handle
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Joint".into()
	}
	
	fn inspect_uid(&self, (handle, application): &Self::Options<'_>) -> u64 {
		handle.inspect_uid(application)
	}
}

impl Inspect for ImpulseJointHandle {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		if ui.button(id_fmt(self.0.index(), "IJ ")).clicked() {
			application.select(self);
		}
	}
}

impl InspectObject for ImpulseJointHandle {
	fn is_selected(&self, application: &Self::Options<'_>) -> bool {
		application.get_selection().joint() == *self
	}
	
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Impulse Joint".into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		use std::hash::{Hash, Hasher};
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		self.type_id().hash(&mut s);
		self.hash(&mut s);
		s.finish()
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Ok(mut physics) = application.physics.try_borrow_mut() {
			let physics = &mut *physics;
			
			if let Some(joint) = physics.impulse_joint_set.get_mut(self) {
				joint.show_collapsing((self, application), ui, collapsing);
				
				return;
			}
		}
		
		Grid::new(self.inspect_uid(&application))
			.min_col_width(100.0)
			.num_columns(2)
			.show(ui, |ui| {
				ui.inspect_row(collapsing.title.unwrap_or_else(|| self.inspect_header(&application)), self, application);
			});
	}
}
