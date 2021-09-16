use std::error::Error;
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader, Seek};
use std::fs::File;
use std::fmt::{Display, Formatter};
use std::ffi::{OsStr, OsString};

pub fn find_asset_path(path: impl AsRef<Path>) -> PathBuf {
	let orig_path = Path::new("assets").join(path.as_ref());
	let override_path = Path::new("assets_overrides").join(path.as_ref());
	
	if override_path.exists() {
		override_path
	} else {
		orig_path
	}
}

pub fn find_asset(path: impl AsRef<Path>) -> Result<impl BufRead + Seek, AssetError> {
	let asset_path = find_asset_path(&path);
	
	match File::open(asset_path) {
		Ok(file) => Ok(BufReader::new(file)),
		Err(err) => Err(AssetError::from_err(err, path.as_ref().to_string_lossy().to_string())),
	}
}

// TODO: Use this
// Windows why
fn lookup_windows_path(root: &PathBuf, orig_path: &str) -> Result<PathBuf, AssetError> {
	if cfg!(target_os = "windows") {
		return Ok(root.join(orig_path));
	}
	
	let mut path = PathBuf::from(orig_path.replace("\\", "/"));
	let file_name = path.file_name().ok_or_else(|| AssetError::new(orig_path.to_string()))?.to_owned();
	path.pop();
	
	let mut cur_dir = root.clone();
	
	for component in path.components() {
		cur_dir.push(lookup_component(&cur_dir, component.as_os_str(), true)?);
	}
	
	cur_dir.push(lookup_component(&cur_dir, &file_name, false)?);
	
	Ok(cur_dir)
}

fn lookup_component(cur_dir: &PathBuf, name: &OsStr, dir: bool) -> Result<OsString, AssetError> {
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
		(Err(err), _) => Err(AssetError::from_err(err, cur_dir.join(name).to_string_lossy().to_string())),
		_ => Err(AssetError::new(cur_dir.join(name).to_string_lossy().to_string())),
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
