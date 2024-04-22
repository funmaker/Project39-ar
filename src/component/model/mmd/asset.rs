use std::{fs, mem};
use std::convert::TryInto;
use std::ffi::OsStr;
use std::fmt::{Display, Formatter};
use std::io::{ErrorKind, Error as IoError};
use std::ops::{Deref, DerefMut};
use std::path::{PathBuf, Path};
use std::sync::Arc;
use anyhow::Result;
use image::ImageFormat;
use mmd::WeightDeform;
use mmd::pmx::bone::{BoneFlags, Connection};
use mmd::pmx::joint::Joint;
use mmd::pmx::material::{Toon, EnvironmentBlendMode, DrawingFlags};
use mmd::pmx::morph::Offsets;
use rapier3d::geometry::{ColliderBuilder, ColliderShape, Group, InteractionGroups};

use crate::{config, debug};
use crate::math::{Color, Isometry3, Rot3, Vec2, Vec3, Vec4, PI};
use crate::renderer::Renderer;
use crate::renderer::assets_manager::{AssetKey, AssetsManager, TomlAsset};
use crate::utils::PatternMatcher;
use super::{Vertex, BodyPart};
use super::overrides::{MMDConfig, MMDJointOverride, MMDRigidBodyOverride};
use super::shared::{MMDModelShared, BoneDesc, BoneConnection, SubMeshDesc, JointDesc, ColliderDesc};


type MMDShapeType = mmd::pmx::rigid_body::ShapeType;

pub struct MMDIndexConfig;

impl mmd::Config for MMDIndexConfig {
	type VertexIndex = u32;
	type TextureIndex = i32;
	type MaterialIndex = i32;
	type BoneIndex = i32;
	type MorphIndex = i32;
	type RigidbodyIndex = i32;
	type Vec2 = Vec2;
	type Vec3 = Vec3;
	type Vec4 = Vec4;
	type AdditionalVec4s = Vec<Vec4>;
}

#[derive(Clone, Hash, Debug)]
pub struct PmxAsset {
	path: PathBuf,
	overrides: bool,
}

impl PmxAsset {
	pub fn at(model_path: impl AsRef<Path>) -> Self {
		PmxAsset {
			path: model_path.as_ref().to_path_buf(),
			overrides: true,
		}
	}
	
	pub fn no_overrides(self) -> Self {
		Self {
			overrides: false,
			..self
		}
	}
}

impl AssetKey for PmxAsset {
	type Asset = Arc<MMDModelShared>;
	
	fn load(&self, assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset> {
		let mut root = PathBuf::from(&self.path);
		root.pop();
		
		let header = mmd::HeaderReader::new(AssetsManager::find_asset(&self.path)?)?;
		
		// dprintln!("{}", header);
		
		let mut overrides: Option<MMDConfig> = if self.overrides {
			match assets_manager.load(TomlAsset::at(&root.join("model.toml")), renderer) {
				Err(err) if err.downcast_ref::<IoError>().map(|err| err.kind()) == Some(ErrorKind::NotFound) => None,
				overrides => Some(overrides?),
			}
		} else {
			None
		};
		
		let mut vertices_reader = mmd::VertexReader::new(header)?;
		let vertices = vertices_reader.iter::<MMDIndexConfig>()
		                              .map(|v| v.map(Into::into))
		                              .collect::<Result<Vec<Vertex>, _>>()?;
		
		let mut surfaces_reader = mmd::SurfaceReader::new(vertices_reader)?;
		let indices = surfaces_reader.iter::<MMDIndexConfig>()
		                             .collect::<Result<Vec<[u32; 3]>, _>>()?
		                             .flatten();
		
		let mut model = MMDModelShared::new(vertices, indices);
		
		let mut textures_reader = mmd::TextureReader::new(surfaces_reader)?;
		let mut textures_alpha = vec![];
		
		for texture_path in textures_reader.iter() {
			let path = root.join(texture_path?);
			let format = find_image_format(AssetsManager::find_asset_path(&path)?)?;
			let texture = image::load(AssetsManager::find_asset(&path)?, format)?;
			let has_alpha = texture.color().has_alpha();
			
			model.add_texture(texture);
			textures_alpha.push(has_alpha);
		}
		
		let mut materials_reader = mmd::MaterialReader::new(textures_reader)?;
		let mut last_index: u32 = 0;
		
		for material in materials_reader.iter::<MMDIndexConfig>() {
			let material = material?;
			
			let toon_index = match material.toon {
				Toon::Texture(id) => id,
				Toon::Internal(_) => -1
			};
			
			let sphere_mode = if material.environment_index < 0 {
				0
			} else {
				match material.environment_blend_mode {
					EnvironmentBlendMode::Disabled => 0,
					EnvironmentBlendMode::Multiply => 1,
					EnvironmentBlendMode::Additive => 2,
					EnvironmentBlendMode::AdditionalVec4 => 3,
				}
			};
			
			let has_alpha = material.texture_index.try_into()
			                        .ok()
			                        .and_then(|id: usize| textures_alpha.get(id))
			                        .copied()
			                        .unwrap_or(true);
			
			let edge = material.draw_flags.contains(DrawingFlags::HasEdge).then_some((material.edge_scale, material.edge_color));
			
			model.add_sub_mesh(SubMeshDesc {
				range: last_index .. last_index + material.surface_count as u32,
				texture: material.texture_index.try_into().ok(),
				toon: toon_index.try_into().ok(),
				sphere_map: material.environment_index.try_into().ok(),
				color: material.diffuse_color,
				specular: material.specular_color,
				specularity: material.specular_strength,
				ambient: material.ambient_color,
				sphere_mode,
				no_cull: material.draw_flags.contains(DrawingFlags::NoCull),
				opaque: !has_alpha,
				edge
			});
			
			last_index += material.surface_count as u32;
		}
		
		let mut bones_reader = mmd::BoneReader::new(materials_reader)?;
		let bone_defs = bones_reader.iter()
		                            .collect::<std::result::Result<Vec<mmd::Bone<MMDIndexConfig>>, _>>()?;
		
		for def in bone_defs.iter() {
			let name = if def.universal_name.len() > 0 {
				&def.universal_name
			} else if let Some(translated) = debug::translate(&def.local_name) {
				translated
			} else {
				&def.local_name
			};
			
			let parent = if def.parent < 0 { None } else { Some(def.parent as usize) };
			
			let model_pos = def.position.from_mmd();
			let display = def.bone_flags.contains(BoneFlags::Display);
			
			let mut color = if def.bone_flags.contains(BoneFlags::InverseKinematics) {
				Color::GREEN
			} else if def.bone_flags.contains(BoneFlags::Rotatable) && def.bone_flags.contains(BoneFlags::Movable) {
				Color::MAGENTA
			} else if def.bone_flags.contains(BoneFlags::Rotatable) {
				Color::BLUE.lightness(1.5)
			} else if def.bone_flags.contains(BoneFlags::Movable) {
				Color::D_YELLOW
			} else {
				Color::D_WHITE
			};
			
			if !def.bone_flags.contains(BoneFlags::CanOperate) {
				color = color.lightness(0.5);
			}
			
			let connection = match def.connection {
				Connection::Index(id) if id < 0 => BoneConnection::None,
				Connection::Index(id) => BoneConnection::Bone(id as usize),
				Connection::Position(pos) => BoneConnection::Offset(pos.from_mmd()),
			};
			
			let local_pos = if def.parent < 0 {
				model_pos
			} else {
				let parent = &bone_defs[def.parent as usize];
				model_pos - parent.position.from_mmd()
			};
			
			model.add_bone(BoneDesc::new(name,
			                             parent,
			                             color,
			                             model_pos,
			                             local_pos,
			                             display,
			                             connection));
		}
		
		let mut morphs_reader = mmd::MorphReader::new(bones_reader)?;
		
		for morph in morphs_reader.iter::<MMDIndexConfig>() {
			let morph = morph?;
			
			if let Offsets::Vertex(offsets) = morph.offsets {
				model.add_morph(offsets.iter()
				                       .map(|offset| (offset.vertex, offset.offset.from_mmd()))
				                       .collect());
			}
		}
		
		let display_reader = mmd::DisplayReader::new(morphs_reader)?;
		
		let mut dump_config = config::get().gen_model_toml.then(|| MMDConfig::default());
		
		let mut rigid_body_reader = mmd::RigidBodyReader::new(display_reader)?;
		let mut rigid_body_defs = rigid_body_reader.iter::<MMDIndexConfig>()
		                                           .collect::<Result<Vec<_>, _>>()?;
		
		if let Some(overrides) = &mut overrides {
			for rb in overrides.rigid_bodies.drain(..) {
				if let Some(id) = rb.id {
					if id >= rigid_body_defs.len() {
						eprintln!("Model {} has no rigid body id {}", self.path.to_string_lossy(), id);
						continue
					}
					
					rb.apply_to(&mut rigid_body_defs[id]);
				} else if let Some(pattern) = &rb.pattern {
					let pattern = PatternMatcher::new(pattern);
					
					let mut matched = false;
					for desc in rigid_body_defs.iter_mut() {
						if pattern.matches(&desc.local_name) || pattern.matches(&desc.universal_name) {
							matched = true;
							rb.apply_to(desc);
						}
					}
					
					if !matched {
						eprintln!("Model {} has no rigid body matching pattern {}", self.path.to_string_lossy(), pattern);
					}
				} else {
					rigid_body_defs.push(rb.into());
				}
			}
		}
		
		for (id, rigid_body) in rigid_body_defs.iter().enumerate() {
			if let Some(dump_config) = &mut dump_config {
				dump_config.rigid_bodies.push(MMDRigidBodyOverride::from_mmd(&rigid_body, id))
			}
			
			let name = if rigid_body.universal_name.len() > 0 {
				&rigid_body.universal_name
			} else if let Some(translated) = debug::translate(&rigid_body.local_name) {
				translated
			} else {
				&rigid_body.local_name
			};
			
			let translation = rigid_body.shape_position.from_mmd();
			let rotation = Rot3::from_axis_angle(&Vec3::y_axis(), -rigid_body.shape_rotation.y)
			             * Rot3::from_axis_angle(&Vec3::x_axis(), -rigid_body.shape_rotation.x)
			             * Rot3::from_axis_angle(&Vec3::z_axis(),  rigid_body.shape_rotation.z);
			let position = Isometry3::from_parts(translation.into(), rotation);
			
			let volume;
			let collider;
			match rigid_body.shape {
				MMDShapeType::Sphere => {
					volume = 4.0 / 3.0 * PI * rigid_body.shape_size.x;
					collider = ColliderBuilder::new(ColliderShape::ball(rigid_body.shape_size.x * MMD_UNIT_SIZE))
				},
				MMDShapeType::Box => {
					volume = rigid_body.shape_size.x / rigid_body.shape_size.y / rigid_body.shape_size.z;
					collider = ColliderBuilder::new(ColliderShape::cuboid(rigid_body.shape_size.x * MMD_UNIT_SIZE,
					                                                      rigid_body.shape_size.y * MMD_UNIT_SIZE,
					                                                      rigid_body.shape_size.z * MMD_UNIT_SIZE))
				},
				MMDShapeType::Capsule => {
					volume = 4.0 / 3.0 * PI * rigid_body.shape_size.y
					       + rigid_body.shape_size.x * rigid_body.shape_size.y * rigid_body.shape_size.y * PI;
					collider = ColliderBuilder::new(ColliderShape::capsule(point![0.0, -rigid_body.shape_size.y * MMD_UNIT_SIZE / 2.0, 0.0],
					                                                       point![0.0,  rigid_body.shape_size.y * MMD_UNIT_SIZE / 2.0, 0.0],
					                                                       rigid_body.shape_size.x * MMD_UNIT_SIZE))
				},
			};
			
			let collider = collider.position(position)
			                       .collision_groups(InteractionGroups::new(Group::from(1 << rigid_body.group_id), Group::from(0xFFFF0000 | rigid_body.non_collision_mask as u32)))
			                       .density(rigid_body.mass / volume)
			                       .build();
			
			model.add_collider(ColliderDesc::new(name,
			                                     rigid_body.bone_index as usize,
			                                     collider,
			                                     rigid_body.move_attenuation,
			                                     rigid_body.rotation_damping,
			                                     rigid_body.repulsion,
			                                     rigid_body.fiction,
			                                     rigid_body.physics_mode));
		}
		
		let mut joints_reader = mmd::JointReader::new(rigid_body_reader)?;
		let mut joints_defs = joints_reader.iter::<MMDIndexConfig>()
		                                   .map(|res| res.map(Into::into))
		                                   .collect::<Result<Vec<_>, _>>()?;
		
		if let Some(overrides) = &mut overrides {
			for mut joint in overrides.joints.drain(..) {
				if let Err(err) = joint.normalize(&bone_defs, &rigid_body_defs) {
					eprintln!("{}", err);
					continue;
				}
				
				if let Some(id) = joint.id {
					if id >= joints_defs.len() {
						eprintln!("Model {} has no joint id {}", self.path.to_string_lossy(), id);
						continue
					}
					
					joint.apply_to(&mut joints_defs[id]);
				} else if let Some(pattern) = &joint.pattern {
					let pattern = PatternMatcher::new(pattern);
					
					let mut matched = false;
					for desc in joints_defs.iter_mut() {
						if pattern.matches(&desc.local_name) || pattern.matches(&desc.universal_name) {
							matched = true;
							joint.apply_to(desc);
						}
					}
					
					if !matched {
						eprintln!("Model {} has no joints matching pattern {}", self.path.to_string_lossy(), pattern);
					}
				} else {
					joints_defs.push(joint.into());
				}
			}
		}
		
		for (id, joint) in joints_defs.iter().enumerate() {
			if let Some(dump_config) = &mut dump_config {
				dump_config.joints.push(MMDJointOverride::from_mmd(&joint, id));
			}
			
			let name = if joint.universal_name.len() > 0 {
				&joint.universal_name
			} else if let Some(translated) = debug::translate(&joint.local_name) {
				translated
			} else {
				&joint.local_name
			};
			
			let translation = joint.position.from_mmd();
			let rotation = Rot3::from_axis_angle(&Vec3::y_axis(), -joint.rotation.y)
			             * Rot3::from_axis_angle(&Vec3::x_axis(), -joint.rotation.x)
			             * Rot3::from_axis_angle(&Vec3::z_axis(),  joint.rotation.z);
			let position = Isometry3::from_parts(translation.into(), rotation);
			let position_min = joint.position_min.from_mmd_scale();
			let position_max = joint.position_max.from_mmd_scale();
			let rotation_min = vector!(joint.rotation_min.x, -joint.rotation_max.y, -joint.rotation_max.z);
			let rotation_max = vector!(joint.rotation_max.x, -joint.rotation_min.y, -joint.rotation_min.z);
			
			model.add_joint(JointDesc::new(name,
			                               joint.joint_type,
			                               joint.rigid_body_a as usize,
			                               joint.rigid_body_b as usize,
			                               position,
			                               position_min,
			                               position_max,
			                               rotation_min,
			                               rotation_max,
			                               joint.position_spring,
			                               joint.rotation_spring,
			                               joint.body_part));
		}
		
		if let Some(dump_config) = &dump_config {
			let model_name = self.path.file_prefix().map(OsStr::to_string_lossy).unwrap_or("unknown".into());
			let dump_path = format!("{}_modeldump.toml", model_name);
			
			dprintln!("Dumping model2.toml of {} to {}", self.path.to_string_lossy(), dump_path);
			fs::write(dump_path, toml::to_string_pretty(dump_config).unwrap()).unwrap();
		}
		
		Ok(Arc::new(model.build(renderer)?))
	}
}

impl Display for PmxAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "PMX model {}", self.path.to_string_lossy())
	}
}

fn find_image_format<P: AsRef<Path>>(path: P) -> Result<ImageFormat> {
	Ok(match imghdr::from_file(&path)? {
		Some(imghdr::Type::Gif) => ImageFormat::Gif,
		Some(imghdr::Type::Tiff) => ImageFormat::Tiff,
		Some(imghdr::Type::Jpeg) => ImageFormat::Jpeg,
		Some(imghdr::Type::Bmp) => ImageFormat::Bmp,
		Some(imghdr::Type::Png) => ImageFormat::Png,
		Some(imghdr::Type::Webp) => ImageFormat::WebP,
		_ => ImageFormat::from_path(&path)?,
	})
}

impl From<mmd::Vertex<MMDIndexConfig>> for Vertex {
	fn from(vertex: mmd::Vertex<MMDIndexConfig>) -> Self {
		let (bones, bones_weights, sdef) = match vertex.weight_deform {
			WeightDeform::Bdef1(bdef) => (
				[bdef.bone_index, 0, 0, 0],
				[1.0, 0.0, 0.0, 0.0],
				None,
			),
			WeightDeform::Bdef2(bdef) => (
				[bdef.bone_1_index, bdef.bone_2_index, 0, 0],
				[bdef.bone_1_weight, 1.0-bdef.bone_1_weight, 0.0, 0.0],
				None,
			),
			WeightDeform::Bdef4(bdef) => (
				[bdef.bone_1_index, bdef.bone_2_index, bdef.bone_3_index, bdef.bone_4_index],
				[bdef.bone_1_weight, bdef.bone_2_weight, bdef.bone_3_weight, bdef.bone_4_weight],
				None,
			),
			WeightDeform::Sdef(sdef) => (
				[sdef.bone_1_index, sdef.bone_2_index, 0, 0],
				[sdef.bone_1_weight, 1.0-sdef.bone_1_weight, 0.0, 0.0],
				Some([
					sdef.c.from_mmd(),
					sdef.r0.from_mmd(),
					sdef.r1.from_mmd(),
				]),
			),
			WeightDeform::Qdef(_) => unimplemented!("QDEF deforms are not supported."),
		};
		
		let bones_indices = [bones[0].max(0) as u32, bones[1].max(0) as u32, bones[2].max(0) as u32, bones[3].max(0) as u32];
		let pos = vertex.position.from_mmd();
		let normal = vertex.normal.from_mmd_normal();
		
		Vertex::new(
			pos,
			normal,
			vertex.uv,
			vertex.edge_scale,
			bones_indices,
			bones_weights,
			sdef,
		)
	}
}

pub struct JointEx<C: mmd::Config> {
	joint: Joint<C>,
	pub body_part: Option<BodyPart>,
}

impl<C: mmd::Config> Deref for JointEx<C> {
	type Target = Joint<C>;
	
	fn deref(&self) -> &Self::Target {
		&self.joint
	}
}

impl<C: mmd::Config> DerefMut for JointEx<C> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.joint
	}
}

impl<C: mmd::Config> From<Joint<C>> for JointEx<C> {
	fn from(joint: Joint<C>) -> Self {
		Self {
			joint,
			body_part: None,
		}
	}
}


impl<C: mmd::Config> Display for JointEx<C>
	where C::RigidbodyIndex: Display {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
		self.joint.fmt(f)?;
		
		if let Some(body_part) = &self.body_part {
			write!(f, "\nbody part: {:?}", body_part)?
		} else {
			write!(f, "\nbody part: None")?
		}
		
		Ok(())
	}
}

const MMD_UNIT_SIZE: f32 = 7.9 / 100.0; // https://www.deviantart.com/hogarth-mmd/journal/1-MMD-unit-in-real-world-units-685870002

trait FromMMD {
	fn from_mmd(self) -> Self;
	fn from_mmd_normal(self) -> Self;
	fn from_mmd_scale(self) -> Self;
}

impl FromMMD for Vec3 {
	fn from_mmd(self) -> Self {
		self.from_mmd_normal().from_mmd_scale()
	}
	
	fn from_mmd_normal(self) -> Self {
		vector!(self.x, self.y, -self.z)
	}
	
	fn from_mmd_scale(self) -> Self {
		self * MMD_UNIT_SIZE
	}
}

trait FlattenArrayVec {
	type Out;
	fn flatten(self) -> Self::Out;
}

impl<T, const N: usize> FlattenArrayVec for Vec<[T; N]> {
	type Out = Vec<T>;
	fn flatten(self) -> Self::Out {
		assert_eq!(
			mem::align_of::<T>(),
			mem::align_of::<[T; N]>(),
		);
		
		assert_eq!(
			N * mem::size_of::<T>(),
			mem::size_of::<[T; N]>(),
		);
		
		// Safety: https://doc.rust-lang.org/std/vec/struct.Vec.html#safety
		// - ptr needs to have been previously allocated via Vec
		// - T needs to have the same alignment as [T; N]
		// - The size of T times the capacity (ie. the allocated size in bytes) needs to be the same size as the pointer was allocated with.
		// - length needs to be less than or equal to capacity.
		unsafe {
			let (ptr, len, cap) = self.into_raw_parts();
			Vec::from_raw_parts(ptr as *mut T, len * N, cap * N)
		}
	}
}
