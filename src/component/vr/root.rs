use std::time::Duration;
use std::collections::BTreeMap;
use std::cell::RefCell;
use egui::Ui;
use rapier3d::dynamics::RigidBodyType;
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedDeviceClass, TrackedDeviceIndex, TrackedControllerRole};
use openvr_sys::ETrackedDeviceProperty_Prop_RenderModelName_String;
use rapier3d::prelude::RigidBodyBuilder;
use image::{DynamicImage, ImageBuffer};
use openvr::render_models;
use rapier3d::geometry::{ColliderBuilder, InteractionGroups};

use crate::application::{Entity, EntityRef, Application, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::component::comedy::Comedy;
use crate::component::model::simple::{SimpleModel, Vertex};
use crate::component::pov::PoV;
use crate::component::hand::HandComponent;
use crate::component::parent::Parent;
use crate::renderer::assets_manager::TextureBundle;
use crate::math::Isometry3;
use crate::utils::ExUi;
use super::VrTracked;

#[derive(ComponentBase)]
pub struct VrRoot {
	#[inner] inner: ComponentInner,
	entities: RefCell<BTreeMap<TrackedDeviceIndex, EntityRef>>,
}

impl VrRoot {
	pub fn new() -> Self {
		VrRoot {
			inner: ComponentInner::new_norender(),
			entities: RefCell::new(BTreeMap::new()),
		}
	}
}

impl Component for VrRoot {
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
							let renderer = &mut *application.renderer.borrow_mut();
							let class = vr.system.tracked_device_class(tracked_id);
							let vertices: Vec<Vertex> = model.vertices().iter().map(Into::into).collect();
							let indices: Vec<u16> = model.indices().iter().copied().map(Into::into).collect();
							let size = texture.dimensions();
							let image = DynamicImage::ImageRgba8(ImageBuffer::from_raw(size.0 as u32, size.1 as u32, texture.data().into()).unwrap());
							let texture = TextureBundle::from_raw_simple(image, renderer)?;
							
							let model = SimpleModel::new(
								&vertices,
								&indices,
								texture,
								renderer,
							)?;
							
							let mut entity = Entity::builder(format!("{:?}", class))
								.rigid_body_type(RigidBodyType::KinematicPositionBased)
								.rigid_body(RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
								                             .additional_mass(1000.0)
								                             .build())
								.component(model)
								.component(VrTracked::new(tracked_id, self.as_cref()))
								.tag("NoGrab", true)
								.collider_from_aabb(1000.0);
							
							if class == TrackedDeviceClass::HMD {
								entity = entity.tag("NoGrab", false)
								               .tag("Head", true)
								               .tag("CloseHide", true)
								               .component(PoV::new(true));
							}
							
							match vr.system.get_controller_role_for_tracked_device_index(tracked_id) {
								Some(TrackedControllerRole::LeftHand) => entity = entity.component(HandComponent::new(Hand::Left)),
								Some(TrackedControllerRole::RightHand) => entity = entity.component(HandComponent::new(Hand::Right)),
								_ => {}
							}
							
							let entity = application.add_entity(entity.build());
							
							if class == TrackedDeviceClass::HMD {
								application.add_entity(
									Entity::builder("Eye")
										.rigid_body_type(RigidBodyType::KinematicPositionBased)
										.collider(ColliderBuilder::ball(0.05)
											.collision_groups(InteractionGroups::none())
											.build())
										.component(Comedy::new(renderer)?)
										.component(Parent::new(entity.clone(), Isometry3::translation(0.06, 0.02, -0.085)))
										.tag("CloseHide", true)
										.build()
								);
								
								application.add_entity(
									Entity::builder("Eye")
										.rigid_body_type(RigidBodyType::KinematicPositionBased)
										.collider(ColliderBuilder::ball(0.05)
											.collision_groups(InteractionGroups::none())
											.build())
										.component(Comedy::new(renderer)?)
										.component(Parent::new(entity.clone(), Isometry3::translation(-0.06, 0.02, -0.085)))
										.tag("CloseHide", true)
										.build()
								);
							}
							
							entities.insert(tracked_id, entity);
							
							println!("Loaded {:?}", vr.system.tracked_device_class(tracked_id));
						}
					}
				}
			}
		}
		
		Ok(())
	}
	
	fn on_inspect(&self, _entity: &Entity, ui: &mut Ui, application: &Application) {
		if let Some(vr) = application.vr.as_ref().and_then(|vr| vr.try_lock().ok()) {
			for (id, entity) in self.entities.borrow_mut().iter() {
				ui.inspect_row(format!("{:?}", vr.system.tracked_device_class(*id)),
				               entity,
				               application);
			}
		}
	}
}

impl From<&render_models::Vertex> for Vertex {
	fn from(vertex: &render_models::Vertex) -> Self {
		Vertex::new(vertex.position, vertex.normal, vertex.texture_coord)
	}
}
