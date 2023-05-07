use std::fmt::{Debug, Formatter};
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, Instant};
use rand::{Rng, thread_rng};

use crate::math::Rot3;


#[derive(Debug, Clone)]
pub struct ProcAnim<V> {
	from: V,
	frames: Vec<Frame<V>>,
	frame_start: Instant,
	frame_cur: usize,
	frame_dur: f32,
	repeat: bool,
	stopped: bool,
	overdrive: Option<Box<ProcAnim<V>>>,
}

impl<V: Interpolate + Clone> ProcAnim<V> {
	pub fn new(initial: V) -> Self {
		ProcAnim {
			from: initial,
			frames: Vec::new(),
			frame_start: Instant::now(),
			frame_cur: 0,
			frame_dur: 0.0,
			repeat: false,
			stopped: false,
			overdrive: None,
		}
	}
	
	pub fn wait(mut self, time: impl Into<DurationRange>) -> Self {
		let end_state = self.end_state();
		let duration = time.into();
		
		if self.frames.is_empty() { self.frame_dur = duration.sample(); }
		
		self.frames.push(Frame {
			target: end_state,
			duration,
			easing: Easing::Step,
		});
		
		self
	}
	
	pub fn anim(mut self, target: V, time: impl Into<DurationRange>, easing: Easing) -> Self {
		let duration = time.into();
		
		if self.frames.is_empty() { self.frame_dur = duration.sample(); }
		
		self.frames.push(Frame {
			target,
			duration,
			easing,
		});
		
		self
	}
	
	pub fn repeat(self) -> Self {
		ProcAnim {
			repeat: true,
			..self
		}
	}
	
	pub fn no_repeat(self) -> Self {
		ProcAnim {
			repeat: false,
			..self
		}
	}
	
	pub fn no_autoplay(self) -> Self {
		ProcAnim {
			stopped: true,
			..self
		}
	}
	
	pub fn end_state(&self) -> V {
		self.frames.last()
		    .map(|frame| frame.target.clone())
		    .unwrap_or(self.from.clone())
	}
	
	pub fn get(&mut self) -> V {
		if let Some(mut overdrive) = self.overdrive.take() {
			let result = overdrive.get();
			
			if overdrive.stopped() {
				self.from = result;
				self.play();
			} else {
				self.overdrive = Some(overdrive);
				return result;
			}
		}
		
		if self.stopped || self.frames.is_empty() {
			return self.from.clone();
		}
		
		let mut elapsed = self.frame_start.elapsed().as_secs_f32();
		
		while elapsed > self.frame_dur {
			elapsed -= self.frame_dur;
			self.from = self.frames[self.frame_cur].at(1.0, &self.from);
			self.frame_start += Duration::from_secs_f32(self.frame_dur);
			self.frame_cur = self.frame_cur + 1;
			
			if self.frame_cur >= self.frames.len() {
				self.frame_cur = 0;
				
				if !self.repeat {
					self.stopped = true;
					return self.from.clone();
				}
			}
			
			self.frame_dur = self.frames[self.frame_cur].duration.sample();
		}
		
		self.frames[self.frame_cur].at(elapsed / self.frame_dur, &self.from)
	}
	
	pub fn stopped(&self) -> bool { self.stopped }
	
	pub fn stop(&mut self) {
		if self.stopped { return; }
		
		self.from = self.get();
		self.frame_cur = 0;
		self.stopped = true;
	}
	
	pub fn play(&mut self) {
		if !self.stopped { return; }
		
		self.stopped = false;
		self.frame_start = Instant::now();
	}
	
	pub fn overdrive(&mut self, mut anim: ProcAnim<V>) {
		if let Some(overdive) = &mut self.overdrive {
			overdive.overdrive(anim);
		} else {
			anim.from = self.get();
			self.stop();
			self.overdrive = Some(Box::new(anim));
		}
	}
}

#[derive(Debug, Clone)]
struct Frame<V> {
	target: V,
	duration: DurationRange,
	easing: Easing,
}

impl<V: Interpolate> Frame<V> {
	fn at(&self, time: f32, from: &V) -> V {
		Interpolate::interpolate(&from, &self.target, self.easing.ease(time))
	}
}

#[allow(unused)]
#[derive(Clone)]
pub enum Easing {
	Step,
	Linear,
	Ease,
	EaseIn,
	EaseOut,
	EaseInOut,
	Bezier(f32, f32, f32, f32),
	Custom(Arc<dyn Fn(f32) -> f32>),
}

impl Debug for Easing {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match self {
			Easing::Step => write!(f, "Step"),
			Easing::Linear => write!(f, "Linear"),
			Easing::Ease => write!(f, "Ease"),
			Easing::EaseIn => write!(f, "EaseIn"),
			Easing::EaseOut => write!(f, "EaseOut"),
			Easing::EaseInOut => write!(f, "EaseInOut"),
			Easing::Bezier(a, b, c, d) => write!(f, "Bezier({}, {}, {}, {})", a, b, c, d),
			Easing::Custom(_) => write!(f, "Custom"),
		}
	}
}

fn bezier(x: f32, a: f32, b: f32, c: f32, d: f32) -> f32 {
	        a * (1.0 - x).powi(3)
	+ 3.0 * b * (1.0 - x).powi(2) * x
	+ 3.0 * c * (1.0 - x) * x.powi(2)
	+       d * x.powi(3)
}

impl Easing {
	pub fn ease(&self, t: f32) -> f32 {
		match self {
			Easing::Step => 1.0,
			Easing::Linear => t,
			Easing::Ease => bezier(t, 0.0, 0.4, 0.9, 1.0),
			Easing::EaseIn => bezier(t, 0.0, 0.2, 0.25, 1.0),
			Easing::EaseOut => bezier(t, 0.0, 0.7, 1.0, 1.0),
			Easing::EaseInOut => bezier(t, 0.0, 0.1, 0.9, 1.0),
			Easing::Bezier(a, b, c, d) => bezier(t, *a, *b, *c, *d),
			Easing::Custom(fun) => fun(t),
		}
	}
}

pub trait Interpolate: Sized {
	fn interpolate(from: &Self, to: &Self, time: f32) -> Self;
}

impl Interpolate for f32 {
	fn interpolate(from: &Self, to: &Self, time: f32) -> Self {
		from + (to - from) * time
	}
}

impl Interpolate for Rot3 {
	fn interpolate(from: &Self, to: &Self, time: f32) -> Self {
		from.slerp(to, time)
	}
}

#[derive(Clone, Debug)]
pub enum DurationRange {
	Fixed(f32),
	Range(Range<f32>),
}

impl DurationRange {
	fn sample(&self) -> f32 {
		match self {
			DurationRange::Fixed(val) => *val,
			DurationRange::Range(range) => thread_rng().gen_range(range.clone()),
		}
	}
}

impl From<f32> for DurationRange {
	fn from(fixed: f32) -> Self {
		DurationRange::Fixed(fixed)
	}
}

impl From<Range<f32>> for DurationRange {
	fn from(range: Range<f32>) -> Self {
		DurationRange::Range(range)
	}
}
