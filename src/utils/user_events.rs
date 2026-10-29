use crate::application::Application;

#[derive(Debug)]
pub enum UserEvent {
	Initialized(Application),
}
