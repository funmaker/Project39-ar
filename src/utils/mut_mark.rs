use std::fmt::{Display, Formatter};
use std::ops::{Deref, DerefMut};
use egui::Ui;

use super::{InspectMut, Inspect};


#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Default)]
pub struct MutMark<T> {
	pub mutated: bool,
	inner: T,
}

impl<T> MutMark<T> {
	pub fn new(inner: T) -> Self {
		MutMark {
			inner,
			mutated: false,
		}
	}
	
	pub fn reset(&mut self) {
		self.mutated = false;
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
