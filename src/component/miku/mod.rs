use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use simba::scalar::SubsetOf;

mod enums;
mod proc_anim;

use crate::application::{Entity, Application};
use crate::component::model::mmd::asset::PmxAsset;
use crate::component::model::MMDModel;
use crate::utils::num_key;
use crate::math::Rot3;
use super::{Component, ComponentBase, ComponentInner, ComponentRef, ComponentError};
pub use enums::{Bones, Morphs, RigidBodies};
use proc_anim::{ProcAnim, Easing};

const MORPH_PRESETS: &[(usize, &[Morphs])] = &[
	(1, &[Morphs::Embarrassment]),
	(2, &[Morphs::Embarrassment3]),
	(3, &[Morphs::BeEmbarrassed]),
	(4, &[Morphs::Embarrassment2]),
	(5, &[Morphs::Smiling]),
	(6, &[Morphs::ASmile]),
	(7, &[Morphs::Grin]),
	(8, &[Morphs::Grin2]),
	(9, &[Morphs::BWinkRight]),
	(0, &[Morphs::BWink2Right]),
];

#[derive(ComponentBase)]
pub struct Miku {
	#[inner] inner: ComponentInner,
	model: ComponentRef<MMDModel>,
	pub anims: RefCell<(
		HashMap<Bones, ProcAnim<Rot3>>,
		HashMap<Morphs, ProcAnim<f32>>,
	)>,
}

impl Miku {
	pub fn new() -> Self {
		Miku {
			inner: ComponentInner::new_norender(),
			model: ComponentRef::null(),
			anims: RefCell::new((
				collection!(
					Bones::UpperBody2 => ProcAnim::new(Rot3::identity())
					                              .anim(Rot3::from_euler_angles(0.05, 0.0, 0.0), 3.0, Easing::EaseInOut)
					                              .anim(Rot3::from_euler_angles(0.0, 0.0, 0.0), 3.0, Easing::EaseInOut)
					                              .wait(0.5)
					                              .repeat(),
					// Bones::ShoulderL => ProcAnim::new(Rot3::identity())
					//                              .anim(Rot3::from_euler_angles(0.0, 0.0, -0.1), 3.0, Easing::EaseInOut)
					//                              .anim(Rot3::from_euler_angles(0.0, 0.0, 0.0), 3.0, Easing::EaseInOut)
					//                              .wait(0.5)
					//                              .repeat(),
					// Bones::ShoulderR => ProcAnim::new(Rot3::identity())
					//                              .anim(Rot3::from_euler_angles(0.0, 0.0, 0.1), 3.0, Easing::EaseInOut)
					//                              .anim(Rot3::from_euler_angles(0.0, 0.0, 0.0), 3.0, Easing::EaseInOut)
					//                              .wait(0.5)
					//                              .repeat(),
					// Bones::LeftArm => ProcAnim::new(Rot3::identity())
					//                            .anim(Rot3::from_euler_angles(0.0, 0.0, 0.605), 3.0, Easing::EaseOut)
					//                            .anim(Rot3::from_euler_angles(0.0, 0.0, 0.6), 3.0, Easing::EaseOut)
					//                            .wait(0.5)
					//                            .repeat(),
					// Bones::RightArm => ProcAnim::new(Rot3::identity())
					//                            .anim(Rot3::from_euler_angles(0.0, 0.0, -0.605), 3.0, Easing::EaseOut)
					//                            .anim(Rot3::from_euler_angles(0.0, 0.0, -0.6), 3.0, Easing::EaseOut)
					//                            .wait(0.5)
					//                            .repeat(),
				), collection!(
					Morphs::Blink => ProcAnim::new(0.0)
					                          .wait(0.5..5.0)
					                          .anim(1.0, 0.1, Easing::EaseIn)
					                          .wait(0.0..0.2)
					                          .anim(0.0, 0.1, Easing::EaseOut)
					                          .repeat(),
					Morphs::Embarrassment => ProcAnim::new(0.0)
					                                  .anim(1.0, 15.0, Easing::Step)
					                                  .anim(0.0, 5.0, Easing::Step)
					                                  .no_autoplay(),
					Morphs::Smiling => ProcAnim::new(0.0)
					                            .anim(1.0, 1.0, Easing::EaseIn)
					                            .wait(15.0)
					                            .anim(0.0, 1.0, Easing::EaseOut)
					                            .no_autoplay(),
				)
			)),
		}
	}
}

impl Component for Miku {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		let model = application.renderer.borrow_mut().load(PmxAsset::at("YYB式初音ミクCrude Hair/YYB式初音ミクCrude Hair.pmx"))?;
		self.model.set(entity.add_component(MMDModel::new(model, &mut *application.renderer.borrow_mut())?));
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let mut model = self.model
		                    .using(entity)
		                    .unwrap()
		                    .state
		                    .borrow_mut();
		
		for morph in model.morphs.iter_mut() {
			*morph = (*morph - 5.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
		
		let active = MORPH_PRESETS.iter().filter(|p| application.input.keyboard.pressed(num_key(p.0))).flat_map(|p| p.1.iter());
		
		for &id in active {
			let id = id as usize;
			model.morphs[id] = (model.morphs[id] + 10.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
		
		let mut anims = self.anims.borrow_mut();
		
		let physics = &mut *application.physics.borrow_mut();
		
		if let Some(rb) = physics.rigid_body_set.get(model.rigid_bodies[RigidBodies::LeftForelock as usize].handle) {
			if rb.linvel().magnitude() > 0.075 {
				anims.1.get_mut(&Morphs::Embarrassment).unwrap().play();
				anims.1.get_mut(&Morphs::Smiling).unwrap().play();
				if !anims.1.get_mut(&Morphs::Blink).unwrap().stopped() {
					anims.1.get_mut(&Morphs::Blink).unwrap().overdrive(
						ProcAnim::new(0.0)
						         .anim(1.0, 0.3, Easing::EaseIn)
						         .wait(3.0)
						         .anim(0.0, 0.3, Easing::EaseOut)
						         .wait(3.0)
					);
				}
			}
		}
		
		for (&bone, anim) in anims.0.iter_mut() {
			let value = anim.get();
			model.bones[bone as usize].anim_transform = value.to_superset();
		}
		
		for (&morph, anim) in anims.1.iter_mut() {
			let value = anim.get();
			model.morphs[morph as usize] = value;
		}
		
		Ok(())
	}
}
