use std::fmt::{Display, Formatter};


#[derive(Eq, PartialEq, Clone, Debug)]
pub struct PatternMatcher {
	parts: Vec<String>,
}

impl PatternMatcher {
	pub fn new(pattern: impl AsRef<str>) -> Self {
		PatternMatcher {
			parts: pattern.as_ref()
			              .split("*")
			              .map(Into::into)
			              .collect()
		}
	}
	
	pub fn matches(&self, other: impl AsRef<str>) -> bool {
		let mut slice = other.as_ref();
		
		if self.parts.is_empty() {
			return slice.is_empty()
		}
		
		for (n, pat) in self.parts.iter().enumerate() {
			if n == 0 && n == self.parts.len() - 1 {
				if slice != pat { return false }
			} else if n == 0 {
				if !slice.starts_with(pat) { return false }
				slice = &slice[pat.len()..];
			} else if n == self.parts.len() - 1 {
				if !slice.ends_with(pat) { return false }
			} else {
				if let Some(location) = slice.find(pat) {
					slice = &slice[location + pat.len()..];
				} else { return false }
			}
		}
		
		true
	}
}

impl Display for PatternMatcher {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.parts.join("*").fmt(f)
	}
}
