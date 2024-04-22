#![allow(dead_code)]
use std::cell::{Ref, RefCell, RefMut};
use std::iter;

pub fn ref_cell_iter<'c, T, U: 'c>(cell: &'c RefCell<T>, map: impl Fn(&T) -> &[U] + 'c) -> impl Iterator<Item=Ref<U>> + 'c {
	let mut i = 0;
	
	iter::from_fn(move || {
		let arr = Ref::map(cell.borrow(), |val| map(val));
		
		if i < arr.len() {
			i += 1;
			Some(Ref::map(arr, |arr| &arr[i - 1]))
		} else {
			None
		}
	})
}

pub fn ref_cell_mut<'c, T, U: 'c>(cell: &'c RefCell<T>, map: impl Fn(&mut T) -> &mut [U] + 'c) -> impl Iterator<Item=RefMut<U>> + 'c {
	let mut i = 0;
	
	iter::from_fn(move || {
		let arr = RefMut::map(cell.borrow_mut(), |val| map(val));
		
		if i < arr.len() {
			i += 1;
			Some(RefMut::map(arr, |arr| &mut arr[i - 1]))
		} else {
			None
		}
	})
}
