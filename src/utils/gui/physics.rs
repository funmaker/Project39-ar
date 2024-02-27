use std::any::Any;
use egui::*;
use nalgebra::{Isometry3, Quaternion};
use rapier3d::dynamics::{ImpulseJoint, ImpulseJointHandle, ImpulseJointSet, JointAxis, RigidBody, RigidBodyHandle, RigidBodyType};
use rapier3d::geometry::{Collider, ColliderHandle, ColliderSet};
use rapier3d::parry::partitioning::IndexedData;
use rapier3d::prelude::JointAxesMask;

use crate::math::{PI, Rot3};
use crate::application::{Application, EntityRef};
use crate::utils::RigidBodyEx;
use super::super::{from_user_data, InspectCollapsing, InspectObject};
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
				ui.inspect_row("Body Type", GetSet(|| (
					self.body_type(),
					|bt| self.set_body_type(bt, true),
				)), ());
				ui.inspect_row("Sleeping", GetSet(|| (
					self.is_sleeping(),
					|sleep| if sleep { self.sleep() },
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
						*self.linvel(),
						|vel| self.set_linvel(vel, true),
					)), ());
					ui.inspect_row("Angular Velocity", GetSet(|| (
						*self.angvel(),
						|angvel| self.set_angvel(angvel, true),
					)), ());
					ui.inspect_row("Sleeping", GetSet(|| (
						self.is_sleeping(),
						|sleep| if sleep { self.sleep() } else { self.wake_up(true) },
					)), ());
				});
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
		if self == RigidBodyHandle::invalid() {
			ui.label(RichText::new("NULL").monospace().italics());
		} else {
			if ui.button(id_fmt(self.index(), "RB ")).clicked() {
				application.select(self);
			}
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
		use std::hash::Hasher;
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
		if self == ColliderHandle::invalid() {
			ui.label(RichText::new("NULL").monospace().italics());
		} else {
			if ui.button(id_fmt(self.index(), "CO ")).clicked() {
				application.select(self);
			}
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
		use std::hash::Hasher;
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
		if self == ImpulseJointHandle::invalid() {
			ui.label(RichText::new("NULL").monospace().italics());
		} else {
			if ui.button(id_fmt(self.0.index(), "IJ ")).clicked() {
				application.select(self);
			}
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
		use std::hash::Hasher;
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
				
				ui.separator();
				
				let (frame1, frame2) = match (physics.rigid_body_set.get(joint.body1), physics.rigid_body_set.get(joint.body2)) {
					(Some(body1), Some(body2)) => (body1.position() * joint.data.local_frame1, body2.position() * joint.data.local_frame2),
					_ => return
				};
				
				let entity = physics.rigid_body_set.get(joint.body1).map_or(EntityRef::null(), |rb| rb.entity_ref());
				
				Grid::new(self.inspect_uid(&application))
					.min_col_width(100.0)
					.num_columns(2)
					.show(ui, |ui| {
						{
						
						}
						
						ui.label("");
						ui.columns(6, |ui| {
							ui[0].label("MIN");
							ui[1].label("CUR");
							ui[2].label("MAX");
						});
						ui.end_row();
						
						let diff = frame1.inverse() * frame2;
						let ang_sgn = frame1.rotation.dot(&frame2.rotation).signum();
						let ang_err = diff.rotation.imag().scale(ang_sgn).map(f32::asin) * 2.0;
						
						let mut offsets = [
							diff.translation.x,
							diff.translation.y,
							diff.translation.z,
							ang_err.x,
							ang_err.y,
							ang_err.z,
						];
						let mut offset_changed = false;
						
						let mut inspect_axis = |axis: JointAxis| {
							ui.label(format!("{axis:?}"));
							ui.columns(7, |ui| {
								let limit = &mut joint.data.limits[axis as usize];
								let angle = JointAxesMask::ANG_AXES.contains(axis.into());
								let scale = if angle { 180.0 / PI } else { 1.0 };
								
								let mut min = limit.min * scale;
								let mut cur = offsets[axis as usize] * scale;
								let mut max = limit.max * scale;
								
								let locked = joint.data.locked_axes.contains(axis.into());
								let limited = joint.data.limit_axes.contains(axis.into());
								let coupled = joint.data.coupled_axes.contains(axis.into());
								
								if !min.is_finite() || min <= -10_000.0 {
									ui[0].label("NONE");
								} else if locked || !limited {
									ui[0].label(format!("{min:.3}"));
								} else if ui[0].add(DragValue::new(&mut min).speed(0.01 * scale)).changed() {
									limit.min = min / scale;
								}
								
								if ui[1].add(DragValue::new(&mut cur).speed(0.01 * scale)).changed() {
									if cur < min && limited { cur = min; }
									if cur > max && limited { cur = max; }
									offsets[axis as usize] = cur / scale;
									offset_changed = true;
								}
								
								if !max.is_finite() || max >= 10_000.0 {
									ui[2].label("NONE");
								} else if locked || !limited {
									ui[2].label(format!("{max:.3}"));
								} else if ui[2].add(DragValue::new(&mut max).speed(0.01 * scale)).changed() {
									limit.max = max / scale;
								}
								
								fn btn_col(active: bool) -> Color32 {
									if active { Color32::LIGHT_BLUE }
									else { Color32::GRAY }
								}
								
								if ui[3].add(Button::new(RichText::new("Lock").color(btn_col(locked)))).clicked() {
									joint.data.locked_axes.set(axis.into(), true);
									
									if angle && joint.data.coupled_axes.contains(axis.into()) {
										joint.data.coupled_axes.set(JointAxesMask::ANG_AXES, false);
									}
								}
								
								if ui[4].add(Button::new(RichText::new("Limit").color(btn_col(!locked && limited)))).clicked() {
									joint.data.locked_axes.set(axis.into(), false);
									joint.data.limit_axes.set(axis.into(), true);
									
									if angle && joint.data.coupled_axes.contains(axis.into()) {
										joint.data.coupled_axes.set(JointAxesMask::ANG_AXES, false);
									}
									
									if limit.min < -10_000.0 || limit.max > 10_000.0 {
										if angle {
											limit.min = -PI / 4.0;
											limit.max = PI / 4.0;
										} else {
											limit.min = cur - 1.0;
											limit.max = cur + 1.0;
										}
									}
								}
								
								if ui[5].add(Button::new(RichText::new("Free").color(btn_col(!locked && !limited)))).clicked() {
									joint.data.locked_axes.set(axis.into(), false);
									joint.data.limit_axes.set(axis.into(), false);
								}
								
								if ui[6].add(Button::new(RichText::new("Couple").color(btn_col(coupled)))).clicked() {
									if angle {
										let free_angles = (joint.data.coupled_axes & JointAxesMask::ANG_AXES).is_empty();
										
										if free_angles {
											let coupled = JointAxesMask::ANG_AXES.difference(axis.into());
											
											joint.data.coupled_axes.set(coupled, true);
											joint.data.limit_axes.set(coupled, true);
											joint.data.locked_axes.set(coupled, false);
											
											for axis in [JointAxis::AngX, JointAxis::AngY, JointAxis::AngZ] {
												if !coupled.contains(axis.into()) { continue; }
												
												let limit = &mut joint.data.limits[axis as usize];
												if limit.max > 10_000.0 {
													limit.max = PI / 4.0;
												}
												
												limit.min = limit.min.max(0.0);
											}
										} else {
											joint.data.coupled_axes.set(JointAxesMask::ANG_AXES, false);
										}
									} else {
										joint.data.coupled_axes.set(axis.into(), !coupled);
									}
								}
							});
							ui.end_row();
						};
						
						inspect_axis(JointAxis::X);
						inspect_axis(JointAxis::Y);
						inspect_axis(JointAxis::Z);
						
						inspect_axis(JointAxis::AngX);
						inspect_axis(JointAxis::AngY);
						inspect_axis(JointAxis::AngZ);
						
						if offset_changed {
							if let Some(entity) = entity.get(application) {
								let i = (offsets[3] / 2.0).sin() * ang_sgn;
								let j = (offsets[4] / 2.0).sin() * ang_sgn;
								let k = (offsets[5] / 2.0).sin() * ang_sgn;
								let w = (1.0 - i * i - j * j - k * k).max(0.0).sqrt();
								
								let new_diff = Isometry3::from_parts(
									vector![offsets[0], offsets[1], offsets[2]].into(),
									Rot3::new_normalize(Quaternion::new(w, i, j, k)),
								);
							
								*entity.state_mut().position = frame2 * new_diff.inverse() * joint.data.local_frame1.inverse();
							}
						}
					});
				
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
