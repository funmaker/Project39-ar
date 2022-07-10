use std::time::{Duration, Instant};
use std::cell::Cell;
use std::ops::DerefMut;
use rapier3d::prelude::{InteractionGroups, ColliderHandle};
use rapier3d::geometry::Ball;

use crate::application::{Entity, Application, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::component::parent::Parent;
use crate::utils::ColliderEx;
use crate::math::{Point3, Color, Translation3};
use crate::debug;
use crate::component::vr::VrTracked;

const GRAB_DIST: f32 = 0.1;
const WALK_SPEED: f32 = 1.5;

#[derive(Debug, Copy, Clone)]
struct FreezeAnim {
	start: Instant,
	origin: Point3,
	dir: f32,
}

#[derive(ComponentBase, Debug)]
pub struct HandComponent {
	#[inner] inner: ComponentInner,
	pub hand: Hand,
	target_parent: ComponentRef<Parent>,
	sticky: Cell<bool>,
	freeze_anim: Cell<Option<FreezeAnim>>,
}

impl HandComponent {
	pub fn new(hand: Hand) -> Self {
		HandComponent {
			inner: ComponentInner::new_norender(),
			target_parent: ComponentRef::null(),
			hand,
			sticky: Cell::new(false),
			freeze_anim: Cell::new(None),
		}
	}
}

impl Component for HandComponent {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(item_parent) = self.target_parent.get(application) {
			let item = item_parent.entity(application);
			
			if application.input.use3_btn(self.hand).down {
				item.freeze(application.physics.borrow_mut().deref_mut());
				item.unset_tag("Grabbed");
				
				self.freeze_anim.set(Some(FreezeAnim {
					start: Instant::now(),
					origin: item.state().position.transform_point(&Point3::origin()),
					dir: -1.0,
				}));
			}
			
			if item.tag::<ComponentRef<HandComponent>>("Grabbed") != Some(self.as_cref())
			|| (!self.sticky.get() && application.input.use_btn(self.hand).up) {
				item.unset_tag("Grabbed");
				item_parent.remove();
				entity.state_mut().hidden = false;
			}
		} else if application.input.use_btn(self.hand).down || application.input.use3_btn(self.hand).down {
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
				if target.unfreeze(application.physics.borrow_mut().deref_mut()) {
					self.freeze_anim.set(Some(FreezeAnim {
						start: Instant::now(),
						origin: target.state().position.transform_point(&Point3::origin()),
						dir: 1.0,
					}));
				}
				
				if application.input.use_btn(self.hand).down {
					let grab_pos = target.tag("GrabPos").unwrap_or(entity.state().position.inverse() * *target.state().position);
					
					target.set_tag("Grabbed", self.as_cref());
					self.sticky.set(target.tag("GrabSticky").unwrap_or_default());
					self.target_parent.set(target.add_component(Parent::new(entity, grab_pos)));
					entity.state_mut().hidden = true;
				}
			}
		} else if let Some(root) = entity.find_component_by_type::<VrTracked>()
		                                 .and_then(|tracked| tracked.root.entity().get(application)) {
			if !root.has_tag("Seat") {
				if let Some(input) = application.input.controller(self.hand) {
					let dir = vector!(input.axis(0), 0.0, -input.axis(1)) * WALK_SPEED * delta_time.as_secs_f32();
					let dir = *entity.state().position * dir;
					
					root.state_mut().position.append_translation_mut(&Translation3::new(dir.x, 0.0, dir.z));
				}
			}
		}
		
		if let Some(anim) = self.freeze_anim.get() {
			let elapsed = anim.start.elapsed().as_secs_f32();
			
			let prog = (elapsed * anim.dir * 4.0).rem_euclid(1.0);
			
			if (elapsed * anim.dir * 4.0).abs() > 1.0 {
				self.freeze_anim.set(None);
			} else {
				debug::draw_point(anim.origin, prog * 200.0, Color::cyan().opactiy(1.0 - prog));
			}
		}
		
		Ok(())
	}
}


