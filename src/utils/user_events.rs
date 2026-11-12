use crate::application::Application;

/// Custom winit events.
#[derive(Debug)]
pub enum UserEvent {
	/// The application finished initializing and is ready to handle events.
	Initialized(Application),
}
