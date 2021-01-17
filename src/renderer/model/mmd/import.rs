use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;
use std::ffi::{OsStr, OsString};
use std::convert::TryFrom;
use err_derive::Error;
use mmd::pmx::material::{Toon, EnvironmentBlendMode, DrawingFlags};
use fallible_iterator::FallibleIterator;
use image::ImageFormat;

use crate::renderer::model::{ModelError, VertexIndex};
use crate::renderer::Renderer;
use super::{MMDModel, Vertex, MaterialInfo};

pub fn from_pmx<VI>(path: &str, renderer: &mut Renderer) -> Result<MMDModel<VI>, MMDModelLoadError> where VI: VertexIndex + TryFrom<u8> + TryFrom<u16> + TryFrom<i32> {
	let mut root = PathBuf::from(path);
	root.pop();
	
	let model_reader = BufReader::new(File::open(path)?);
	let header = mmd::HeaderReader::new(model_reader)?;
	
	dprintln!("{}", header);
	
	let mut vertices_reader = mmd::VertexReader::new(header)?;
	let vertices: Vec<Vertex> = vertices_reader.iter()
	                                           .map(|v: mmd::Vertex<i16>| Ok(v.into()))
	                                           .collect()?;
	
	let mut surfaces_reader = mmd::SurfaceReader::new(vertices_reader)?;
	let indices: Vec<VI> = surfaces_reader.iter()
	                                      .fold(Vec::new(), |mut acc, surface| { acc.extend_from_slice(&surface); Ok(acc) })?;
	
	let mut model = MMDModel::new(&vertices, &indices, renderer)?;
	
	let mut textures_reader = mmd::TextureReader::new(surfaces_reader)?;
	
	let textures = textures_reader.iter()
	                              .map_err(MMDModelLoadError::PmxError)
	                              .map(|texture_path| {
		                              let path = lookup_windows_path(&root, &texture_path)?;
		                              let texture_reader = BufReader::new(File::open(path)?);
		                              let image = image::load(texture_reader, ImageFormat::from_path(&texture_path)?)?;
		                              let has_alpha = image.color().has_alpha();
		
		                              Ok((model.add_texture(image, renderer)?, has_alpha))
	                              })
	                              .collect::<Vec<_>>()?;
	
	let mut materials_reader = mmd::MaterialReader::new(textures_reader)?;
	let mut last_index = 0_usize;
	
	materials_reader.iter::<i32>()
	                .map_err(MMDModelLoadError::PmxError)
	                .for_each(|material| {
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
			                sphere_mode: sphere_mode,
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
		
		                Ok(())
	                })?;
	
	let mut bones_reader = mmd::BoneReader::new(materials_reader)?;
	
	model.bones = bones_reader.iter().collect()?;
	
	for bone in model.bones.iter_mut() {
		bone.position[0] *= MMD_UNIT_SIZE;
		bone.position[1] *= MMD_UNIT_SIZE;
		bone.position[2] *= MMD_UNIT_SIZE;
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

impl<I> From<mmd::Vertex<I>> for Vertex {
	fn from(vertex: mmd::Vertex<I>) -> Self {
		Vertex::new(
			[-vertex.position[0] * MMD_UNIT_SIZE, vertex.position[1] * MMD_UNIT_SIZE, vertex.position[2] * MMD_UNIT_SIZE],
			[-vertex.normal[0], vertex.normal[1], vertex.normal[2]],
			vertex.uv,
			vertex.edge_scale,
		)
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
