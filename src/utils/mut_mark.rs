use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use egui::Ui;

use super::{InspectMut, Inspect};


/// Wrapper that tracks whether the value was mutably accessed (through `DerefMut`) since the
/// mark was last cleared.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct MutMark<T> {
	mutated: bool,
	inner: T,
}

impl<T> MutMark<T> {
	pub fn new(inner: T) -> Self {
		MutMark {
			inner,
			mutated: false,
		}
	}
	
	pub fn new_mutated(inner: T) -> Self {
		MutMark {
			inner,
			mutated: false,
		}
	}
	
	/// Marks the value as mutated.
	pub fn mark_mut(&mut self) {
		self.mutated = true;
	}
	
	/// Returns `true` if the value was mutated since the last `clear_mut`.
	pub fn was_mut(&self) -> bool {
		self.mutated
	}
	
	/// Clears the mutation mark, returning the previous state.
	pub fn clear_mut(&mut self) -> bool {
		std::mem::replace(&mut self.mutated, false)
	}
}

impl<T> From<T> for MutMark<T> {
	fn from(inner: T) -> Self {
		MutMark::new(inner)
	}
}

impl<T> Deref for MutMark<T> {
	type Target = T;
	
	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl<T> DerefMut for MutMark<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.mutated = true;
		&mut self.inner
	}
}

impl<T: Display> Display for MutMark<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.inner.fmt(f)
	}
}

impl<T: InspectMut + Clone + PartialEq> Inspect for &mut MutMark<T> {
	type Options<'a> = T::Options<'a>;
	
	fn inspect_ui(self, ui: &mut Ui, options: T::Options<'_>) {
		use crate::utils::gui::ExUi;
		
		let mut value = self.clone();
		
		ui.inspect(&mut value, options);
		
		if value != **self {
			**self = value;
		}
	}
}
