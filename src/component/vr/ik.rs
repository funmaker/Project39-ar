use std::cell::Cell;
use std::time::Duration;
use egui::Ui;
use rapier3d::dynamics::{RigidBodyBuilder, RigidBodyType};
use rapier3d::prelude::{ColliderBuilder, RigidBodyHandle};

use crate::application::{Entity, Application, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::math::{Isometry3, Vec3, to_euler, from_euler};
use crate::utils::{ExUi, get_user_data};

// Based on Mathias Parger's
// Inverse Kinematics for Virtual Reality
// https://diglib.tugraz.at/download.php?id=5c4a48dc5a282&location=browse

#[derive(ComponentBase)]
pub struct VrIk {
	#[inner] inner: ComponentInner,
	hand_left: EntityRef,
	hand_right: EntityRef,
	rb_torso: Cell<RigidBodyHandle>,
	
	neck_offset: Cell<Vec3>,
	shoulder_offset: Cell<Vec3>,
}

impl VrIk {
	pub fn new(hand_left: EntityRef, hand_right: EntityRef) -> Self {
		VrIk {
			inner: ComponentInner::new_norender(),
			hand_left,
			hand_right,
			rb_torso: Cell::new(RigidBodyHandle::invalid()),
			
			neck_offset: Cell::new(Vec3::new(0.0, -0.1, -0.05)),
			shoulder_offset: Cell::new(Vec3::new(0.0, -0.1, -0.02)),
		}
	}
}

impl Component for VrIk {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let physics = &mut *application.physics.borrow_mut();
		let user_data = get_user_data(entity.id, self.id());
		
		self.rb_torso.set(physics.rigid_body_set.insert(
			RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
				.user_data(user_data)
				.build()
		));
		
		physics.collider_set.insert_with_parent(
			ColliderBuilder::cuboid(0.2, 0.25, 0.1)
				.translation(vector!(0.0, -0.2, 0.0))
				.user_data(user_data),
			self.rb_torso.get(),
			&mut physics.rigid_body_set,
		);
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let state = entity.state();
		let physics = &mut *application.physics.borrow_mut();
		
		if let Some(torso) = physics.rigid_body_set.get_mut(self.rb_torso.get()) {
			let mut position = Isometry3::identity();
			
			position.translation = state.position.translation;
			position.translation.vector += state.position.transform_vector(&self.neck_offset.get());
			position.translation.vector += self.shoulder_offset.get();
			
			let (pitch, yaw, _) = to_euler(state.position.rotation);
			
			position.rotation = from_euler(pitch, yaw, 0.0);
			
			torso.set_position(position, true);
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		ui.inspect_row("Neck Offset", &self.neck_offset, ());
		ui.inspect_row("Shoulder Offset", &self.shoulder_offset, ());
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_collapsing()
		  .title("Torso")
		  .show(ui, self.rb_torso.get(), application);
	}
}
