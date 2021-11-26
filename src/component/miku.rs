use std::time::Duration;
use std::cell::Cell;

use crate::application::{Entity, Application};
use crate::component::model::mmd::asset::PmxAsset;
use crate::component::model::MMDModel;
use crate::math::Rot3;
use crate::utils::num_key;
use super::{Component, ComponentBase, ComponentInner, ComponentRef, ComponentError};

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
	model: ComponentRef<MMDModel>,
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
		
		let hair_swing = self.hair_swing.get() + delta_time.as_secs_f32() * 3.0;
		self.hair_swing.set(hair_swing);
		
		for id in 0..model.bones.len() {
			if model.bones[id].name.starts_with("Right H") || model.bones[id].name.starts_with("Left H") {
				// let swing = hair_swing.sin() / 30.0;
				// model.bones[id].anim_transform.isometry.rotation = Rot3::from_euler_angles(0.0, 0.0, swing);
			}
			if model.bones[id].name == "Bend" {
				// let swing = (hair_swing / 3.0).sin() * std::f32::consts::PI / 2.0;
				// model.bones[id].anim_transform.isometry.rotation = Rot3::from_euler_angles(0.0, 0.0, swing);
			}
		}
	
		for morph in model.morphs.iter_mut() {
			*morph = (*morph - 5.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
	
		let active = MORPH_PRESETS.iter().filter(|p| application.input.keyboard.pressed(num_key(p.0))).flat_map(|p| p.1.iter());
	
		for &id in active {
			model.morphs[id] = (model.morphs[id] + 10.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
		
		Ok(())
	}
}
