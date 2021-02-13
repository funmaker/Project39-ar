use std::io::BufReader;
use std::fs::File;
use err_derive::Error;
use image::ImageFormat;
use obj::Obj;
use num_traits::FromPrimitive;

use crate::renderer::model::{VertexIndex, ModelError};
use crate::renderer::Renderer;
use super::{SimpleModel, Vertex};

#[allow(unused)]
pub fn from_obj<VI: VertexIndex + FromPrimitive>(path: &str, renderer: &mut Renderer) -> Result<SimpleModel<VI>, SimpleModelLoadError> {
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


#[derive(Debug, Error)]
pub enum SimpleModelLoadError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] obj::ObjError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] image::ImageError),
}