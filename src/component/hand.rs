use std::cell::{Cell, RefCell};
use std::ops::DerefMut;
use std::time::{Duration, Instant};
use egui::{RichText, Ui};
use rapier3d::dynamics::FixedJoint;
use rapier3d::geometry::Ball;
use rapier3d::pipeline::QueryFilter;
use rapier3d::prelude::ColliderHandle;

use crate::debug;
use crate::application::{Entity, Application, Hand, EntityRef};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use crate::component::physics::joint::JointComponent;
use crate::component::vr::VrTracked;
use crate::math::{Point3, Color, Translation3, Isometry3};
use crate::utils::{ColliderEx, ExUi};


const GRAB_DIST: f32 = 0.1;
const WALK_SPEED: f32 = 1.5;

#[derive(Debug, Copy, Clone)]
struct FreezeAnim {
	start: Instant,
	origin: Point3,
	dir: f32,
}

#[derive(Debug, Clone)]
enum Grab {
	None,
	Dynamic(ComponentRef<JointComponent>),
	Kinematic(EntityRef),
}

#[derive(ComponentBase, Debug)]
pub struct HandComponent {
	#[inner] inner: ComponentInner,
	pub hand: Hand,
	grab: RefCell<Grab>,
	sticky: Cell<bool>,
	freeze_anim: Cell<Option<FreezeAnim>>,
}

impl HandComponent {
	pub fn new(hand: Hand) -> Self {
		HandComponent {
			inner: ComponentInner::new_norender(),
			grab: RefCell::new(Grab::None),
			hand,
			sticky: Cell::new(false),
			freeze_anim: Cell::new(None),
		}
	}
	
	pub fn grabbed_entity(&self) -> EntityRef {
		match &*self.grab.borrow() {
			Grab::None => EntityRef::null(),
			Grab::Dynamic(joint) => joint.entity(),
			Grab::Kinematic(entity) => entity.clone(),
		}
	}
}

impl Component for HandComponent {
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		if let Some(item) = self.grabbed_entity().get(application) {
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
				entity.state_mut().hidden = false;
				
				match &*self.grab.borrow() {
					Grab::None => {},
					Grab::Dynamic(joint) => {
						if let Some(joint) = joint.get(application) {
							joint.remove();
						}
					},
					Grab::Kinematic(_) => {
						if item.parent() == entity {
							item.unset_parent(application);
						}
					},
				}
				
				self.grab.replace(Grab::None);
			}
		} else if application.input.use_btn(self.hand).down || application.input.use3_btn(self.hand).down {
			let mut target = None;
			let mut dynamic = false;
			
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
					
					dynamic = col.parent()
					             .and_then(|rb| physics.rigid_body_set.get(rb))
					             .map(|rb| rb.is_dynamic())
					             .unwrap_or(false);
					false
				};
				
				physics.query_pipeline.intersections_with_shape(&physics.rigid_body_set,
				                                                &physics.collider_set,
				                                                &entity.state().position,
				                                                &Ball::new(GRAB_DIST),
				                                                QueryFilter::new(),
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
					if dynamic {
						self.grab.replace(Grab::Dynamic(target.add_component(JointComponent::new(
							*FixedJoint::new()
								.set_local_frame1(Isometry3::identity())
								.set_local_frame2(grab_pos),
							entity,
						))));
					} else {
						let root = target.root(application);
						
						root.set_parent_and_offset(entity.as_ref(), Some(grab_pos), application);
						
						self.grab.replace(Grab::Kinematic(root.as_ref()));
					}
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
				debug::draw_point(anim.origin, prog * 200.0, Color::CYAN.opactiy(1.0 - prog));
			}
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.inspect_row("Hand", format!("{:?}", self.hand), ());
		ui.inspect_row("Sticky", &self.sticky, ());
		ui.label("Grabbed");
		match &*self.grab.borrow() {
			Grab::None => {
				ui.label(RichText::new("NONE").monospace().italics());
				ui.end_row();
			},
			Grab::Dynamic(joint) => ui.inspect(joint, application),
			Grab::Kinematic(entity) => ui.inspect(entity, application),
		}
	}
}


