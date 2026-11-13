use std::collections::HashMap;
use rand::Rng;

use crate::application::SoundHandle;
use crate::math::Vec3;
use crate::renderer::assets_manager::{Sound, SoundAsset};
use super::super::super::model::mmd::BodyPart;
use super::{Character, Ctx};


/// Character's voice (OpenViva's `Lily.Voice`): plays voice lines from the imported sound sets.
pub struct Voice {
	handle: SoundHandle,
	last_line: String,
	/// Stop speaking when the animation changes.
	pub bound_to_animation: bool,
	next_random_speak: f32,
	/// Last played clip of each line, to avoid repeating it.
	last_clip: HashMap<String, usize>,
	sounds: HashMap<String, Option<Sound>>,
	/// Mouth movement, from the voice's loudness.
	pub mouth: f32,
	pub volume: f32,
}

impl Voice {
	pub fn new() -> Self {
		Voice {
			handle: SoundHandle::none(),
			last_line: String::new(),
			bound_to_animation: false,
			next_random_speak: 0.0,
			last_clip: HashMap::new(),
			sounds: HashMap::new(),
			mouth: 0.0,
			volume: 1.0,
		}
	}
}

impl Character {
	fn load_sound(&mut self, path: &str, ctx: &Ctx) -> Option<Sound> {
		if let Some(sound) = self.voice.sounds.get(path) {
			return sound.clone();
		}
		let sound = match ctx.app.renderer.borrow_mut().load(SoundAsset::at(path)) {
			Ok(sound) => Some(sound),
			Err(err) => {
				eprintln!("{}", err);
				None
			},
		};
		self.voice.sounds.insert(path.to_string(), sound.clone());
		sound
	}

	pub fn is_speaking(&self, line: &str) -> bool {
		self.voice.handle.playing() && self.voice.last_line == line
	}

	pub fn is_speaking_at_all(&self) -> bool {
		self.voice.handle.playing()
	}

	pub fn stop_speaking(&mut self) {
		self.voice.handle.stop();
	}

	/// Speaks a random clip of a voice line (eg. `HEADPAT_HAPPY`).
	pub fn speak(&mut self, line: &str, bind_to_animation: bool, ctx: &Ctx) {
		let Some(clips) = self.voice_data.lines.get(line).cloned() else { return };
		if clips.is_empty() { return; }

		let mut index = rand::thread_rng().gen_range(0..clips.len());
		if clips.len() > 1 && self.voice.last_clip.get(line) == Some(&index) {
			index = (index + 1) % clips.len();
		}
		self.voice.last_clip.insert(line.to_string(), index);

		let Some(sound) = self.load_sound(&clips[index], ctx) else { return };
		self.voice.handle.stop();
		self.voice.last_line = line.to_string();
		self.voice.bound_to_animation = bind_to_animation;

		let head = self.body(BodyPart::Head).map(|b| b.entity.clone());
		self.voice.handle = match head {
			Some(head) => ctx.app.audio.play_on(&sound, head, Vec3::zeros(), self.voice.volume, ctx.app),
			None => ctx.app.audio.play(&sound, self.head_position(), self.voice.volume),
		};
		self.log(format!("Speak {line}"));
	}

	pub fn speak_at_random_intervals(&mut self, line: &str, min_interval: f32, extra: f32, ctx: &Ctx) {
		if !self.is_speaking_at_all() && self.time > self.voice.next_random_speak {
			self.voice.next_random_speak = self.time + min_interval + Self::random() * extra;
			self.speak(line, false, ctx);
		}
	}

	/// Plays a sound effect set (eg. `get_up`) at the character.
	pub fn play_sfx(&mut self, set: &str, volume: f32, ctx: &Ctx) {
		let Some(clips) = self.voice_data.sfx.get(set).cloned() else { return };
		if clips.is_empty() { return; }
		let index = rand::thread_rng().gen_range(0..clips.len());
		if let Some(sound) = self.load_sound(&clips[index], ctx) {
			ctx.app.audio.play(&sound, self.floor_pos() + Vec3::y() * 0.3, volume);
		}
	}

	pub(super) fn update_voice(&mut self, ctx: &Ctx) {
		// Simple lip flap while speaking.
		let target = if self.is_speaking_at_all() {
			0.35 + 0.35 * (self.time * 17.0).sin().abs() * (self.time * 5.3).sin().abs()
		} else {
			0.0
		};
		self.voice.mouth += (target - self.voice.mouth) * (ctx.dt * 20.0).min(1.0);
	}
}
