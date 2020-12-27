use std::io::BufReader;
use std::fs::File;
use std::path::PathBuf;
use err_derive::Error;
use obj::{Obj, ObjError};
use image::{ImageFormat, ImageError, DynamicImage, ImageBuffer};
use fallible_iterator::FallibleIterator;
use openvr::render_models as openvr_rm;

use super::{Vertex, Model, ModelError, VertexIndex};
use crate::renderer::Renderer;

pub fn from_obj(path: &str, renderer: &Renderer) -> Result<Model, LoadError> {
	let model_reader = BufReader::new(File::open(format!("{}.obj", path))?);
	let model: Obj<obj::TexturedVertex, VertexIndex> = obj::load_obj(model_reader)?;
	
	let texture_reader = BufReader::new(File::open(format!("{}.png", path))?);
	let texture = image::load(texture_reader, ImageFormat::Png)?;
	
	Ok(Model::new(
		&model.vertices.iter().map(Into::into).collect::<Vec<_>>(),
		&model.indices,
		texture,
		renderer,
	)?)
}

impl From<&obj::TexturedVertex> for Vertex {
	fn from(vertex: &obj::TexturedVertex) -> Self {
		Vertex::new(
			vertex.position[0],
			vertex.position[1],
			vertex.position[2],
			vertex.texture[0],
			1.0 - vertex.texture[1],
		)
	}
}


pub fn from_openvr(model: openvr_rm::Model, texture: openvr_rm::Texture, renderer: &Renderer) -> Result<Model, LoadError> {
	let vertices: Vec<Vertex> = model.vertices().iter().map(Into::into).collect();
	let indices: Vec<VertexIndex> = model.indices().iter().copied().map(Into::into).collect();
	let size = texture.dimensions();
	let image = DynamicImage::ImageRgba8(ImageBuffer::from_raw(size.0 as u32, size.1 as u32, texture.data().into()).unwrap());
	
	Ok(Model::new(&vertices, &indices, image, renderer)?)
}

impl From<&openvr_rm::Vertex> for Vertex {
	fn from(vertex: &openvr_rm::Vertex) -> Self {
		Vertex::new(
			vertex.position[0],
			vertex.position[1],
			vertex.position[2],
			vertex.texture_coord[0],
			vertex.texture_coord[1],
		)
	}
}


pub fn from_pmx(path: &str, renderer: &Renderer) -> Result<Model, LoadError> {
	let mut root = PathBuf::from(path);
	root.pop();
	
	let model_reader = BufReader::new(File::open(path)?);
	let header = mmd::HeaderReader::new(model_reader)?;
	
	println!("{}", header);
	
	let mut vertices_reader = mmd::VertexReader::new(header)?;
	let vertices: Vec<Vertex> = vertices_reader.iter()
	                                           .map(|v: mmd::Vertex<i16>| Ok(v.into()))
	                                           .collect()?;
	
	let mut surfaces_reader = mmd::SurfaceReader::new(vertices_reader)?;
	let indices: Vec<VertexIndex> = surfaces_reader.iter()
	                                               .fold(Vec::new(), |mut acc, surface| { acc.extend_from_slice(&surface); Ok(acc) })?;
	
	let mut textures_reader = mmd::TextureReader::new(surfaces_reader)?;
	
	let texture_path = textures_reader.next()?.unwrap();
	let texture_reader = BufReader::new(File::open(root.join(texture_path))?);
	let texture = image::load(texture_reader, ImageFormat::Png)?;
	
	
	let mut materials_reader = mmd::MaterialReader::new(textures_reader)?;
	println!("Materials:");
	
	materials_reader.iter::<i32>().enumerate().for_each(|(i, m)| {
		println!("{}) {}", i, m);
		
		Ok(())
	})?;
	
	Ok(Model::new(
		&vertices,
		&indices,
		texture,
		renderer,
	)?)
}

const MMD_UNIT_SIZE: f32 = 7.9 / 100.0; // https://www.deviantart.com/hogarth-mmd/journal/1-MMD-unit-in-real-world-units-685870002

impl<I> From<mmd::Vertex<I>> for Vertex {
	fn from(vertex: mmd::Vertex<I>) -> Self {
		Vertex::new(
			vertex.position[0] * MMD_UNIT_SIZE,
			vertex.position[1] * MMD_UNIT_SIZE,
			vertex.position[2] * MMD_UNIT_SIZE,
			vertex.uv[0],
			vertex.uv[1],
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
