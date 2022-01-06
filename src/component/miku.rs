use std::time::Duration;

use crate::application::{Entity, Application};
use crate::component::model::mmd::asset::PmxAsset;
use crate::component::model::MMDModel;
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
}

impl Miku {
	pub fn new() -> Self {
		Miku {
			inner: ComponentInner::new(),
			model: ComponentRef::null(),
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
			model.morphs[id] = (model.morphs[id] + 10.0 * delta_time.as_secs_f32()).clamp(0.0, 1.0);
		}
		
		Ok(())
	}
}
