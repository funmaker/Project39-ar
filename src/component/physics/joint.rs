use std::cell::{Cell, RefCell};
use std::f32::consts::PI;
use anyhow::Result;
use egui::{Button, Color32, DragValue, RichText, Ui};
use nalgebra::Quaternion;
use rapier3d::dynamics::{GenericJoint, ImpulseJoint, ImpulseJointHandle, JointAxesMask, JointAxis};
use rapier3d::pipeline::PhysicsWorld;

use crate::application::{Entity, Application, EntityRef};
use crate::component::model::mmd::shared::{ConeTwistDesc, JointDesc};
use crate::math::{from_euler, Isometry3, Rot3};
use crate::utils::{get_user_data, ExUi};
use super::super::{Component, ComponentBase, ComponentInner};

/// Impulse joint between the entity's rigid body and the `target` entity's rigid body, existing
/// for the lifetime of the component.
#[derive(ComponentBase)]
pub struct JointComponent {
	#[inner] inner: ComponentInner,
	pub name: String,
	/// Original MMD joint description, if created by `from_mmd`. Enables the joint limits editor in the inspector.
	mmd_desc: Option<RefCell<MMDDesc>>,
	/// Joint inserted into the physics world on start.
	template: GenericJoint,
	target: EntityRef,
	handle: Cell<ImpulseJointHandle>,
}

impl JointComponent {
	/// Creates a joint between the entity and `target`.
	pub fn new(joint: impl Into<GenericJoint>, target: impl Into<EntityRef>) -> Self {
		JointComponent {
			inner: ComponentInner::new_norender(),
			name: "Joint".into(),
			mmd_desc: None,
			template: joint.into(),
			target: target.into(),
			handle: Cell::new(ImpulseJointHandle::invalid()),
		}
	}
	
	/// Creates a joint from an MMD joint description, with the given joint frames relative to the
	/// entity and `target` bodies.
	pub fn from_mmd(local_frame1: Isometry3, local_frame2: Isometry3,
	                joint_desc: JointDesc,
	                target: impl Into<EntityRef>)
	                -> Self {
		let mmd_desc = MMDDesc::new(joint_desc, local_frame1, local_frame2);
		let mut component = Self::new(mmd_desc.joint(), target);
		
		component.mmd_desc = Some(RefCell::new(mmd_desc));
		
		component
	}
	
	/// Sets the name shown in the inspector.
	pub fn named(mut self, name: impl Into<String>) -> Self {
		self.name = name.into();
		self
	}
	
	/// Returns the other entity of the joint.
	pub fn other<'a>(&self, application: &'a Application) -> Option<&'a Entity> {
		self.target.get(application)
	}
	
	/// Returns the handle of the joint in the physics world. Invalid before `start`.
	pub fn handle(&self) -> ImpulseJointHandle {
		self.handle.get()
	}
	
	/// Returns the Rapier joint. Panics if the component wasn't started.
	pub fn inner<'p>(&self, physics: &'p PhysicsWorld) -> &'p ImpulseJoint {
		physics.impulse_joints.get(self.handle.get()).unwrap()
	}
	
	/// Returns the Rapier joint mutably. Panics if the component wasn't started.
	pub fn inner_mut<'p>(&self, physics: &'p mut PhysicsWorld, wake_up: bool) -> &'p mut ImpulseJoint {
		physics.impulse_joints.get_mut(self.handle.get(), wake_up).unwrap()
	}
}

impl Component for JointComponent {
	fn start(&self, entity: &Entity, application: &Application) -> Result<()> {
		let physics = &mut application.physics.borrow_mut();
		
		if let Some(target) = self.target.get(application) {
			let mut joint = self.template.clone();
			joint.user_data = get_user_data(entity.id, self.id());
			self.handle.set(physics.insert_impulse_joint(entity.rigid_body, target.rigid_body, joint));
		}
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<()> {
		let physics = &mut application.physics.borrow_mut();
		
		physics.remove_impulse_joint(self.handle.get());
		
		Ok(())
	}
	
	fn on_inspect(&self, entity: &Entity, ui: &mut Ui, application: &Application) {
		if let Some(mmd_desc) = self.mmd_desc.as_ref() {
			if let Some(other) = self.other(application) {
				if let Ok(mut physics) = application.physics.try_borrow_mut() {
					if let Some(joint) = physics.impulse_joints.get_mut(self.handle.get(), false) {
						mmd_desc.borrow_mut()
						        .on_inspect(joint, entity, other, application, ui);
					}
				}
			}
		}
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_collapsing()
		  .title("Joint")
		  .show(ui, self.handle.get(), application);
	}
}

/// MMD joint description of a joint, kept to allow editing its limits at runtime.
struct MMDDesc {
	joint_desc: JointDesc,
	local_frame1: Isometry3,
	local_frame2: Isometry3,
}

impl MMDDesc {
	fn new(joint_desc: JointDesc, local_frame1: Isometry3, local_frame2: Isometry3) -> Self {
		Self { joint_desc, local_frame1, local_frame2 }
	}
	
	/// Draws an editor of the joint's per-axis limits and current offsets. Editing the offsets moves
	/// the first entity (and its descendants), editing the limits rebuilds the joint.
	fn on_inspect(&mut self,
	              joint: &mut ImpulseJoint,
	              entity1: &Entity,
	              entity2: &Entity,
	              application: &Application,
	              ui: &mut Ui) {
		let mut offset_changed = false;
		let mut limits_changed = false;
		
		let is_cone_twist = self.joint_desc.cone_twist.is_some();
		ui.label("Model");
		ui.horizontal(|ui| {
			if ui.selectable_label(!is_cone_twist, "6DoF").clicked()
			&& let Some(cone_twist) = self.joint_desc.cone_twist.take() {
				(self.joint_desc.rotation_min, self.joint_desc.rotation_max) = cone_twist.rotation_limits();
				limits_changed = true;
			}
			
			if ui.selectable_label(is_cone_twist, "ConeTwist").clicked() && !is_cone_twist {
				self.joint_desc.cone_twist = Some(ConeTwistDesc::from_rotation_limits(self.joint_desc.rotation_min, self.joint_desc.rotation_max));
				limits_changed = true;
			}
		});
		ui.end_row();
		
		let frame1 = *entity1.state().position * self.local_frame1;
		let mut frame2 = *entity2.state().position * self.local_frame2;
		if let Some(cone_twist) = self.joint_desc.cone_twist {
			frame2 = frame2 * from_euler(cone_twist.bias.x, cone_twist.bias.y, 0.0);
		}
		
		ui.label("");
		ui.columns(6, |ui| {
			ui[0].label("MIN");
			ui[1].label("CUR");
			ui[2].label("MAX");
			ui[3].label("BIAS");
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
		
		// limit_min is the bias for coupled/cone-axis
		let mut inspect_axis = |axis: JointAxis, limit_min: &mut f32, limit_max: &mut f32, free_limit: f32| {
			let free = !joint.data.locked_axes.contains(axis.into()) && !joint.data.limit_axes.contains(axis.into());
			let locked = joint.data.locked_axes.contains(axis.into());
			let coupled = joint.data.coupled_axes.contains(axis.into());
			
			ui.label(format!("{axis:?}{}", if coupled { " 🔗" } else { "" }));
			ui.columns(6, |ui| {
				let angle = JointAxesMask::ANG_AXES.contains(axis.into());
				let scale = if angle { 180.0 / PI } else { 1.0 };
				
				fn btn_col(active: bool) -> Color32 {
					if active { Color32::LIGHT_BLUE } else { Color32::GRAY }
				}
				
				if coupled {
					let limit_bias = limit_min;
					let mut cur = offsets[axis as usize] * scale;
					let mut max = *limit_max * scale;
					let mut bias = *limit_bias * scale;
					
					ui[0].add(Button::new("---"));
					
					if ui[1].add(DragValue::new(&mut cur).speed(0.01 * scale)).changed() {
						if cur < -max && !free { cur = -max; }
						if cur > max && !free { cur = max; }
						offsets[axis as usize] = cur / scale;
						offset_changed = true;
					}
					
					if ui[2].add(DragValue::new(&mut max).speed(0.01 * scale)).changed() {
						*limit_max = max / scale;
						limits_changed = true;
					}
					
					if ui[3].add(DragValue::new(&mut bias).speed(0.01 * scale)).changed() {
						*limit_bias = bias / scale;
						limits_changed = true;
					}
					
					if ui[4].add(Button::new(RichText::new("Free").color(btn_col(free)))).clicked() {
						*limit_max = free_limit;
						limits_changed = true;
					}
					
					if ui[5].add(Button::new(RichText::new("Lock").color(btn_col(locked)))).clicked() {
						*limit_max = 0.0;
						limits_changed = true;
					}
				} else {
					let mut min = *limit_min * scale;
					let mut cur = offsets[axis as usize] * scale;
					let mut max = *limit_max * scale;
					let mut bias = (min + max) / 2.0;
					
					if ui[0].add(DragValue::new(&mut min).speed(0.01 * scale)).changed() {
						*limit_min = min / scale;
						limits_changed = true;
					}
					
					if ui[1].add(DragValue::new(&mut cur).speed(0.01 * scale)).changed() {
						if cur < min && !free { cur = min; }
						if cur > max && !free { cur = max; }
						offsets[axis as usize] = cur / scale;
						offset_changed = true;
					}
					
					if ui[2].add(DragValue::new(&mut max).speed(0.01 * scale)).changed() {
						*limit_max = max / scale;
						limits_changed = true;
					}
					
					if ui[3].add(DragValue::new(&mut bias).speed(0.01 * scale)).changed() {
						let spread = max - min;
						min = bias - spread / 2.0;
						max = bias + spread / 2.0;
						*limit_min = min / scale;
						*limit_max = max / scale;
						limits_changed = true;
					}
					
					if ui[4].add(Button::new(RichText::new("Free").color(btn_col(free)))).clicked() {
						*limit_min = -free_limit / 2.0;
						*limit_max = free_limit / 2.0;
						limits_changed = true;
					}
					
					if ui[5].add(Button::new(RichText::new("Lock").color(btn_col(locked)))).clicked() {
						*limit_min = bias / scale;
						*limit_max = bias / scale;
						limits_changed = true;
					}
				}
			});
			ui.end_row();
		};
		
		inspect_axis(JointAxis::LinX, &mut self.joint_desc.position_min.x, &mut self.joint_desc.position_max.x, 100.0);
		inspect_axis(JointAxis::LinY, &mut self.joint_desc.position_min.y, &mut self.joint_desc.position_max.y, 100.0);
		inspect_axis(JointAxis::LinZ, &mut self.joint_desc.position_min.z, &mut self.joint_desc.position_max.z, 100.0);
		
		if let Some(cone) = self.joint_desc.cone_twist.as_mut() {
			inspect_axis(JointAxis::AngX, &mut cone.bias.x, &mut cone.swing, PI);
			inspect_axis(JointAxis::AngY, &mut cone.bias.y, &mut cone.swing, PI);
			inspect_axis(JointAxis::AngZ, &mut cone.twist.0, &mut cone.twist.1, PI * 2.0);
		} else {
			inspect_axis(JointAxis::AngX, &mut self.joint_desc.rotation_min.x, &mut self.joint_desc.rotation_max.x, PI * 2.0);
			inspect_axis(JointAxis::AngY, &mut self.joint_desc.rotation_min.y, &mut self.joint_desc.rotation_max.y, PI * 2.0);
			inspect_axis(JointAxis::AngZ, &mut self.joint_desc.rotation_min.z, &mut self.joint_desc.rotation_max.z, PI * 2.0);
		}
		
		if offset_changed {
			let i = (offsets[3] / 2.0).sin() * ang_sgn;
			let j = (offsets[4] / 2.0).sin() * ang_sgn;
			let k = (offsets[5] / 2.0).sin() * ang_sgn;
			let w = (1.0 - i * i - j * j - k * k).max(0.0).sqrt();
			
			let new_diff = Isometry3::from_parts(
				vector![offsets[0], offsets[1], offsets[2]].into(),
				Rot3::new_normalize(Quaternion::new(w, i, j, k)),
			);
			
			let entity1_new_pos = frame2 * new_diff.inverse() * Isometry3::from(joint.data.local_frame1).inverse();
			let transform = entity1_new_pos * entity1.state().position.inverse();
			*entity1.state_mut().position = entity1_new_pos;
			
			for desc in entity1.descendants(application) {
				let new_pos = transform * *desc.state().position;
				*desc.state_mut().position = new_pos;
			}
		}
		
		if limits_changed {
			joint.data = self.joint();
			entity1.state_mut().position.mark_mut();
		}
	}
	
	/// Builds the Rapier joint from the description. Axes with equal limits are locked, axes with
	/// limits spanning the full range are free. Cone twist joints get coupled swing axes.
	fn joint(&self) -> GenericJoint {
		let mut joint = GenericJoint::default();
		
		joint.set_local_frame1(self.local_frame1.into())
		     .set_local_frame2(self.local_frame2.into())
		     .set_contacts_enabled(false);
		
		fn limit(joint: &mut GenericJoint, axis: JointAxis, min: f32, max: f32, free_limit: f32) {
			if max - min >= free_limit || min > max {
				// free
			} else if min != max {
				joint.set_limits(axis, [min, max]);
				// Need rapier coupled limited joints
				// joint.set_motor(axis, 0.0, 0.0, 0.1, 0.5);
			} else {
				joint.lock_axes(axis.into());
			}
		}
		
		limit(&mut joint, JointAxis::LinX, self.joint_desc.position_min.x, self.joint_desc.position_max.x, 100.0);
		limit(&mut joint, JointAxis::LinY, self.joint_desc.position_min.y, self.joint_desc.position_max.y, 100.0);
		limit(&mut joint, JointAxis::LinZ, self.joint_desc.position_min.z, self.joint_desc.position_max.z, 100.0);
		
		if let Some(cone) = &self.joint_desc.cone_twist {
			joint.local_frame2 = (self.local_frame2 * from_euler(cone.bias.x, cone.bias.y, 0.0)).into();
			
			limit(&mut joint, JointAxis::AngX, 0.0, cone.swing, PI);
			limit(&mut joint, JointAxis::AngY, 0.0, cone.swing, PI);
			limit(&mut joint, JointAxis::AngZ, cone.twist.0, cone.twist.1, PI * 2.0);
			joint.coupled_axes |= JointAxesMask::ANG_X | JointAxesMask::ANG_Y;
		} else {
			limit(&mut joint, JointAxis::AngX, self.joint_desc.rotation_min.x, self.joint_desc.rotation_max.x, PI * 2.0);
			limit(&mut joint, JointAxis::AngY, self.joint_desc.rotation_min.y, self.joint_desc.rotation_max.y, PI * 2.0);
			limit(&mut joint, JointAxis::AngZ, self.joint_desc.rotation_min.z, self.joint_desc.rotation_max.z, PI * 2.0);
		}
		
		joint
	}
}
