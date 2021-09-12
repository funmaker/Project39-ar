use std::time::Duration;
use std::collections::BTreeMap;
use std::cell::RefCell;
use openvr::{MAX_TRACKED_DEVICE_COUNT, TrackedDeviceClass, TrackedDeviceIndex};
use openvr_sys::ETrackedDeviceProperty_Prop_RenderModelName_String;

use crate::application::{Entity, EntityRef, Application};
use crate::math::{Point3, Rot3};
use crate::component::{Component, ComponentBase, ComponentInner, ComponentError};
use crate::component::model::SimpleModel;
use super::VrTracked;
use crate::component::pov::PoV;

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
							let model = SimpleModel::<u16>::from_openvr(model, texture, &mut *application.renderer.borrow_mut())?;
							let entity = Entity::new(
								format!("{:?}", class),
								Point3::origin(),
								Rot3::identity(),
								[model.boxed(), VrTracked::new(tracked_id).boxed()],
							);
							
							if class == TrackedDeviceClass::HMD {
								entity.state_mut().hidden = true;
								entity.add_component(PoV::new());
							}
							
							let entity = application.add_entity(entity);
							
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
