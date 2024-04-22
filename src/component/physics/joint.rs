use std::cell::{Cell, RefCell};
use std::f32::consts::PI;
use anyhow::Result;
use egui::{Button, Color32, DragValue, RichText, Ui};
use nalgebra::{Quaternion, Unit};
use rapier3d::prelude::*;

use crate::application::{Entity, Application, Physics, EntityRef};
use crate::component::model::mmd::shared::JointDesc;
use crate::math::{Isometry3, Rot3, Vec3};
use crate::utils::ExUi;
use super::super::{Component, ComponentBase, ComponentInner};

#[derive(ComponentBase)]
pub struct JointComponent {
	#[inner] inner: ComponentInner,
	pub name: String,
	mmd_desc: Option<RefCell<MMDDesc>>,
	template: GenericJoint,
	target: EntityRef,
	handle: Cell<ImpulseJointHandle>,
}

impl JointComponent {
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
	
	pub fn from_mmd(local_frame1: Isometry3, local_frame2: Isometry3,
	                joint_desc: JointDesc,
	                target: impl Into<EntityRef>)
	                -> Self {
		let mmd_desc = MMDDesc::new(joint_desc, local_frame1, local_frame2);
		let mut component = Self::new(mmd_desc.joint(), target);
		
		component.mmd_desc = Some(RefCell::new(mmd_desc));
		
		component
	}
	
	pub fn named(mut self, name: impl Into<String>) -> Self {
		self.name = name.into();
		self
	}
	
	pub fn other<'a>(&self, application: &'a Application) -> Option<&'a Entity> {
		self.target.get(application)
	}
	
	pub fn handle(&self) -> ImpulseJointHandle {
		self.handle.get()
	}
	
	pub fn inner<'p>(&self, physics: &'p Physics) -> &'p ImpulseJoint {
		physics.impulse_joint_set.get(self.handle.get()).unwrap()
	}
	
	pub fn inner_mut<'p>(&self, physics: &'p mut Physics) -> &'p mut ImpulseJoint {
		physics.impulse_joint_set.get_mut(self.handle.get()).unwrap()
	}
}

impl Component for JointComponent {
	fn start(&self, entity: &Entity, application: &Application) -> Result<()> {
		let physics = &mut *application.physics.borrow_mut();
		
		if let Some(target) = self.target.get(application) {
			self.handle.set(physics.impulse_joint_set.insert(entity.rigid_body, target.rigid_body, self.template, true));
		}
		
		Ok(())
	}
	
	fn end(&self, _entity: &Entity, application: &Application) -> Result<()> {
		let physics = &mut *application.physics.borrow_mut();
		
		physics.impulse_joint_set.remove(self.handle.get(), true);
		
		Ok(())
	}
	
	fn on_inspect(&self, entity: &Entity, ui: &mut Ui, application: &Application) {
		if let Some(mmd_desc) = self.mmd_desc.as_ref() {
			if let Some(other) = self.other(application) {
				if let Ok(mut physics) = application.physics.try_borrow_mut() {
					if let Some(joint) = physics.impulse_joint_set.get_mut(self.handle.get()) {
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

struct MMDDesc {
	joint_desc: JointDesc,
	local_frame1: Isometry3,
	local_frame2: Isometry3,
}

impl MMDDesc {
	fn new(joint_desc: JointDesc, local_frame1: Isometry3, local_frame2: Isometry3) -> Self {
		Self { joint_desc, local_frame1, local_frame2 }
	}
	
	fn on_inspect(&mut self,
	              joint: &mut ImpulseJoint,
	              entity1: &Entity,
	              entity2: &Entity,
	              application: &Application,
	              ui: &mut Ui) {
		let frame1 = *entity1.state().position * self.local_frame1;
		let frame2 = *entity2.state().position * self.local_frame2;
		
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
		let mut offset_changed = false;
		let mut limits_changed = false;
		
		let mut inspect_axis = |axis: JointAxis, limit_min: &mut f32, limit_max: &mut f32, free_limit: f32| {
			let free = !joint.data.locked_axes.contains(axis.into()) && !joint.data.limit_axes.contains(axis.into());
			let locked = joint.data.locked_axes.contains(axis.into());
			let coupled = joint.data.coupled_axes.contains(axis.into());
			
			ui.label(format!("{axis:?}{}", if coupled { " 🔗" } else { "" }));
			ui.columns(6, |ui| {
				let angle = JointAxesMask::ANG_AXES.contains(axis.into());
				let scale = if angle { 180.0 / PI } else { 1.0 };
				
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
				
				fn btn_col(active: bool) -> Color32 {
					if active { Color32::LIGHT_BLUE } else { Color32::GRAY }
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
			});
			ui.end_row();
		};
		
		inspect_axis(JointAxis::X, &mut self.joint_desc.position_min.x, &mut self.joint_desc.position_max.x, 100.0);
		inspect_axis(JointAxis::Y, &mut self.joint_desc.position_min.y, &mut self.joint_desc.position_max.y, 100.0);
		inspect_axis(JointAxis::Z, &mut self.joint_desc.position_min.z, &mut self.joint_desc.position_max.z, 100.0);
		
		inspect_axis(JointAxis::AngX, &mut self.joint_desc.rotation_min.x, &mut self.joint_desc.rotation_max.x, PI * 2.0);
		inspect_axis(JointAxis::AngY, &mut self.joint_desc.rotation_min.y, &mut self.joint_desc.rotation_max.y, PI * 2.0);
		inspect_axis(JointAxis::AngZ, &mut self.joint_desc.rotation_min.z, &mut self.joint_desc.rotation_max.z, PI * 2.0);
		
		if offset_changed {
			let i = (offsets[3] / 2.0).sin() * ang_sgn;
			let j = (offsets[4] / 2.0).sin() * ang_sgn;
			let k = (offsets[5] / 2.0).sin() * ang_sgn;
			let w = (1.0 - i * i - j * j - k * k).max(0.0).sqrt();
			
			let new_diff = Isometry3::from_parts(
				vector![offsets[0], offsets[1], offsets[2]].into(),
				Rot3::new_normalize(Quaternion::new(w, i, j, k)),
			);
			
			let entity1_new_pos = frame2 * new_diff.inverse() * joint.data.local_frame1.inverse();
			let transform = entity1_new_pos * entity1.state().position.inverse();
			*entity1.state_mut().position = entity1_new_pos;
			
			for desc in entity1.descendants(application) {
				let new_pos = transform * *desc.state().position;
				*desc.state_mut().position = new_pos;
			}
		}
		
		if limits_changed {
			joint.data = self.joint();
		}
	}
	
	fn joint(&self) -> GenericJoint {
		let mut joint = GenericJoint::default();
		
		joint.set_local_frame1(self.local_frame1).set_local_frame2(self.local_frame2);
		
		fn limit(joint: &mut GenericJoint, axis: JointAxis, min: f32, max: f32, free_limit: f32) {
			if max - min >= free_limit || min > max {
				// free
			} else if min != max {
				joint.set_limits(axis, [min, max]);
			} else {
				joint.lock_axes(axis.into());
			}
		}
		
		fn couple(joint: &mut GenericJoint, axis1: JointAxis, axis2: JointAxis, angle_axis_1: Unit<Vec3>, angle_axis_2: Unit<Vec3>) {
			let limit1 = joint.limits(axis1).unwrap();
			let limit2 = joint.limits(axis2).unwrap();
			let local_frame2 = joint.local_frame2;
			let bias1 = (limit1.min + limit1.max) / 2.0;
			let bias2 = (limit2.min + limit2.max) / 2.0;
			
			let avg = (limit1.max - bias1 + limit2.max - bias2) / 2.0;
			
			joint.set_limits(axis1, [0.0, avg]);
			joint.set_limits(axis2, [0.0, avg]);
			
			joint.coupled_axes |= axis1.into();
			joint.coupled_axes |= axis2.into();
			
			joint.local_frame2 = local_frame2 * Rot3::from_axis_angle(&angle_axis_1, -bias1) * Rot3::from_axis_angle(&angle_axis_2, -bias2);
		}
		
		limit(&mut joint, JointAxis::X, self.joint_desc.position_min.x, self.joint_desc.position_max.x, 100.0);
		limit(&mut joint, JointAxis::Y, self.joint_desc.position_min.y, self.joint_desc.position_max.y, 100.0);
		limit(&mut joint, JointAxis::Z, self.joint_desc.position_min.z, self.joint_desc.position_max.z, 100.0);
		
		limit(&mut joint, JointAxis::AngX, self.joint_desc.rotation_min.x, self.joint_desc.rotation_max.x, PI * 2.0);
		limit(&mut joint, JointAxis::AngY, self.joint_desc.rotation_min.y, self.joint_desc.rotation_max.y, PI * 2.0);
		limit(&mut joint, JointAxis::AngZ, self.joint_desc.rotation_min.z, self.joint_desc.rotation_max.z, PI * 2.0);
		
		match (joint.limits(JointAxis::AngX), joint.limits(JointAxis::AngY), joint.limits(JointAxis::AngZ)) {
			(Some(x), Some(y), Some(z)) => {
				let dx = x.max - x.min;
				let dy = y.max - y.min;
				let dz = z.max - z.min;
				let dxy = f32::abs(dx - dy);
				let dyz = f32::abs(dy - dz);
				let dxz = f32::abs(dx - dz);
				let min = dxy.min(dyz).min(dxz);
				
				if min == dxy { couple(&mut joint, JointAxis::AngX, JointAxis::AngY, Vec3::x_axis(), Vec3::y_axis()); }
				else if min == dyz { couple(&mut joint, JointAxis::AngY, JointAxis::AngZ, Vec3::y_axis(), Vec3::z_axis()); }
				else if min == dxz { couple(&mut joint, JointAxis::AngX, JointAxis::AngZ, Vec3::x_axis(), Vec3::z_axis()); }
			},
			(Some(_), Some(_), None) => couple(&mut joint, JointAxis::AngX, JointAxis::AngY, Vec3::x_axis(), Vec3::y_axis()),
			(None, Some(_), Some(_)) => couple(&mut joint, JointAxis::AngY, JointAxis::AngZ, Vec3::y_axis(), Vec3::z_axis()),
			(Some(_), None, Some(_)) => couple(&mut joint, JointAxis::AngX, JointAxis::AngZ, Vec3::x_axis(), Vec3::z_axis()),
			_ => {},
		}
		
		joint
	}
}
