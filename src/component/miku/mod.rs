use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::Duration;
use anyhow::Result;
use egui::Ui;
use simba::scalar::SubsetOf;

mod anim;
mod character;
mod enums;
mod gui;
mod proc_anim;

use crate::debug;
use crate::application::{Entity, Application};
use crate::math::Rot3;
use crate::renderer::assets_manager::TomlAsset;
use crate::utils::num_key;
use super::{Component, ComponentBase, ComponentInner, ComponentRef};
use super::model::MMDModel;
use super::model::mmd::{BodyPart, MMDRigidBody};
use super::model::mmd::asset::PmxAsset;
pub use character::Character;
pub use enums::{Bones, Morphs};
use character::{CharacterConfig, ModelCharacterConfig};
use proc_anim::{ProcAnim, Easing};


/// Facial expression presets, activated by holding the given number key.
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

/// The Miku character: loads her MMD model and brings it to life as an OpenViva character (see
/// `character`), if the model's `model.toml` has a `[character]` section. Otherwise only procedural idle
/// animations (breathing, blinking) are played.
///
/// Number keys show facial expression presets. Also shows the character state, a ragdoll and
/// bone/rigid body browser in the inspector.
#[derive(ComponentBase)]
pub struct Miku {
	#[inner] inner: ComponentInner,
	asset: PmxAsset,
	/// Model component created on start.
	pub model: ComponentRef<MMDModel>,
	/// Character configuration from `model.toml`.
	config: RefCell<Option<CharacterConfig>>,
	/// Character runtime, created once the model's ragdoll is ready.
	pub character: RefCell<Option<Character>>,
	/// Creating the character failed, don't retry.
	character_failed: Cell<bool>,
	/// Procedural animations of bones and morphs.
	pub anims: RefCell<(
		HashMap<Bones, ProcAnim<Rot3>>,
		HashMap<Morphs, ProcAnim<f32>>,
	)>,
}

impl Miku {
	pub fn new(asset: PmxAsset) -> Self {
		Miku {
			inner: ComponentInner::new_norender(),
			asset,
			model: ComponentRef::null(),
			config: RefCell::new(None),
			character: RefCell::new(None),
			character_failed: Cell::new(false),
			anims: RefCell::new((
				HashMap::new(), collection!(
					Morphs::Blink => ProcAnim::new(0.0)
					                          .wait(0.5..5.0)
					                          .anim(1.0, 0.1, Easing::EaseIn)
					                          .wait(0.0..0.2)
					                          .anim(0.0, 0.1, Easing::EaseOut)
					                          .repeat(),
				)
			)),
		}
	}
	
	/// Creates the character once possible and runs it.
	fn update_character(&self, entity: &Entity, model: &MMDModel, application: &Application, delta_time: Duration) {
		if self.character_failed.get() { return; }
		let mut character = self.character.borrow_mut();
		
		if character.is_none() {
			let Some(config) = self.config.borrow().clone() else { return };
			match Character::new(config, model, entity, application) {
				Ok(created) => *character = Some(created),
				Err(err) => {
					eprintln!("Unable to create character: {:?}", err);
					self.character_failed.set(true);
					return;
				},
			}
		}
		
		let Some(character) = character.as_mut() else { return };
		if character.bodies.is_empty() && !character.init_bodies(entity, application) {
			return;
		}
		
		for morph in model.state_mut().morphs.iter_mut() {
			*morph = 0.0;
		}
		
		character.update(application, delta_time.as_secs_f32());
	}
	
	/// Returns the ragdoll rigid body of the given body part, if the model has one.
	pub fn body_part<'a>(&self, body_part: BodyPart, application: &'a Application) -> Option<&'a MMDRigidBody> {
		self.model
			.get(application)
			.and_then(|model| model.state.try_borrow().ok())
			.iter()
			.flat_map(|state| state.rigid_bodies.iter())
			.filter_map(|rb| rb.get(application))
			.find(|rb| rb.body_part == Some(body_part))
	}
}

impl Component for Miku {
	fn start(&self, entity: &Entity, application: &Application) -> Result<()> {
		let model = application.renderer.borrow_mut().load(self.asset.clone())?;
		self.model.set(entity.add_component(MMDModel::new(model, &mut *application.renderer.borrow_mut())?));
		
		let config_path = self.asset.path().with_file_name("model.toml");
		match application.renderer.borrow_mut().load(TomlAsset::<ModelCharacterConfig>::at(&config_path)) {
			Ok(config) => *self.config.borrow_mut() = config.character,
			Err(err) => eprintln!("Unable to load character config: {}", err),
		}
		
		// The pelvis/root body must be simulated: the ragdoll joints connect the torso and legs to it.
		entity.rigid_body_mut(&mut application.physics.borrow_mut())
		      .set_body_type(rapier3d::dynamics::RigidBodyType::Dynamic, true);
		
		application.miku.set(self.as_cref());
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, delta_time: Duration) -> Result<()> {
		let Some(model) = self.model.using(entity) else { return Ok(()) };
		
		self.update_character(entity, model, application, delta_time);
		
		let mut model = model.state.borrow_mut();
		let has_character = self.character.borrow().is_some();
		
		if !has_character {
			for morph in model.morphs.iter_mut() {
				*morph = 0.0;
			}
		}
		
		let mut presets = vec![0.0; model.morphs.len()];
		let active = MORPH_PRESETS.iter().filter(|p| application.input.keyboard.pressed(num_key(p.0))).flat_map(|p| p.1.iter());
		for &id in active {
			presets[id as usize] = 1.0;
		}
		
		let mut anims = self.anims.borrow_mut();
		
		for (&bone, anim) in anims.0.iter_mut() {
			let value = anim.get();
			model.bones[bone as usize].anim_transform = value.to_superset();
		}
		
		for (&morph, anim) in anims.1.iter_mut() {
			let value = anim.get();
			let slot = &mut model.morphs[morph as usize];
			*slot = slot.max(value);
		}
		
		for (slot, preset) in model.morphs.iter_mut().zip(presets) {
			*slot = slot.max(preset);
		}
		
		let _ = delta_time;
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, _application: &Application) {
		let Ok(mut character) = self.character.try_borrow_mut() else { return };
		let Some(character) = character.as_mut() else {
			ui.label("No character");
			ui.end_row();
			return;
		};
		
		ui.label("Animation");
		ui.label(&character.anim.current);
		ui.end_row();
		ui.label("Body State");
		ui.label(&character.anim.body_state);
		ui.end_row();
		ui.label("Task");
		ui.label(character.active.name());
		ui.end_row();
		ui.label("Balance");
		ui.label(format!("{} (pin {:.2}, muscle {:.2})", character.anim.has_balance, character.puppet.pin_weight, character.puppet.muscle_weight));
		ui.end_row();
		ui.label("Happiness");
		ui.add(egui::Slider::new(&mut character.happiness, -2..=2));
		ui.end_row();
		ui.label("Tired");
		ui.checkbox(&mut character.tired, "");
		ui.end_row();
		ui.label("Voice Volume");
		ui.add(egui::Slider::new(&mut character.voice.volume, 0.0..=2.0));
		ui.end_row();
		ui.label("Chopsticks");
		ui.label(format!("Miku {:?}, You {:?}", character.chopsticks.miku, character.chopsticks.player));
		ui.end_row();
		
		ui.label("Actions");
		ui.horizontal_wrapped(|ui| {
			if ui.button("Chopsticks").clicked() {
				character.attempt_chopsticks();
			}
			if ui.button("Idle").clicked() {
				character.set_task(character::ActiveTask::Idle);
			}
			if ui.button("Ragdoll").clicked() {
				character.begin_ragdoll_mode(0.3, "FALLING_LOOP");
			}
			let mut draw = debug::get_flag_or_default::<bool>("DebugCharacterDraw");
			if ui.checkbox(&mut draw, "Debug Draw").changed() {
				debug::set_flag("DebugCharacterDraw", draw);
			}
		});
		ui.end_row();
		
		ui.label("Log");
		ui.vertical(|ui| {
			for line in character.debug_log.iter().rev().take(12) {
				ui.label(egui::RichText::new(line).monospace().small());
			}
		});
		ui.end_row();
	}
	
	fn on_inspect_extra(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		ui.separator();
		
		let model = match self.model.get(application) {
			Some(model) => model,
			None => return,
		};
		
		let miku_ent = self.entity(application);
		let id = ui.id().with(&miku_ent.name).with("Miku Gui");
		let selected_bone = application.gui_selections().selection.mmd_bone();
		
		let new_selection = ui.columns(3, |ui| {
			[
				gui::list(&mut ui[0],
				          id.with("bones"),
				          selected_bone,
				          model.state()
				               .bones
				               .iter()
				               .map(|bone| bone.name.as_str().into())
				               .enumerate()),
				gui::ragdoll(&mut ui[1], selected_bone, self, application),
				gui::list(&mut ui[2],
				          id.with("rigidbodies"),
				          selected_bone,
				          model.state()
				               .rigid_bodies(application)
				               .map(|rb| (rb.bone, rb.entity(application).name.as_str().into()))
				),
			].into_iter()
			 .find_map(|x| x)
		});
		
		ui.separator();
		
		ui.horizontal(|ui| {
			if ui.button("Freeze All").clicked() {
				let physics = &mut application.physics.borrow_mut();
				
				for rigid_body in model.state().rigid_bodies(application) {
					rigid_body.entity(application).freeze(physics);
				}
			}
			
			if ui.button("Unfreeze All").clicked() {
				let physics = &mut application.physics.borrow_mut();
				
				for rigid_body in model.state().rigid_bodies(application) {
					rigid_body.entity(application).unfreeze(physics);
				}
			}
			
			if ui.button("Reset Pose").clicked() {
				let root_pos = *miku_ent.state().position;
				
				for rigid_body in model.state().rigid_bodies(application) {
					*rigid_body.entity(application).state_mut().position = root_pos * rigid_body.rest_pos.get();
				}
			}
		});
		
		if let Some(bone) = new_selection {
			application.select((model.as_cref(), bone), false);
		}
	}
}
