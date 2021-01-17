use std::io::BufReader;
use std::fs::File;
use err_derive::Error;
use image::{ImageFormat, DynamicImage, ImageBuffer};
use obj::Obj;
use openvr::render_models;

use crate::renderer::model::{VertexIndex, ModelError};
use crate::renderer::Renderer;
use super::{SimpleModel, Vertex};

pub fn from_obj<VI: VertexIndex>(path: &str, renderer: &mut Renderer) -> Result<SimpleModel<VI>, SimpleModelLoadError> {
	let model_reader = BufReader::new(File::open(format!("{}.obj", path))?);
	let model: Obj<obj::TexturedVertex, VI> = obj::load_obj(model_reader)?;
	
	let texture_reader = BufReader::new(File::open(format!("{}.png", path))?);
	let texture = image::load(texture_reader, ImageFormat::Png)?;
	
	Ok(SimpleModel::new(
		&model.vertices.iter().map(Into::into).collect::<Vec<_>>(),
		&model.indices,
		texture,
		renderer,
	)?)
}

impl From<&obj::TexturedVertex> for Vertex {
	fn from(vertex: &obj::TexturedVertex) -> Self {
		Vertex::new(
			vertex.position,
			vertex.normal,
			[vertex.texture[0], 1.0 - vertex.texture[1]]
		)
	}
}


pub fn from_openvr(model: render_models::Model, texture: render_models::Texture, renderer: &mut Renderer) -> Result<SimpleModel<u16>, SimpleModelLoadError> {
	let vertices: Vec<Vertex> = model.vertices().iter().map(Into::into).collect();
	let indices: Vec<u16> = model.indices().iter().copied().map(Into::into).collect();
	let size = texture.dimensions();
	let image = DynamicImage::ImageRgba8(ImageBuffer::from_raw(size.0 as u32, size.1 as u32, texture.data().into()).unwrap());
	
	Ok(SimpleModel::new(
		&vertices,
		&indices,
		image,
		renderer
	)?)
}

impl From<&render_models::Vertex> for Vertex {
	fn from(vertex: &render_models::Vertex) -> Self {
		Vertex::new(vertex.position, vertex.normal, vertex.texture_coord)
	}
}



#[derive(Debug, Error)]
pub enum SimpleModelLoadError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
}