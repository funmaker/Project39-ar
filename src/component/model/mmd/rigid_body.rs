use std::cell::Cell;
use std::time::Duration;
use anyhow::Result;
use egui::Ui;

use crate::application::{Entity, Application};
use crate::component::model::MMDModel;
use crate::math::Isometry3;
use crate::utils::ExUi;
use super::super::super::{ComponentRef, Component, ComponentBase, ComponentInner};
use super::super::super::physics::joint::JointComponent;
use super::BodyPart;


#[derive(ComponentBase)]
pub struct MMDRigidBody {
	#[inner] inner: ComponentInner,
	pub bone: usize,
	pub body_part: Option<BodyPart>,
	pub rest_pos: Cell<Isometry3>,
	pub joint: ComponentRef<JointComponent>,
	pub model: ComponentRef<MMDModel>,
}

impl MMDRigidBody {
	pub fn new(bone: usize, body_part: Option<BodyPart>, rest_pos: Isometry3, joint: ComponentRef<JointComponent>, model: ComponentRef<MMDModel>) -> Self {
		MMDRigidBody {
			inner: ComponentInner::new_norender(),
			bone,
			body_part,
			rest_pos: rest_pos.into(),
			joint,
			model,
		}
	}
}

impl Component for MMDRigidBody {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<()> {
		
		// TODO: investigate unintended sleeps?
		if let Some(rb) = application.physics.borrow_mut().rigid_body_set.get_mut(entity.rigid_body) {
			rb.wake_up(true);
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_row("Bone", (self.model.clone(), self.bone), application);
		ui.inspect_row("Body Part", self.body_part.map_or_else(|| "NONE".into(), |body_part| format!("{:?}", body_part)), ());
		ui.inspect_row("Rest Pos", &self.rest_pos, ());
		ui.inspect_row("Joint", &self.joint, application);
		ui.inspect_row("Model", &self.model, application);
	}
}
