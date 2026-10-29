use crate::application::Key;


pub fn num_key(num: usize) -> Key {
	match num {
		0 => Key::Digit0,
		1 => Key::Digit1,
		2 => Key::Digit2,
		3 => Key::Digit3,
		4 => Key::Digit4,
		5 => Key::Digit5,
		6 => Key::Digit6,
		7 => Key::Digit7,
		8 => Key::Digit8,
		9 => Key::Digit9,
		n => panic!("Invalid numeric key: {}", n),
	}
}
