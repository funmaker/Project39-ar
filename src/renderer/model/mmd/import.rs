use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;
use std::ffi::{OsStr, OsString};
use err_derive::Error;
use mmd::pmx::material::{Toon, EnvironmentBlendMode, DrawingFlags};
use mmd::pmx::bone::{BoneFlags, Connection};
use mmd::WeightDeform;
use image::ImageFormat;

use crate::application::entity::{Bone, BoneConnection};
use crate::renderer::model::{ModelError, VertexIndex};
use crate::renderer::Renderer;
use crate::math::{Vec3, Color};
use crate::debug;
use super::{Vertex, sub_mesh::MaterialInfo, shared::MMDModelShared};

pub fn from_pmx<VI>(path: &str, renderer: &mut Renderer) -> Result<MMDModelShared<VI>, MMDModelLoadError> where VI: VertexIndex + mmd::VertexIndex {
	let mut root = PathBuf::from(path);
	root.pop();
	
	let model_reader = BufReader::new(File::open(path)?);
	let header = mmd::HeaderReader::new(model_reader)?;
	
	// dprintln!("{}", header);
	
	let mut vertices_reader = mmd::VertexReader::new(header)?;
	let vertices = vertices_reader.iter::<i16>()
                                  .map(|v| v.map(Into::into))
                                  .collect::<Result<Vec<Vertex>, _>>()?;
	
	let mut surfaces_reader = mmd::SurfaceReader::new(vertices_reader)?;
	let indices = surfaces_reader.iter()
	                             .collect::<Result<Vec<[VI; 3]>, _>>()?
	                             .flatten();
	
	let mut model = MMDModelShared::new(&vertices, &indices, renderer)?;
	
	let mut textures_reader = mmd::TextureReader::new(surfaces_reader)?;
	let mut textures = vec![];
	
	for texture_path in textures_reader.iter() {
		let texture_path = texture_path?;
		let os_path = lookup_windows_path(&root, &texture_path)?;
		let texture_reader = BufReader::new(File::open(os_path)?);
		let image = image::load(texture_reader, ImageFormat::from_path(&texture_path)?)?;
		let has_alpha = image.color().has_alpha();
		
		textures.push((model.add_texture(image, renderer)?, has_alpha));
	}
	
	let mut materials_reader = mmd::MaterialReader::new(textures_reader)?;
	let mut last_index = 0_usize;
	
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
		
		let material_info = MaterialInfo {
			color: material.diffuse_color,
			specular: material.specular_color,
			specularity: material.specular_strength,
			ambient: material.ambient_color,
			sphere_mode,
		};
		
		let (texture, has_alpha) = textures.get(material.texture_index as usize)
		                                   .cloned()
		                                   .map_or((None, false), |(texture, has_alpha)| (Some(texture), has_alpha));
		
		let toon = textures.get(toon_index as usize)
		                   .cloned()
		                   .map(|(t, _)| t);
		
		let sphere_map = textures.get(material.environment_index as usize)
		                         .cloned()
		                         .map(|(t, _)| t);
		
		let edge = material.draw_flags.contains(DrawingFlags::HasEdge).then_some((material.edge_scale, material.edge_color));
		
		model.add_sub_mesh(
			last_index .. last_index + material.surface_count as usize,
			material_info,
			texture,
			toon,
			sphere_map,
			material.draw_flags.contains(DrawingFlags::NoCull),
			!has_alpha,
			edge,
			renderer,
		)?;
		
		last_index += material.surface_count as usize;
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
	
	Ok(model)
}

// Windows why
fn lookup_windows_path(root: &PathBuf, orig_path: &str) -> Result<PathBuf, MMDModelLoadError> {
	if cfg!(target_os = "windows") {
		return Ok(root.join(orig_path));
	}
	
	let mut path = PathBuf::from(orig_path.replace("\\", "/"));
	let file_name = path.file_name().ok_or_else(|| MMDModelLoadError::PathError(orig_path.to_string()))?.to_owned();
	path.pop();
	
	let mut cur_dir = root.clone();
	
	for component in path.components() {
		cur_dir.push(lookup_component(&cur_dir, component.as_os_str(), true)?);
	}
	
	cur_dir.push(lookup_component(&cur_dir, &file_name, false)?);
	
	Ok(cur_dir)
}

fn lookup_component(cur_dir: &PathBuf, name: &OsStr, dir: bool) -> Result<OsString, MMDModelLoadError> {
	let mut next_dir = None;
	
	for file in std::fs::read_dir(&cur_dir)? {
		let file = file?;
		
		if (!dir && file.file_type()?.is_file()) || (dir && file.file_type()?.is_dir()) {
			if file.file_name() == name {
				next_dir = Some(name.to_owned());
				break;
			} else if file.file_name().to_ascii_lowercase() == name.to_ascii_lowercase() {
				next_dir = Some(file.file_name());
			}
		}
	}
	
	match next_dir {
		Some(next_dir) => Ok(next_dir),
		None => Err(MMDModelLoadError::FileNotFound(cur_dir.join(name).to_string_lossy().to_string())),
	}
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
		Vec3::new(-self.x, self.y, self.z)
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
	#[error(display = "Failed to parse path {}", _0)] PathError(String),
	#[error(display = "File not found: {}", _0)] FileNotFound(String),
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] PmxError(#[error(source)] mmd::Error),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
}
