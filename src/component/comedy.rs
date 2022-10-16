use std::cell::Cell;
use std::time::Duration;
use egui::Ui;
use nalgebra::Point3;
use rapier3d::dynamics::RigidBodyType;
use rapier3d::prelude::{ColliderBuilder, InteractionGroups, RevoluteJoint};

use crate::application::{Entity, Application, EntityRef};
use crate::math::{Color, Isometry3, Similarity3, Vec3};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::component::model::simple::{ObjAsset, ObjLoadError};
use crate::component::model::SimpleModel;
use crate::component::physics::joint::JointComponent;
use crate::renderer::{RenderContext, Renderer, RenderType};
use crate::utils::ExUi;

const SCALE: f32 = 0.5;

#[derive(ComponentBase)]
pub struct Comedy {
	#[inner] inner: ComponentInner,
	model: SimpleModel,
	iris: EntityRef,
	iris_pos: Cell<Isometry3>,
}

impl Comedy {
	pub fn new(renderer: &mut Renderer) -> Result<Self, ObjLoadError> {
		Ok(Comedy {
			inner: ComponentInner::from_render_type(RenderType::Transparent),
			model: renderer.load(ObjAsset::at("comedy/base.obj", "comedy/base.png"))?,
			iris: EntityRef::null(),
			iris_pos: Cell::new(Isometry3::identity()),
		})
	}
}

impl Component for Comedy {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let pos = *entity.state().position;
		
		self.iris.set(application.add_entity(
			Entity::builder("Iris")
				.position(pos * Isometry3::translation(0.0, 0.02 * SCALE, 0.0))
				.collider(ColliderBuilder::ball(0.025)
				                          .collision_groups(InteractionGroups::none())
				                          .build())
				.rigid_body_type(RigidBodyType::Dynamic)
				.component(JointComponent::new(
					*RevoluteJoint::new(Vec3::z_axis())
					               .set_local_anchor1(point!(0.0, -0.02 * SCALE, 0.0))
					               .set_local_anchor2(Point3::origin()),
					entity.as_ref()
				))
				.build()
		));
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(iris) = self.iris.get(application) {
			self.iris_pos.set(*iris.state().position);
			
			let iris_state = &mut *iris.state_mut();
			let fric = 0.1_f32.powf(delta_time.as_secs_f32());
			*iris_state.angular_velocity *= fric;
			
			let self_pos = entity.state().position.translation.vector;
			let pos_diff = iris_state.position.translation.vector - self_pos;
			if pos_diff.magnitude_squared() > 0.02 * SCALE * 0.02 * SCALE {
				iris_state.position.translation.vector = self_pos + pos_diff.normalize() * 0.02 * SCALE;
			}
		} else {
			entity.remove();
		}
		
		Ok(())
	}
	
	fn render(&self, entity: &Entity, context: &mut RenderContext, _renderer: &mut Renderer) -> Result<(), ComponentError> {
		let base_pos = *entity.state().position;
		
		self.model.render_impl(Similarity3::from_isometry(self.iris_pos.get(), 0.6 * SCALE), Color::full_black(), context)?;
		self.model.render_impl(Similarity3::from_isometry(base_pos, 1.0 * SCALE), Color::full_white().opactiy(0.6), context)?;
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_row("Iris", &self.iris, application);
		ui.inspect_row("Offset", &self.iris_pos, ());
	}
}
