use std::cell::{Cell, RefCell};
use std::rc::Rc;
use rodio::{MixerDeviceSink, DeviceSinkBuilder, SpatialPlayer};

use crate::math::{Isometry3, Point3, Vec3};
use crate::renderer::assets_manager::Sound;
use super::{Application, EntityRef};


/// Distance between the listener's ears, in meters.
const EAR_DISTANCE: f32 = 0.2;

/// 3D positional audio output. Sounds are played with `play` from a position in the world,
/// optionally following an entity, and are heard from the player's point of view.
///
/// If no audio device is available, the audio is silently disabled.
pub struct Audio {
	sink: Option<MixerDeviceSink>,
	emitters: RefCell<Vec<Rc<Emitter>>>,
	listener: Cell<Isometry3>,
	/// Master volume.
	pub volume: Cell<f32>,
}

struct Emitter {
	player: SpatialPlayer,
	/// Entity the sound follows, with offset in the entity's space.
	follow: Option<(EntityRef, Vec3)>,
	position: Cell<Point3>,
	volume: Cell<f32>,
}

/// Handle to a playing sound. Dropping the handle does not stop the sound.
#[derive(Clone)]
pub struct SoundHandle(Option<Rc<Emitter>>);

impl Audio {
	pub fn new() -> Self {
		let sink = match DeviceSinkBuilder::open_default_sink() {
			Ok(mut sink) => {
				sink.log_on_drop(false);
				Some(sink)
			},
			Err(err) => {
				eprintln!("Unable to open audio output, audio disabled: {}", err);
				None
			},
		};

		Audio {
			sink,
			emitters: RefCell::new(vec![]),
			listener: Cell::new(Isometry3::identity()),
			volume: Cell::new(1.0),
		}
	}

	/// Returns `true` if an audio device is open.
	#[allow(dead_code)]
	pub fn enabled(&self) -> bool {
		self.sink.is_some()
	}

	/// Plays a sound at a fixed position.
	pub fn play(&self, sound: &Sound, position: Point3, volume: f32) -> SoundHandle {
		self.spawn(sound, position, None, volume)
	}

	/// Plays a sound that follows an entity, at `offset` in its local space.
	pub fn play_on(&self, sound: &Sound, entity: impl Into<EntityRef>, offset: Vec3, volume: f32, application: &Application) -> SoundHandle {
		let entity = entity.into();
		let position = entity.get(application)
		                     .map(|ent| ent.state().position.transform_point(&offset.into()))
		                     .unwrap_or(offset.into());

		self.spawn(sound, position, Some((entity, offset)), volume)
	}

	fn spawn(&self, sound: &Sound, position: Point3, follow: Option<(EntityRef, Vec3)>, volume: f32) -> SoundHandle {
		let Some(sink) = &self.sink else { return SoundHandle(None) };

		let (left, right) = self.ears();
		let player = SpatialPlayer::connect_new(sink.mixer(), position.coords.into(), left, right);
		player.set_volume(volume * self.volume.get());
		player.append(sound.buffer.clone());

		let emitter = Rc::new(Emitter {
			player,
			follow,
			position: Cell::new(position),
			volume: Cell::new(volume),
		});

		self.emitters.borrow_mut().push(emitter.clone());

		SoundHandle(Some(emitter))
	}

	fn ears(&self) -> ([f32; 3], [f32; 3]) {
		let listener = self.listener.get();
		let left = listener.transform_point(&point!(-EAR_DISTANCE / 2.0, 0.0, 0.0));
		let right = listener.transform_point(&point!(EAR_DISTANCE / 2.0, 0.0, 0.0));

		(left.coords.into(), right.coords.into())
	}

	/// Moves the listener and following sounds, and drops finished sounds. Called once per frame.
	pub fn update(&self, listener: Isometry3, application: &Application) {
		self.listener.set(listener);
		let (left, right) = self.ears();
		let master = self.volume.get();

		self.emitters.borrow_mut().retain(|emitter| {
			if emitter.player.empty() {
				return false;
			}

			if let Some((entity, offset)) = &emitter.follow
			&& let Some(entity) = entity.get(application) {
				emitter.position.set(entity.state().position.transform_point(&(*offset).into()));
			}

			emitter.player.set_emitter_position(emitter.position.get().coords.into());
			emitter.player.set_left_ear_position(left);
			emitter.player.set_right_ear_position(right);
			emitter.player.set_volume(emitter.volume.get() * master);

			true
		});
	}
}

impl SoundHandle {
	/// Handle not associated with any sound.
	pub fn none() -> Self {
		SoundHandle(None)
	}

	/// Returns `true` while the sound is playing.
	pub fn playing(&self) -> bool {
		self.0.as_ref().map_or(false, |emitter| !emitter.player.empty())
	}

	pub fn stop(&self) {
		if let Some(emitter) = &self.0 {
			emitter.player.stop();
		}
	}

	pub fn set_volume(&self, volume: f32) {
		if let Some(emitter) = &self.0 {
			emitter.volume.set(volume);
		}
	}

	pub fn volume(&self) -> f32 {
		self.0.as_ref().map_or(0.0, |emitter| emitter.volume.get())
	}
}

impl Default for SoundHandle {
	fn default() -> Self {
		SoundHandle::none()
	}
}
