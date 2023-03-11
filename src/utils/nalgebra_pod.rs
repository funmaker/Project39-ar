use std::fmt::{Debug, Display, Formatter};
use std::ops::{Deref, DerefMut};

#[repr(transparent)]
pub struct NgPod<T>(T);

impl<T: Clone> Clone for NgPod<T> {
	fn clone(&self) -> Self {
		NgPod(self.0.clone())
	}
}

impl<T: Copy> Copy for NgPod<T> {}

impl<T: Default> Default for NgPod<T> {
	fn default() -> Self {
		NgPod(T::default())
	}
}

impl<T> From<T> for NgPod<T> {
	fn from(inner: T) -> Self {
		NgPod(inner)
	}
}

impl<T: Debug> Debug for NgPod<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

impl <T: Display> Display for NgPod<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

impl<T> Deref for NgPod<T> {
	type Target = T;
	
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> DerefMut for NgPod<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

macro_rules! ng_pod_impl {
	( $( $type:ty ),* ) => {
		$(
			unsafe impl bytemuck::Zeroable for crate::utils::NgPod<$type> {}
			unsafe impl bytemuck::Pod for crate::utils::NgPod<$type> {}
		)*
	}
}

pub(crate) use ng_pod_impl;
