use std::time::Duration;
use std::collections::BTreeMap;
use std::cell::RefCell;
use rapier3d::dynamics::RigidBodyType;
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedDeviceClass, TrackedDeviceIndex, TrackedControllerRole};
use openvr_sys::ETrackedDeviceProperty_Prop_RenderModelName_String;
use rapier3d::prelude::{ColliderBuilder, RigidBodyBuilder};
use image::{DynamicImage, ImageBuffer, GenericImageView};
use openvr::render_models;
use vulkano::image::{ImmutableImage, ImageDimensions, MipmapsCount};
use vulkano::format::Format;

use crate::application::{Entity, EntityRef, Application, Hand};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::component::model::simple::{SimpleModel, Vertex};
use crate::component::pov::PoV;
use super::VrTracked;
use crate::utils::ImageEx;

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
							let renderer = &mut *application.renderer.borrow_mut();
							let class = vr.system.tracked_device_class(tracked_id);
							let vertices: Vec<Vertex> = model.vertices().iter().map(Into::into).collect();
							let indices: Vec<u16> = model.indices().iter().copied().map(Into::into).collect();
							let size = texture.dimensions();
							let source = DynamicImage::ImageRgba8(ImageBuffer::from_raw(size.0 as u32, size.1 as u32, texture.data().into()).unwrap());
							let width = source.width();
							let height = source.height();
							
							let (image, image_promise) = ImmutableImage::from_iter(source.into_pre_mul_iter(),
							                                                       ImageDimensions::Dim2d{ width, height, array_layers: 1 },
							                                                       MipmapsCount::Log2,
							                                                       Format::R8G8B8A8_UNORM,
							                                                       renderer.load_queue.clone())?;
							
							let model = SimpleModel::new(
								&vertices,
								&indices,
								image,
								image_promise,
								renderer,
							)?;
							
							let aabb = model.aabb();
							let hsize = aabb.half_extents();
							
							let mut entity = Entity::builder(format!("{:?}", class))
								.rigid_body_type(RigidBodyType::KinematicPositionBased)
								.collider(ColliderBuilder::cuboid(hsize.x, hsize.y, hsize.z)
								                          .translation(aabb.center().coords)
								                          .build())
								.rigid_body(RigidBodyBuilder::new(RigidBodyType::KinematicPositionBased)
								                             .additional_mass(1000.0)
								                             .build())
								.component(model)
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

impl From<&render_models::Vertex> for Vertex {
	fn from(vertex: &render_models::Vertex) -> Self {
		Vertex::new(vertex.position, vertex.normal, vertex.texture_coord)
	}
}
