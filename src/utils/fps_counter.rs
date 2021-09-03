use std::time::Instant;

pub struct FpsCounter<const N: usize> {
	measurements: [Instant; N],
	current: usize,
}

impl<const N: usize> FpsCounter<N> {
	pub fn new() -> Self {
		FpsCounter {
			measurements: [Instant::now(); N],
			current: 0,
		}
	}
	
	pub fn tick(&mut self) {
		self.measurements[self.current] = Instant::now();
		self.current = (self.current + 1) % N;
	}
	
	pub fn fps(&self) -> f32 {
		let last = self.current;
		let first = self.current.checked_sub(1).unwrap_or(N - 1);
		let duration = self.measurements[first].duration_since(self.measurements[last]).as_secs_f32();
		
		if duration < f32::EPSILON {
			0.0
		} else {
			(N - 1) as f32 / duration
		}
	}
}
