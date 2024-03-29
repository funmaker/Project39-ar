use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;
use egui::Ui;
use image::{DynamicImage, ImageBuffer};
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedDeviceClass, TrackedDeviceIndex, TrackedControllerRole, render_models};
use openvr_sys::ETrackedDeviceProperty_Prop_RenderModelName_String;
use rapier3d::dynamics::RigidBodyType;
use rapier3d::geometry::{ColliderBuilder, InteractionGroups};
use rapier3d::prelude::RigidBodyBuilder;

use crate::application::{Entity, EntityRef, Application, Hand};
use crate::math::Isometry3;
use crate::renderer::assets_manager::TextureBundle;
use crate::utils::ExUi;
use super::super::{Component, ComponentBase, ComponentInner, ComponentError, ComponentRef};
use super::super::comedy::Comedy;
use super::super::hand::HandComponent;
use super::super::model::simple::{SimpleModel, Vertex};
use super::super::pov::PoV;
use super::{VrTracked, VrIk};


#[derive(ComponentBase)]
pub struct VrRoot {
	#[inner] inner: ComponentInner,
	entities: RefCell<HashMap<TrackedDeviceIndex, EntityRef>>,
	ik: ComponentRef<VrIk>,
}

impl VrRoot {
	pub fn new() -> Self {
		VrRoot {
			inner: ComponentInner::new_norender(),
			entities: RefCell::new(HashMap::new()),
			ik: ComponentRef::null(),
		}
	}
}

impl Component for VrRoot {
	fn start(&self, entity: &Entity, _application: &Application) -> Result<(), ComponentError> {
		self.ik.set(entity.add_component(VrIk::new(EntityRef::null(), EntityRef::null(), EntityRef::null())));
		
		Ok(())
	}
	
	fn tick(&self, _entity: &Entity, application: &Application, _delta_time: Duration) -> Result<(), ComponentError> {
		let vr = application.vr.as_ref().expect("VR has not been initialized.").lock().unwrap();
		let mut entities = self.entities.borrow_mut();
		
		entities.retain(|_, entref| entref.get(application).is_some());
		
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
										.position(Isometry3::translation(0.06, 0.02, -0.085))
										.parent(entity.clone(), true)
										.collider(ColliderBuilder::ball(0.05)
											.collision_groups(InteractionGroups::none())
											.build())
										.component(Comedy::new(renderer)?)
										.tag("CloseHide", true)
										.build()
								);
								
								application.add_entity(
									Entity::builder("Eye")
										.rigid_body_type(RigidBodyType::KinematicPositionBased)
										.position(Isometry3::translation(-0.06, 0.02, -0.085))
										.parent(entity.clone(), true)
										.collider(ColliderBuilder::ball(0.05)
											.collision_groups(InteractionGroups::none())
											.build())
										.component(Comedy::new(renderer)?)
										.tag("CloseHide", true)
										.build()
								);
							}
							
							
							if let Some(ik) = self.ik.get(application) {
								if class == TrackedDeviceClass::HMD {
									ik.set_hmd(entity.clone());
								}
								
								match vr.system.get_controller_role_for_tracked_device_index(tracked_id) {
									Some(TrackedControllerRole::LeftHand) => ik.set_hand_left(entity.clone()),
									Some(TrackedControllerRole::RightHand) => ik.set_hand_right(entity.clone()),
									_ => {}
								}
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
