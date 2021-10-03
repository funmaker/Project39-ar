use std::time::Duration;
use std::collections::BTreeMap;
use std::cell::RefCell;
use rapier3d::dynamics::RigidBodyType;
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedDeviceClass, TrackedDeviceIndex, TrackedControllerRole};
use openvr_sys::ETrackedDeviceProperty_Prop_RenderModelName_String;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder};

use crate::application::{Entity, EntityRef, Application, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::component::model::SimpleModel;
use crate::component::pov::PoV;
use crate::math::Vec3;
use super::VrTracked;

#[derive(ComponentBase)]
pub struct VrSpawner {
	#[inner] inner: ComponentInner,
	entities: RefCell<BTreeMap<TrackedDeviceIndex, EntityRef>>,
}

impl VrSpawner {
	pub fn new() -> Self {
		VrSpawner {
			inner: ComponentInner::new(),
			entities: RefCell::new(BTreeMap::new()),
		}
	}
}

impl Component for VrSpawner {
	fn tick(&self, _entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let vr = application.vr.as_ref().expect("VR has not been initialized.").lock().unwrap();
		let mut entities = self.entities.borrow_mut();
		
		entities.drain_filter(|_, entref| entref.get(application).is_none());
		
		for tracked_id in 0..MAX_TRACKED_DEVICE_COUNT as u32 {
			if vr.system.is_tracked_device_connected(tracked_id) {
				if entities.get(&tracked_id).is_none() {
					let model_name = vr.system.string_tracked_device_property(tracked_id, ETrackedDeviceProperty_Prop_RenderModelName_String)?;
					let model = vr.render_models.load_render_model(&vr.system.string_tracked_device_property(tracked_id, ETrackedDeviceProperty_Prop_RenderModelName_String)?);
					
					if let Err(err) = model {
						dprintln!("Failed to load model \"{}\": {}", model_name.to_string_lossy(), err);
					} else if let Ok(Some(model)) = model {
						if let Some(texture) = vr.render_models.load_texture(model.diffuse_texture_id().unwrap())? {
							let class = vr.system.tracked_device_class(tracked_id);
							
							let mut aabb = (Vec3::zeros(), Vec3::zeros());
							
							for vertex in model.vertices() {
								let vertex = Vec3::from(vertex.position);
								
								if vertex.x < aabb.0.x { aabb.0.x = vertex.x; }
								if vertex.y < aabb.0.y { aabb.0.y = vertex.y; }
								if vertex.z < aabb.0.z { aabb.0.z = vertex.z; }
								if vertex.x > aabb.1.x { aabb.1.x = vertex.x; }
								if vertex.y > aabb.1.y { aabb.1.y = vertex.y; }
								if vertex.z > aabb.1.z { aabb.1.z = vertex.z; }
							}
							
							let center = (aabb.1 + aabb.0) / 2.0;
							let hsize = (aabb.1 - aabb.0) / 2.0;
							
							let mut entity = Entity::builder(format!("{:?}", class))
								.rigid_body_type(RigidBodyType::KinematicPositionBased)
								.collider(ColliderBuilder::cuboid(hsize.x, hsize.y, hsize.z)
								                          .translation(center.into())
								                          .build())
								.rigid_body(RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
								                             .additional_mass(1000.0)
								                             .build())
								.component(SimpleModel::<u16>::from_openvr(model, texture, &mut *application.renderer.borrow_mut())?)
								.component(VrTracked::new(tracked_id));
							
							if class == TrackedDeviceClass::HMD {
								entity = entity.hidden(true)
								               .component(PoV::new());
							}
							
							match vr.system.get_controller_role_for_tracked_device_index(tracked_id) {
								Some(TrackedControllerRole::LeftHand) => entity = entity.tag("Hand", Hand::Left),
								Some(TrackedControllerRole::RightHand) => entity = entity.tag("Hand", Hand::Right),
								_ => {}
							}
							
							let entity = application.add_entity(entity.build());
							
							entities.insert(tracked_id, entity);
							
							println!("Loaded {:?}", vr.system.tracked_device_class(tracked_id));
						}
					}
				}
			}
		}
		
		Ok(())
	}
}
