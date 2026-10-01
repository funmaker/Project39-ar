use anyhow::Result;
use glamx::Vec3Swizzles;
use linked_hash_map::LinkedHashMap;
use rapier3d::geometry::{Collider, ColliderBuilder, ColliderShape};
use serde_derive::Deserialize;

use crate::math::{PI, AABB};
use crate::renderer::Renderer;
use crate::renderer::assets_manager::TomlAsset;
use super::super::model::SimpleModel;
use super::super::model::simple::asset::ObjAsset;


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
	pub fn new(renderer: &mut Renderer) -> Result<Self> {
		let mut props = Vec::new();
		
		let config: LinkedHashMap<String, PropConfig> = renderer.load(TomlAsset::at("props.toml"))?;
		
		for (name, pconf) in config {
			let model = renderer.load(ObjAsset::at(&pconf.model, &pconf.texture))?;
			let aabb = if let Some([x1, y1, z1, x2, y2, z2]) = pconf.phys_aabb {
				AABB::new(point!(x1, y1, z1).into(), point!(x2, y2, z2).into())
			} else {
				model.aabb()
			};
			let extents = aabb.extents();
			let center = aabb.center();
			
			let collider = match pconf.collider {
				PropCollider::Box       => ColliderBuilder::new(ColliderShape::cuboid(extents.x / 2.0, extents.y / 2.0, extents.z / 2.0)).translation(center),
				PropCollider::Sphere    => ColliderBuilder::new(ColliderShape::ball(extents.max_element() / 2.0)).translation(center),
				PropCollider::CylinderX => ColliderBuilder::new(ColliderShape::cylinder(extents.x / 2.0, extents.yz().max_element() / 2.0)).translation(center).rotation(vector!(0.0, 0.0, PI / 2.0).into()),
				PropCollider::CylinderY => ColliderBuilder::new(ColliderShape::cylinder(extents.y / 2.0, extents.xz().max_element() / 2.0)).translation(center),
				PropCollider::CylinderZ => ColliderBuilder::new(ColliderShape::cylinder(extents.z / 2.0, extents.xy().max_element() / 2.0)).translation(center).rotation(vector!(PI / 2.0, 0.0, 0.0).into()),
				PropCollider::ConePX    => ColliderBuilder::new(ColliderShape::cone(extents.x / 2.0, extents.yz().max_element() / 2.0)).translation(center).rotation(vector!(0.0, 0.0, PI / 2.0).into()),
				PropCollider::ConePY    => ColliderBuilder::new(ColliderShape::cone(extents.y / 2.0, extents.xz().max_element() / 2.0)).translation(center),
				PropCollider::ConePZ    => ColliderBuilder::new(ColliderShape::cone(extents.z / 2.0, extents.xy().max_element() / 2.0)).translation(center).rotation(vector!(PI / 2.0, 0.0, 0.0).into()),
				PropCollider::ConeNX    => ColliderBuilder::new(ColliderShape::cone(extents.x / 2.0, extents.yz().max_element() / 2.0)).translation(center).rotation(vector!(0.0, 0.0, -PI / 2.0).into()),
				PropCollider::ConeNY    => ColliderBuilder::new(ColliderShape::cone(extents.y / 2.0, extents.xz().max_element() / 2.0)).translation(center).rotation(vector!(0.0, 0.0, PI).into()),
				PropCollider::ConeNZ    => ColliderBuilder::new(ColliderShape::cone(extents.z / 2.0, extents.xy().max_element() / 2.0)).translation(center).rotation(vector!(-PI / 2.0, 0.0, 0.0).into()),
				PropCollider::Capsule   => {
					let max = extents.max_element();
					let radius;
					let offset;
					
					if extents.x == max {
						radius = extents.yz().max_element() / 2.0;
						offset = glamx::Vec3::X * (extents.x - radius) / 2.0;
					} else if extents.y == max {
						radius = extents.xz().max_element() / 2.0;
						offset = glamx::Vec3::Y * (extents.y - radius) / 2.0;
					} else {
						radius = extents.xy().max_element() / 2.0;
						offset = glamx::Vec3::Z * (extents.z - radius) / 2.0;
					}
					
					ColliderBuilder::new(ColliderShape::capsule(center - offset, center + offset, radius))
				},
			};
			
			let collider = collider.density(100.0);
			
			let seat = pconf.seat.map(|[x1, y1, z1, x2, y2, z2]| AABB::new(point!(x1, y1, z1).into(), point!(x2, y2, z2).into()));
			
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

