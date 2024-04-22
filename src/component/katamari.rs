use std::time::Duration;
use anyhow::Result;
use rapier3d::geometry::ColliderBuilder;
use rapier3d::prelude::{RigidBodyType, SharedShape};
use smallvec::SmallVec;

use crate::application::{Entity, Application};
use crate::math::PI;
use crate::utils::ColliderEx;
use super::{Component, ComponentBase, ComponentInner, ComponentRef};
use super::physics::collider::ColliderComponent;


#[derive(ComponentBase)]
pub struct Katamari {
	#[inner] inner: ComponentInner,
	collider: ComponentRef<ColliderComponent>,
}

impl Katamari {
	pub fn new() -> Self {
		Katamari {
			inner: ComponentInner::new_norender(),
			collider: ComponentRef::null(),
		}
	}
}

impl Component for Katamari {
	fn start(&self, entity: &Entity, _application: &Application) -> Result<()> {
		self.collider.set(
			entity.add_component(ColliderComponent::new(
				ColliderBuilder::ball(0.6)
				                .density(10000.0)
				                .build()
			))
		);
		
		Ok(())
	}
	
	fn tick(&self, entity: &Entity, application: &Application, _delta_time: Duration) -> Result<()> {
		if let Some(collider) = self.collider.using(entity) {
			let physics = &mut *application.physics.borrow_mut();
			let volume = collider.inner(physics).volume();
			let pos = *entity.state().position;
			
			let mut to_eat = SmallVec::<[_; 16]>::new();
			
			for contact in physics.narrow_phase.contact_pairs_with(collider.handle()) {
				let other = if contact.collider1 == collider.handle() { contact.collider2 } else { contact.collider1 };
				
				if let Some(other) = physics.collider_set.get(other) {
					if let Some(other_ent) = other.component_ref().entity().get(application) {
						let other_rb = other_ent.rigid_body(physics);
						let other_volume = other.volume();
						
						if other_rb.body_type() == RigidBodyType::Dynamic && other_volume < volume * 0.33 {
							to_eat.push((other_ent, other_volume));
						}
					}
				}
			}
			
			for (other_ent, other_volume) in to_eat.iter() {
				let other_rb = other_ent.rigid_body_mut(physics);
				
				other_ent.set_parent(entity.as_ref(), true, application);
				other_rb.set_body_type(RigidBodyType::KinematicPositionBased, true);
				
				for collider in other_ent.iter_component_by_type::<ColliderComponent>() {
					collider.remove();
				}
				
				let new_radius = f32::powf((volume + other_volume) / PI / 4.0 * 3.0, 1.0 / 3.0);
				
				let other_pos = *other_ent.state().position;
				let rel_pos = other_pos.translation.vector - pos.translation.vector;
				other_ent.state_mut().position.translation.vector = pos.translation.vector + rel_pos.normalize() * new_radius;
				
				collider.inner_mut(physics)
				        .set_shape(SharedShape::ball(new_radius));
			}
		}
		
		Ok(())
	}
}
