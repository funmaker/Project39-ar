use rapier3d::geometry::Collider;


pub type PhysicsMode = mmd::pmx::rigid_body::PhysicsMode;

#[derive(Clone)]
pub struct ColliderDesc {
	pub name: String,
	pub bone: usize,
	pub collider: Collider,
	pub move_attenuation: f32,
	pub rotation_damping: f32,
	pub repulsion: f32,
	pub fiction: f32,
	pub physics_mode: PhysicsMode,
}

impl ColliderDesc {
	pub fn new(name: impl Into<String>,
	           bone: usize,
	           collider: Collider,
	           move_attenuation: f32,
	           rotation_damping: f32,
	           repulsion: f32,
	           fiction: f32,
	           physics_mode: PhysicsMode) -> Self {
		ColliderDesc {
			name: name.into(),
			bone,
			collider,
			move_attenuation,
			rotation_damping,
			repulsion,
			fiction,
			physics_mode,
		}
	}
}
