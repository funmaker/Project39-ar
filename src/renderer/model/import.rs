use std::io::BufReader;
use std::path::PathBuf;
use std::fs::File;
use std::sync::Arc;
use err_derive::Error;
use obj::{Obj, ObjError};
use image::{ImageFormat, ImageError, DynamicImage, ImageBuffer};
use cgmath::num_traits::FromPrimitive;
use openvr::render_models as openvr_rm;

use super::{Model, ModelError, VertexIndex, simple, mmd as mmd_model};
use crate::renderer::Renderer;
use fallible_iterator::FallibleIterator;
use mmd::pmx::material::Toon;

pub fn from_obj<VI: VertexIndex + FromPrimitive>(path: &str, renderer: &mut Renderer) -> Result<Arc<dyn Model>, LoadError> {
	let model_reader = BufReader::new(File::open(format!("{}.obj", path))?);
	let model: Obj<obj::TexturedVertex, VI> = obj::load_obj(model_reader)?;
	
	let texture_reader = BufReader::new(File::open(format!("{}.png", path))?);
	let texture = image::load(texture_reader, ImageFormat::Png)?;
	
	Ok(Arc::new(simple::SimpleModel::new(
		&model.vertices.iter().map(Into::into).collect::<Vec<_>>(),
		&model.indices,
		texture,
		renderer,
	)?))
}

impl From<&obj::TexturedVertex> for simple::Vertex {
	fn from(vertex: &obj::TexturedVertex) -> Self {
		simple::Vertex::new(
			vertex.position,
			vertex.normal,
			[vertex.texture[0], 1.0 - vertex.texture[1]]
		)
	}
}


pub fn from_openvr(model: openvr_rm::Model, texture: openvr_rm::Texture, renderer: &mut Renderer) -> Result<Arc<dyn Model>, LoadError> {
	let vertices: Vec<simple::Vertex> = model.vertices().iter().map(Into::into).collect();
	let indices: Vec<u16> = model.indices().iter().copied().map(Into::into).collect();
	let size = texture.dimensions();
	let image = DynamicImage::ImageRgba8(ImageBuffer::from_raw(size.0 as u32, size.1 as u32, texture.data().into()).unwrap());
	
	Ok(Arc::new(simple::SimpleModel::new(
		&vertices,
		&indices,
		image,
		renderer
	)?))
}

impl From<&openvr_rm::Vertex> for simple::Vertex {
	fn from(vertex: &openvr_rm::Vertex) -> Self {
		simple::Vertex::new(vertex.position, vertex.normal, vertex.texture_coord)
	}
}


pub fn from_pmx(path: &str, renderer: &mut Renderer) -> Result<Arc<dyn Model>, LoadError> {
	let mut root = PathBuf::from(path);
	root.pop();

	let model_reader = BufReader::new(File::open(path)?);
	let header = mmd::HeaderReader::new(model_reader)?;

	println!("{}", header);

	let mut vertices_reader = mmd::VertexReader::new(header)?;
	let vertices: Vec<mmd_model::Vertex> = vertices_reader.iter()
	                                                      .map(|v: mmd::Vertex<i16>| Ok(v.into()))
	                                                      .collect()?;
	
	let mut surfaces_reader = mmd::SurfaceReader::new(vertices_reader)?;
	let indices: Vec<u16> = surfaces_reader.iter()
	                                               .fold(Vec::new(), |mut acc, surface| { acc.extend_from_slice(&surface); Ok(acc) })?;
	
	let mut model = mmd_model::MMDModel::new(&vertices, &indices, renderer)?;
	
	let mut textures_reader = mmd::TextureReader::new(surfaces_reader)?;
	
	let textures = textures_reader.iter()
	                              .map_err(LoadError::PmxError)
	                              .map(|texture_path| {
		                              let texture_reader = BufReader::new(File::open(root.join(&texture_path))?);
		                              let image = image::load(texture_reader, ImageFormat::from_path(&texture_path)?)?;
		
		                              Ok(model.add_texture(image, renderer)?)
	                              })
	                              .collect::<Vec<_>>()?;
	
	let mut materials_reader = mmd::MaterialReader::new(textures_reader)?;
	let mut last_index = 0_usize;
	
	materials_reader.iter::<i32>()
	                .map_err(LoadError::PmxError)
	                .for_each(|material| {
		                let toon_index = match material.toon {
			                Toon::Texture(id) => id,
			                Toon::Internal(_) => -1
		                };
		                
		                model.add_sub_mesh(
			                last_index .. last_index + material.surface_count as usize,
			                textures.get(material.texture_index as usize).cloned(),
			                textures.get(toon_index as usize).cloned(),
			                textures.get(material.environment_index as usize).cloned(),
			                renderer,
		                )?;
	
		                last_index += material.surface_count as usize;
	
		                Ok(())
	                })?;
	
	Ok(Arc::new(model))
}

const MMD_UNIT_SIZE: f32 = 7.9 / 100.0; // https://www.deviantart.com/hogarth-mmd/journal/1-MMD-unit-in-real-world-units-685870002

impl<I> From<mmd::Vertex<I>> for mmd_model::Vertex {
	fn from(vertex: mmd::Vertex<I>) -> Self {
		mmd_model::Vertex::new(
			[vertex.position[0] * MMD_UNIT_SIZE, vertex.position[1] * MMD_UNIT_SIZE, vertex.position[2] * MMD_UNIT_SIZE],
			vertex.normal,
			vertex.uv,
		)
	}
}

#[derive(Debug, Error)]
pub enum LoadError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] ObjError),
	#[error(display = "{}", _0)] PmxError(#[error(source)] mmd::Error),
	#[error(display = "{}", _0)] ImageError(#[error(source)] ImageError),
}
