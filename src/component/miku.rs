use std::time::Duration;
use std::cell::Cell;

use crate::application::{Entity, Application};
use crate::component::{ComponentRef, ComponentError};
use crate::component::model::MMDModel;
use crate::math::Rot3;
use crate::debug;
use super::{Component, ComponentBase, ComponentInner};

const MORPH_PRESETS: &[(usize, &[usize])] = &[
	(1, &[0, 29, 66]),
	(2, &[1, 45, 92]),
	(3, &[24, 65]),
	(4, &[39, 60]),
	(5, &[47, 2, 61])
];

#[derive(ComponentBase)]
pub struct Miku {
	#[inner] inner: ComponentInner,
	model: ComponentRef<MMDModel::<u16>>,
	hair_swing: Cell<f32>,
}

impl Miku {
	pub fn new() -> Self {
		Miku {
			inner: ComponentInner::new(),
			model: ComponentRef::null(),
			hair_swing: Cell::new(0.0),
		}
	}
}

impl Component for Miku {
	fn start(&self, entity: &Entity, application: &Application) -> Result<(), ComponentError> {
		self.model.set(entity.add_component(MMDModel::<u16>::from_pmx("YYB式初音ミクCrude Hair/YYB式初音ミクCrude Hair.pmx", &mut *application.renderer.borrow_mut())?));
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, _application: &Application, delta_time: Duration) -> Result<(), ComponentError> {
		let mut model = self.model
		                    .using(entity)
		                    .unwrap()
		                    .state
		                    .borrow_mut();
		
		let hair_swing = self.hair_swing.get() + delta_time.as_secs_f32() * 3.0;
		self.hair_swing.set(hair_swing);
		
		for id in 0..model.bones.len() {
			if model.bones[id].name.starts_with("Right H") || model.bones[id].name.starts_with("Left H") {
				let swing = hair_swing.sin() / 30.0;
				model.bones[id].anim_transform.isometry.rotation = Rot3::from_euler_angles(0.0, 0.0, swing);
			}
			if model.bones[id].name == "Bend" {
				let swing = (hair_swing / 3.0).sin() * std::f32::consts::PI / 2.0;
				model.bones[id].anim_transform.isometry.rotation = Rot3::from_euler_angles(0.0, 0.0, swing);
			}
		}
	
		for morph in model.morphs.iter_mut() {
			*morph = (*morph - 5.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
	
		let active = MORPH_PRESETS.iter().filter(|p| debug::get_flag_or_default(&format!("KeyKey{}", p.0))).flat_map(|p| p.1.iter());
	
		for &id in active {
			model.morphs[id] = (model.morphs[id] + 10.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
		
		Ok(())
	}
}