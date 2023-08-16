use err_derive::Error;
use linked_hash_map::LinkedHashMap;
use rapier3d::geometry::{Collider, ColliderBuilder, ColliderShape};
use serde_derive::Deserialize;

use crate::math::{PI, Vec3, AABB};
use crate::renderer::Renderer;
use crate::renderer::assets_manager::{TomlAsset, TomlLoadError};
use super::super::model::SimpleModel;
use super::super::model::simple::asset::{ObjAsset, ObjLoadError};


#[derive(Deserialize, Debug, Copy, Clone, PartialEq)]
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
	seat: Option<[f32; 6]>,
	phys_aabb: Option<[f32; 6]>,
}

pub struct Prop {
	pub model: SimpleModel,
	pub name: String,
	pub collider: Collider,
	pub tip: Option<String>,
	pub seat: Option<AABB>,
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
			let aabb = if let Some([x1, y1, z1, x2, y2, z2]) = pconf.phys_aabb {
				AABB::new(point!(x1, y1, z1), point!(x2, y2, z2))
			} else {
				model.aabb()
			};
			let extents = aabb.extents();
			let center = aabb.center();
			
			let collider = match pconf.collider {
				PropCollider::Box       => ColliderBuilder::new(ColliderShape::cuboid(extents.x / 2.0, extents.y / 2.0, extents.z / 2.0)).translation(center.coords),
				PropCollider::Sphere    => ColliderBuilder::new(ColliderShape::ball(extents.max() / 2.0)).translation(center.coords),
				PropCollider::CylinderX => ColliderBuilder::new(ColliderShape::cylinder(extents.x / 2.0, extents.yz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, PI / 2.0)),
				PropCollider::CylinderY => ColliderBuilder::new(ColliderShape::cylinder(extents.y / 2.0, extents.xz().max() / 2.0)).translation(center.coords),
				PropCollider::CylinderZ => ColliderBuilder::new(ColliderShape::cylinder(extents.z / 2.0, extents.xy().max() / 2.0)).translation(center.coords).rotation(vector!(PI / 2.0, 0.0, 0.0)),
				PropCollider::ConePX    => ColliderBuilder::new(ColliderShape::cone(extents.x / 2.0, extents.yz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, PI / 2.0)),
				PropCollider::ConePY    => ColliderBuilder::new(ColliderShape::cone(extents.y / 2.0, extents.xz().max() / 2.0)).translation(center.coords),
				PropCollider::ConePZ    => ColliderBuilder::new(ColliderShape::cone(extents.z / 2.0, extents.xy().max() / 2.0)).translation(center.coords).rotation(vector!(PI / 2.0, 0.0, 0.0)),
				PropCollider::ConeNX    => ColliderBuilder::new(ColliderShape::cone(extents.x / 2.0, extents.yz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, -PI / 2.0)),
				PropCollider::ConeNY    => ColliderBuilder::new(ColliderShape::cone(extents.y / 2.0, extents.xz().max() / 2.0)).translation(center.coords).rotation(vector!(0.0, 0.0, PI)),
				PropCollider::ConeNZ    => ColliderBuilder::new(ColliderShape::cone(extents.z / 2.0, extents.xy().max() / 2.0)).translation(center.coords).rotation(vector!(-PI / 2.0, 0.0, 0.0)),
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
					
					ColliderBuilder::new(ColliderShape::capsule(center - offset, center + offset, radius))
				},
			};
			
			let collider = collider.density(100.0);
			
			let seat = pconf.seat.map(|[x1, y1, z1, x2, y2, z2]| AABB::new(point!(x1, y1, z1), point!(x2, y2, z2)));
			
			props.push(Prop {
				model,
				name,
				collider: collider.build(),
				tip: pconf.tip,
				seat,
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

