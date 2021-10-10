use std::sync::Arc;
use std::convert::TryInto;
use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter};
use err_derive::Error;
use image::ImageFormat;
use mmd::pmx::morph::Offsets;
use mmd::pmx::bone::{BoneFlags, Connection};
use mmd::pmx::material::{Toon, EnvironmentBlendMode, DrawingFlags};
use mmd::WeightDeform;

use crate::renderer::Renderer;
use crate::component::model::mmd::{Vertex, MMDModelShared, BoneConnection, Bone, SubMeshDesc};
use crate::debug;
use crate::math::{Color, Vec3};
use crate::component::model::ModelError;
use crate::renderer::assets_manager::AssetError;
use super::{AssetKey, AssetsManager};

#[derive(Clone, Hash, Debug)]
pub struct PmxAsset {
	path: PathBuf,
}

impl PmxAsset {
	pub fn at(model_path: impl AsRef<Path>) -> Self {
		PmxAsset {
			path: model_path.as_ref().to_path_buf(),
		}
	}
}

impl AssetKey for PmxAsset {
	type Asset = Arc<MMDModelShared>;
	type Error = MMDModelLoadError;
	
	fn load(&self, _assets_manager: &mut AssetsManager, renderer: &mut Renderer) -> Result<Self::Asset, Self::Error> {
		let mut root = PathBuf::from(&self.path);
		root.pop();
		
		let header = mmd::HeaderReader::new(AssetsManager::find_asset(&self.path)?)?;
		
		// dprintln!("{}", header);
		
		let mut vertices_reader = mmd::VertexReader::new(header)?;
		let vertices = vertices_reader.iter::<i16>()
		                              .map(|v| v.map(Into::into))
		                              .collect::<Result<Vec<Vertex>, _>>()?;
		
		let mut surfaces_reader = mmd::SurfaceReader::new(vertices_reader)?;
		let indices = surfaces_reader.iter()
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
		
		for material in materials_reader.iter::<i32>() {
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
		                            .collect::<Result<Vec<mmd::Bone<i32>>, _>>()?;
		
		for def in bone_defs.iter() {
			let name = if def.universal_name.len() > 0 {
				&def.universal_name
			} else if let Some(translated) = debug::translate(&def.local_name) {
				translated
			} else {
				&def.local_name
			};
			
			let parent = if def.parent < 0 { None } else { Some(def.parent as usize) };
			
			let model_pos = Vec3::from(def.position).flip_x() * MMD_UNIT_SIZE;
			let display = def.bone_flags.contains(BoneFlags::Display);
			
			let mut color = if def.bone_flags.contains(BoneFlags::InverseKinematics) {
				Color::green()
			} else if def.bone_flags.contains(BoneFlags::Rotatable) && def.bone_flags.contains(BoneFlags::Movable) {
				Color::magenta()
			} else if def.bone_flags.contains(BoneFlags::Rotatable) {
				Color::blue().lightness(1.5)
			} else if def.bone_flags.contains(BoneFlags::Movable) {
				Color::dyellow()
			} else {
				Color::dwhite()
			};
			
			if !def.bone_flags.contains(BoneFlags::CanOperate) {
				color = color.lightness(0.5);
			}
			
			let connection = match def.connection {
				Connection::Index(id) if id <= 0 => BoneConnection::None,
				Connection::Index(id) => BoneConnection::Bone(id as usize),
				Connection::Position(pos) => BoneConnection::Offset(Vec3::from(pos).flip_x() * MMD_UNIT_SIZE),
			};
			
			let local_pos = if def.parent < 0 {
				model_pos
			} else {
				let parent = &bone_defs[def.parent as usize];
				model_pos - Vec3::from(parent.position).flip_x() * MMD_UNIT_SIZE
			};
			
			model.add_bone(Bone::new(name,
			                         parent,
			                         color,
			                         &model_pos,
			                         &local_pos,
			                         display,
			                         connection));
		}
		
		let mut morphs_reader = mmd::MorphReader::new(bones_reader)?;
		
		for morph in morphs_reader.iter::<i32, u32, i32, i32, i32>() {
			let morph = morph?;
			if let Offsets::Vertex(offsets) = morph.offsets {
				model.add_morph(offsets.iter()
				                       .map(|offset| (offset.vertex, Vec3::from(offset.offset).flip_x() * MMD_UNIT_SIZE))
				                       .collect());
			}
		}
		
		Ok(Arc::new(model.build(renderer)?))
	}
}

impl Display for PmxAsset {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "PMX model {}", self.path.to_string_lossy())
	}
}

fn find_image_format<P: AsRef<Path>>(path: P) -> Result<ImageFormat, MMDModelLoadError> {
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

const MMD_UNIT_SIZE: f32 = 7.9 / 100.0; // https://www.deviantart.com/hogarth-mmd/journal/1-MMD-unit-in-real-world-units-685870002

impl<I: Into<i32>> From<mmd::Vertex<I>> for Vertex {
	fn from(vertex: mmd::Vertex<I>) -> Self {
		let (bones, bones_weights) = match vertex.weight_deform {
			WeightDeform::Bdef1(bdef) => ([bdef.bone_index.into(), 0, 0, 0],
			                              [1.0, 0.0, 0.0, 0.0]),
			WeightDeform::Bdef2(bdef) => ([bdef.bone_1_index.into(), bdef.bone_2_index.into(), 0, 0],
			                              [bdef.bone_1_weight, 1.0-bdef.bone_1_weight, 0.0, 0.0]),
			WeightDeform::Bdef4(bdef) => ([bdef.bone_1_index.into(), bdef.bone_2_index.into(), bdef.bone_3_index.into(), bdef.bone_4_index.into()],
			                              [bdef.bone_1_weight, bdef.bone_2_weight, bdef.bone_3_weight, bdef.bone_4_weight]),
			WeightDeform::Sdef(sdef) => ([sdef.bone_1_index.into(), sdef.bone_2_index.into(), 0, 0], // TODO: Proper SDEF support
			                             [sdef.bone_1_weight, 1.0-sdef.bone_1_weight, 0.0, 0.0]),
			WeightDeform::Qdef(_) => unimplemented!("QDEF weight deforms are not supported."),
		};
		
		let bones_indices = [bones[0].max(0) as u32, bones[1].max(0) as u32, bones[2].max(0) as u32, bones[3].max(0) as u32];
		let pos = Vec3::from(vertex.position).flip_x() * MMD_UNIT_SIZE;
		let normal = Vec3::from(vertex.normal).flip_x();
		
		Vertex::new(
			pos,
			normal,
			vertex.uv,
			vertex.edge_scale,
			bones_indices,
			bones_weights,
		)
	}
}

trait FlipX {
	fn flip_x(self) -> Self;
}

impl FlipX for Vec3 {
	fn flip_x(self) -> Self {
		vector!(-self.x, self.y, self.z)
	}
}

trait FlattenArrayVec {
	type Out;
	fn flatten(self) -> Self::Out;
}

impl<T> FlattenArrayVec for Vec<[T; 3]> {
	type Out = Vec<T>;
	fn flatten(self) -> Self::Out {
		unsafe {
			let (ptr, len, cap) = self.into_raw_parts();
			Vec::from_raw_parts(ptr as *mut T, len * 3, cap * 3)
		}
	}
}

#[derive(Debug, Error)]
pub enum MMDModelLoadError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] AssetError(#[error(source)] AssetError),
	#[error(display = "{}", _0)] IoError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] PmxError(#[error(source)] mmd::Error),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
}
