use err_derive::Error;
use serde_derive::Deserialize;
use rapier3d::geometry::{Collider, ColliderBuilder, ColliderShape};
use linked_hash_map::LinkedHashMap;

use crate::component::model::SimpleModel;
use crate::renderer::Renderer;
use crate::math::{PI, Vec3};
use crate::renderer::assets_manager::obj::{ObjAsset, ObjLoadError};
use crate::renderer::assets_manager::toml::{TomlAsset, TomlLoadError};

#[derive(Deserialize, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum PropCollider {
	Box,
	Sphere,
	CylinderX,
	CylinderY,
	CylinderZ,
	Capsule,
	ConePX,
	ConePY,
	ConePZ,
	ConeNX,
	ConeNY,
	ConeNZ,
}

#[derive(Deserialize, Debug, Clone)]
struct PropConfig {
	model: String,
	texture: String,
	#[serde(default)] collider: PropCollider,
	tip: Option<String>,
}

pub struct Prop {
	pub model: SimpleModel<u32>,
	pub name: String,
	pub collider: Collider,
	pub tip: Option<String>,
}

pub struct PropCollection {
	pub props: Vec<Prop>,
}

impl PropCollection {
	pub fn new(renderer: &mut Renderer) -> Result<Self, PropManagerError> {
		let mut props = Vec::new();
		
		let config: LinkedHashMap<String, PropConfig> = renderer.load(TomlAsset::at("props.toml"))?;
		
		for (name, pconf) in config {
			let model = renderer.load(ObjAsset::at(&pconf.model, &pconf.texture))?;
			let aabb = model.aabb();
			let extents = aabb.extents();
			let center = aabb.center();
			
			let collider = match pconf.collider {
				PropCollider::Box       => ColliderBuilder::new(ColliderShape::cuboid(extents.x / 2.0, extents.y / 2.0, extents.z / 2.0)).translation(center.coords).build(),
				PropCollider::Sphere    => ColliderBuilder::new(ColliderShape::ball(extents.max() / 2.0)).translation(center.coords).build(),
				PropCollider::CylinderX => ColliderBuilder::new(ColliderShape::cylinder(extents.x / 2.0, extents.yz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, PI / 2.0)).build(),
				PropCollider::CylinderY => ColliderBuilder::new(ColliderShape::cylinder(extents.y / 2.0, extents.xz().max() / 2.0)).translation(center.coords).build(),
				PropCollider::CylinderZ => ColliderBuilder::new(ColliderShape::cylinder(extents.z / 2.0, extents.xy().max() / 2.0)).translation(center.coords).rotation(vector!(PI / 2.0, 0.0, 0.0)).build(),
				PropCollider::ConePX    => ColliderBuilder::new(ColliderShape::cone(extents.x / 2.0, extents.yz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, PI / 2.0)).build(),
				PropCollider::ConePY    => ColliderBuilder::new(ColliderShape::cone(extents.y / 2.0, extents.xz().max() / 2.0)).translation(center.coords).build(),
				PropCollider::ConePZ    => ColliderBuilder::new(ColliderShape::cone(extents.z / 2.0, extents.xy().max() / 2.0)).translation(center.coords).rotation(vector!(PI / 2.0, 0.0, 0.0)).build(),
				PropCollider::ConeNX    => ColliderBuilder::new(ColliderShape::cone(extents.x / 2.0, extents.yz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, -PI / 2.0)).build(),
				PropCollider::ConeNY    => ColliderBuilder::new(ColliderShape::cone(extents.y / 2.0, extents.xz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, PI)).build(),
				PropCollider::ConeNZ    => ColliderBuilder::new(ColliderShape::cone(extents.z / 2.0, extents.xy().max() / 2.0)).translation(center.coords).rotation(vector!(-PI / 2.0, 0.0, 0.0)).build(),
				PropCollider::Capsule   => {
					let max = extents.max();
					let radius;
					let offset;
					
					if extents.x == max {
						radius = extents.yz().max() / 2.0;
						offset = *Vec3::x_axis() * (extents.x - radius) / 2.0;
					} else if extents.y == max {
						radius = extents.xz().max() / 2.0;
						offset = *Vec3::y_axis() * (extents.y - radius) / 2.0;
					} else {
						radius = extents.xy().max() / 2.0;
						offset = *Vec3::z_axis() * (extents.z - radius) / 2.0;
					}
					
					ColliderBuilder::new(ColliderShape::capsule(center - offset, center + offset, radius)).build()
				},
			};
			
			props.push(Prop {
				model,
				name,
				collider,
				tip: pconf.tip,
			});
		}
		
		Ok(PropCollection {
			props,
		})
	}
}

impl Default for PropCollider {
	fn default() -> Self {
		PropCollider::Box
	}
}

#[derive(Debug, Error)]
pub enum PropManagerError {
	#[error(display = "{}", _0)] ObjLoadError(#[error(source)] ObjLoadError),
	#[error(display = "{}", _0)] TomlLoadError(#[error(source)] TomlLoadError),
}

