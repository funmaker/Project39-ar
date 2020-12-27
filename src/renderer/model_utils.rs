use std::io::BufReader;
use std::fs::File;
use err_derive::Error;
use obj::{Obj, TexturedVertex, ObjError};
use image::{ImageFormat, ImageError};

use crate::renderer::model::{Model, ModelError};
use crate::renderer::Renderer;

pub(crate) fn load_obj(path: &str, renderer: &Renderer) -> Result<Model, LoadError> {
	let model_reader = BufReader::new(File::open(format!("{}.obj", path))?);
	let mut model: Obj<TexturedVertex, u16> = obj::load_obj(model_reader)?;
	
	let texture_reader = BufReader::new(File::open(format!("{}.png", path))?);
	let texture = image::load(texture_reader, ImageFormat::Png)?;
	
	Ok(Model::new(
		&model.vertices.iter().map(Into::into).collect::<Vec<_>>(),
		&model.indices,
		texture,
		renderer,
	)?)
}

#[derive(Debug, Error)]
pub enum LoadError {
	#[error(display = "{}", _0)] ModelError(#[error(source)] ModelError),
	#[error(display = "{}", _0)] IOError(#[error(source)] std::io::Error),
	#[error(display = "{}", _0)] ObjError(#[error(source)] ObjError),
	#[error(display = "{}", _0)] ImageError(#[error(source)] ImageError),
}
