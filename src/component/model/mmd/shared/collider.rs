use rapier3d::geometry::Collider;


pub type PhysicsMode = mmd::pmx::rigid_body::PhysicsMode;

/// Rigid body of an MMD model as loaded from the file.
#[derive(Clone)]
pub struct ColliderDesc {
	pub name: String,
	/// Bone the rigid body is attached to.
	pub bone: usize,
	/// Collider positioned in model space.
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
