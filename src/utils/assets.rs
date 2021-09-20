use std::error::Error;
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, Seek};
use std::fs::File;
use std::fmt::{Display, Formatter};
use std::ffi::{OsStr, OsString};

pub fn find_asset(path: impl AsRef<Path>) -> Result<impl BufRead + Seek, AssetError> {
	let asset_path = find_asset_path(&path)?;
	
	match File::open(asset_path) {
		Ok(file) => Ok(BufReader::new(file)),
		Err(err) => Err(AssetError::from_err(err, path.as_ref().to_string_lossy())),
	}
}

pub fn find_asset_path(path: impl AsRef<Path>) -> Result<PathBuf, AssetError> {
	let orig_path = lookup_windows_path("assets", path.as_ref());
	let override_path = lookup_windows_path("assets_overrides", path.as_ref());
	
	match (orig_path, override_path) {
		(_, Ok(path)) if path.exists() => Ok(path),
		(Ok(path), _) => Ok(path),
		(Err(err), _) => Err(err),
	}
}

// Windows why
fn lookup_windows_path(root: &str, orig_path: &Path) -> Result<PathBuf, AssetError> {
	if cfg!(target_os = "windows") {
		return Ok(PathBuf::from(root).join(orig_path));
	}
	
	let mut cur_dir = PathBuf::from(root);
	let mut path = PathBuf::from(orig_path.to_string_lossy().replace("\\", "/"));
	let full_path = cur_dir.join(&path);
	let file_name = path.file_name().ok_or_else(|| AssetError::new(orig_path.to_string_lossy()))?.to_owned();
	path.pop();
	
	for component in path.components() {
		cur_dir.push(lookup_component(&cur_dir, component.as_os_str(), &full_path, true)?);
	}
	
	cur_dir.push(lookup_component(&cur_dir, &file_name, &full_path, false)?);
	
	Ok(cur_dir)
}

fn lookup_component(cur_dir: &PathBuf, name: &OsStr, full_path: &PathBuf, dir: bool) -> Result<OsString, AssetError> {
	let mut next_dir = None;
	
	let result = try {
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
	};
	
	match (result, next_dir) {
		(Ok(()), Some(next_dir)) => Ok(next_dir),
		(Err(err), _) => Err(AssetError::from_err(err, full_path.to_string_lossy())),
		_ => Err(AssetError::new(full_path.to_string_lossy())),
	}
}

#[derive(Debug)]
pub struct AssetError {
	inner: Option<std::io::Error>,
	path: String,
}

impl AssetError {
	fn new(path: impl ToString) -> Self {
		AssetError {
			inner: None,
			path: path.to_string(),
		}
	}
	
	fn from_err(err: std::io::Error, path: impl ToString) -> Self {
		AssetError {
			inner: Some(err),
			path: path.to_string(),
		}
	}
}

impl Display for AssetError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		match &self.inner {
			None => write!(f, "Unable to load asset `{}`", self.path),
			Some(inner) => write!(f, "Unable to load asset `{}`: {}", self.path, inner),
		}
	}
}

impl Error for AssetError {}
