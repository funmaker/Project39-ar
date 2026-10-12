use std::any::Any;
use rapier3d::dynamics::{ImpulseJoint, ImpulseJointHandle, ImpulseJointSet, RigidBody, RigidBodyHandle, RigidBodyType};
use rapier3d::geometry::{Collider, ColliderHandle, Group, InteractionTestMode};

use crate::application::{select_button, Application};
use super::super::from_user_data;
use super::*;


impl SimpleInspect for RigidBodyType {
	fn inspect_ui(&mut self, ui: &mut Ui) {
		
		ComboBox::from_id_source("RigidBodyType")
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

impl InspectMut for RigidBody {
	type Options<'a> = (RigidBodyHandle, &'a Application, &'a ImpulseJointSet);
	
	fn inspect_ui(&mut self, ui: &mut Ui, (handle, application, joints_set): Self::Options<'_>) {
		Grid::new("Rigid Body")
			.num_columns(2)
			.min_col_width(100.0)
			.show(ui, |ui| {
				ui.inspect_row("ID", handle, application);
				ui.inspect_row("Owner", UserData(self.user_data), application);
				ui.inspect_row("Body Type", GetSet(|| (
					self.body_type(),
					|bt| self.set_body_type(bt, true),
				)), ());
				ui.inspect_row("Sleeping", GetSet(|| (
					self.is_sleeping(),
					|sleep| if sleep { self.sleep() } else { self.wake_up(true) },
				)), ());
			});
		
		ui.collapsing("Kinematics", |ui| {
			Grid::new("Rigid Body")
				.num_columns(2)
				.min_col_width(100.0)
				.show(ui, |ui| {
					ui.inspect_row("Position", GetSet(|| (
						*self.position(),
						|pos| self.set_position(pos, true),
					)), ());
					ui.inspect_row("Velocity", GetSet(|| (
						self.linvel(),
						|vel| self.set_linvel(vel, true),
					)), ());
					ui.inspect_row("Angular Velocity", GetSet(|| (
						self.angvel(),
						|angvel| self.set_angvel(angvel, true),
					)), ());
				});
		});
		
		if !self.colliders().is_empty() {
			CollapsingHeader::new(format!("Colliders ({})", self.colliders().len()))
				.id_source("Colliders")
				.show(ui, |ui| {
					Grid::new("Colliders Grid")
						.num_columns(2)
						.min_col_width(100.0)
						.show(ui, |ui| {
							for handle in self.colliders() {
								ui.inspect_row(id_fmt(handle.into_raw_parts().0, "CO "), *handle, application);
							}
						});
				});
		}
		
		let joints = joints_set.attached_joints(handle).count();
		if joints > 0 {
			CollapsingHeader::new(format!("Joints ({})", joints))
				.id_source("Joints")
				.show(ui, |ui| {
					Grid::new("Colliders Grid")
						.num_columns(2)
						.min_col_width(100.0)
						.show(ui, |ui| {
							for (_, _, handle, _) in joints_set.attached_joints(handle) {
								ui.inspect_row(id_fmt(handle.into_raw_parts().0, "IJ "), handle, application);
							}
						});
				});
		}
	}
}

impl InspectObject for &mut RigidBody {
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Rigid Body".into()
	}
	
	fn inspect_uid(&self, (handle, application, _): &Self::Options<'_>) -> u64 {
		handle.inspect_uid(application)
	}
	
	fn selection_level(&self, (handle, application, _): &Self::Options<'_>) -> SelectionLevel {
		application.gui_selections_test(*handle, true)
	}
}

impl Inspect for RigidBodyHandle {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		select_button(ui, self, application);
	}
}

impl InspectObject for RigidBodyHandle {
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Rigid Body".into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		use std::hash::Hasher;
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		self.type_id().hash(&mut s);
		self.hash(&mut s);
		s.finish()
	}
	
	fn selection_level(&self, application: &Self::Options<'_>) -> SelectionLevel {
		application.gui_selections_test(*self, true)
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Ok(mut physics) = application.physics.try_borrow_mut() {
			let physics = &mut *physics;
			
			if let Some(rb) = physics.bodies.get_mut(self) {
				rb.show_collapsing((self, application, &mut physics.impulse_joints), ui, collapsing);
				
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
				ui.inspect_row("ID", handle, application);
				ui.inspect_row("Owner", UserData(self.user_data), application);
				ui.inspect_row("Rigid Body", self.parent().unwrap_or(RigidBodyHandle::invalid()), application);
				
				if let Some(&pos_wrt_parent) = self.position_wrt_parent() {
					ui.inspect_row("Position", GetSet(|| (
						pos_wrt_parent,
						|pos| self.set_position_wrt_parent(pos),
					)), ());
				} else {
					ui.inspect_row("Position", GetSet(|| (
						*self.position(),
						|pos| self.set_position(pos),
					)), ());
				}
				
				ui.inspect_row("Shape", format!("{:?}", self.shape().shape_type()), ());
				
				if let Some(ball) = self.shape_mut().as_ball_mut() {
					ui.inspect_row("Radius", &mut ball.radius, (0.1, 0.01..=f32::INFINITY));
				} else if let Some(cuboid) = self.shape_mut().as_cuboid_mut() {
					ui.inspect_row("Half Extends", &mut cuboid.half_extents, ());
				} else if let Some(capsule) = self.shape_mut().as_capsule_mut() {
					ui.inspect_row("Radius", &mut capsule.radius, (0.1, 0.01..=f32::INFINITY));
					ui.inspect_row("Start", &mut capsule.segment.a, ());
					ui.inspect_row("End", &mut capsule.segment.b, ());
				} else if let Some(cylinder) = self.shape_mut().as_cylinder_mut() {
					ui.inspect_row("Radius", &mut cylinder.radius, (0.1, 0.01..=f32::INFINITY));
					ui.inspect_row("Half Height", &mut cylinder.half_height, (0.1, 0.01..=f32::INFINITY));
				}
				
				let mut collisions = self.collision_groups();
				
				ui.label("Collisions");
				if let Some(test_mode) = match collisions.test_mode { // :^)
					InteractionTestMode::And if ui.button("And").clicked() => Some(InteractionTestMode::Or),
					InteractionTestMode::Or if ui.button("Or").clicked() => Some(InteractionTestMode::And),
					_ => None,
				} {
					collisions.test_mode = test_mode;
					self.set_collision_groups(collisions);
				}
				ui.end_row();
				
				ui.label("Membership");
				ui.horizontal_top(|ui| {
					for col in (0..8).rev() {
						ui.vertical(|ui| {
							for pos in (col..32).step_by(8).rev() {
								let flag = Group::from(1 << pos);
								if ui.toggle_value(&mut collisions.memberships.contains(flag), format!("{pos}")).changed() {
									collisions.memberships.toggle(flag);
									self.set_collision_groups(collisions);
								}
							}
						});
					}
				});
				ui.end_row();
				
				ui.label("Filter");
				ui.horizontal_top(|ui| {
					for col in (0..8).rev() {
						ui.vertical(|ui| {
							for pos in (col..32).step_by(8).rev() {
								let flag = Group::from(1 << pos);
								if ui.toggle_value(&mut collisions.filter.contains(flag), format!("{pos}")).changed() {
									collisions.filter.toggle(flag);
									self.set_collision_groups(collisions);
								}
							}
						});
					}
				});
				ui.end_row();
			});
	}
}

impl InspectObject for &mut Collider {
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Collider".into()
	}
	
	fn inspect_uid(&self, (handle, application): &Self::Options<'_>) -> u64 {
		handle.inspect_uid(application)
	}
	
	fn selection_level(&self, (handle, application): &Self::Options<'_>) -> SelectionLevel {
		application.gui_selections_test(*handle, true)
	}
}

impl Inspect for ColliderHandle {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		select_button(ui, self, application);
	}
}

impl InspectObject for ColliderHandle {
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Collider".into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		use std::hash::Hasher;
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		self.type_id().hash(&mut s);
		self.hash(&mut s);
		s.finish()
	}
	
	fn selection_level(&self, application: &Self::Options<'_>) -> SelectionLevel {
		application.gui_selections_test(*self, true)
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Ok(mut physics) = application.physics.try_borrow_mut() {
			let physics = &mut *physics;
			
			if let Some(col) = physics.colliders.get_mut(self) {
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
				ui.inspect_row("Body 1", self.body1(), application);
				ui.inspect_row("Frame 1", &mut self.data.local_frame1, ());
				ui.inspect_row("Body 2", self.body2(), application);
				ui.inspect_row("Frame 2", &mut self.data.local_frame2, ());
			});
	}
}

impl InspectObject for &mut ImpulseJoint {
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Joint".into()
	}
	
	fn inspect_uid(&self, (handle, application): &Self::Options<'_>) -> u64 {
		handle.inspect_uid(application)
	}
	
	fn selection_level(&self, (handle, application): &Self::Options<'_>) -> SelectionLevel {
		application.gui_selections_test(*handle, true)
	}
}

impl Inspect for ImpulseJointHandle {
	type Options<'a> = &'a Application;
	
	fn inspect_ui(self, ui: &mut Ui, application: &Application) {
		select_button(ui, self, application);
	}
}

impl InspectObject for ImpulseJointHandle {
	fn inspect_header(&self, _options: &Self::Options<'_>) -> WidgetText {
		"Impulse Joint".into()
	}
	
	fn inspect_uid(&self, _options: &Self::Options<'_>) -> u64 {
		use std::hash::Hasher;
		use std::collections::hash_map::DefaultHasher;
		
		let mut s = DefaultHasher::new();
		self.type_id().hash(&mut s);
		self.hash(&mut s);
		s.finish()
	}
	
	fn selection_level(&self, application: &Self::Options<'_>) -> SelectionLevel {
		application.gui_selections_test(*self, true)
	}
	
	fn show_collapsing(self, application: Self::Options<'_>, ui: &mut Ui, collapsing: InspectCollapsing) {
		if let Ok(mut physics) = application.physics.try_borrow_mut() {
			let physics = &mut *physics;
			
			if let Some(col) = physics.impulse_joints.get_mut(self, false) {
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
