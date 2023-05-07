use crate::application::Key;


pub fn num_key(num: usize) -> Key {
	match num {
		0 => Key::Key0,
		1 => Key::Key1,
		2 => Key::Key2,
		3 => Key::Key3,
		4 => Key::Key4,
		5 => Key::Key5,
		6 => Key::Key6,
		7 => Key::Key7,
		8 => Key::Key8,
		9 => Key::Key9,
		n => panic!("Invalid numeric key: {}", n),
	}
}
