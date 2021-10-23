use std::time::Duration;
use std::cell::Cell;
use rapier3d::prelude::{InteractionGroups, ColliderHandle};
use rapier3d::geometry::Ball;

use crate::application::{Entity, Application, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::component::parent::Parent;
use crate::utils::ColliderEx;

const GRAB_DIST: f32 = 0.1;

#[derive(ComponentBase, Debug)]
pub struct HandComponent {
	#[inner] inner: ComponentInner,
	pub hand: Hand,
	target_parent: ComponentRef<Parent>,
	sticky: Cell<bool>,
}

impl HandComponent {
	pub fn new(hand: Hand) -> Self {
		HandComponent {
			inner: ComponentInner::new(),
			target_parent: ComponentRef::null(),
			hand,
			sticky: Cell::new(false),
		}
	}
}

impl Component for HandComponent {
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(item_parent) = self.target_parent.get(application) {
			if item_parent.entity(application).tag::<ComponentRef<HandComponent>>("Grabbed") != Some(self.as_cref())
			|| (!self.sticky.get() && application.input.use_btn(self.hand).up) {
				item_parent.remove();
				entity.state_mut().hidden = false;
			}
		} else if application.input.use_btn(self.hand).down {
			let mut target = None;
			
			{
				let physics = application.physics.borrow_mut();
				
				let callback = |col: ColliderHandle| {
					let col = physics.collider_set.get(col).unwrap();
					if col.entity_ref() == entity {
						return true;
					}
					
					let ent = col.entity(application);
					if ent.tag("World").unwrap_or_default() || ent.tag("NoGrab").unwrap_or_default() {
						return true;
					}
					
					target = Some(ent);
					false
				};
				
				physics.query_pipeline.intersections_with_shape(&physics.collider_set,
				                                                &entity.state().position,
				                                                &Ball::new(GRAB_DIST),
				                                                InteractionGroups::all(),
				                                                None,
				                                                callback);
			}
			
			if let Some(target) = target {
				let grab_pos = target.tag("GrabPos").unwrap_or(entity.state().position.inverse() * target.state().position);
				
				target.set_tag("Grabbed", self.as_cref());
				self.sticky.set(target.tag("GrabSticky").unwrap_or_default());
				self.target_parent.set(target.add_component(Parent::new(entity, grab_pos)));
				entity.state_mut().hidden = true;
			}
		}
		
		Ok(())
	}
}


