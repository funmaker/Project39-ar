use std::fmt::Display;
use std::str::FromStr;
pub use project39_ar_derive::FromArgs;
use anyhow::{Result, Error};
use getopts::{Options, Matches};
use nalgebra::{Matrix, Scalar, DefaultAllocator, DimName};
use nalgebra::allocator::Allocator;


pub trait FromArgs {
	fn hint(&self) -> String {
		String::new()
	}
	
	fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String {
		let short_flag = if short.len() > 0 {
			format!("-{}, ", short)
		} else {
			"    ".to_string()
		};
		
		format!("\t{}--{} {}\t{}", short_flag, path, self.hint(), doc)
	}
	
	fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<()> {
		opts.optopt(short, path, doc, &self.hint());
		Ok(())
	}
	
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<()>;
}

impl FromArgs for bool {
	fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String {
		let short_flag = if short.len() > 0 {
			format!("-{}, ", short)
		} else {
			"    ".to_string()
		};
		
		if *self {
			let (d1, d2) = doc.split_at(1);
			format!("\t{}--no-{}\tDo not {}{}", short_flag.to_uppercase(), path, d1.to_lowercase(), d2)
		} else {
			format!("\t{}--{}\t{}", short_flag, path, doc)
		}
	}
	
	fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<()> {
		let neg = format!("no-{}", path);
		let short_neg = short.to_uppercase();
		opts.optflag(short, path, doc);
		opts.optflag(&short_neg, &neg, doc);
		Ok(())
	}
	
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<()> {
		let neg = format!("no-{}", path);
		if matches.opt_present(path) { *self = true; }
		if matches.opt_present(&neg) { *self = false; }
		Ok(())
	}
}

impl<T, R: DimName, C: DimName> FromArgs for Matrix<T, R, C, <DefaultAllocator as Allocator<T, R, C>>::Buffer>
	where T: Scalar + Display + FromStr,
	      DefaultAllocator: Allocator<T, R, C>,
	      <T as FromStr>::Err: std::error::Error + Send + Sync {
	
	fn hint(&self) -> String {
		let mut ret = String::new();
		
		for (y, row) in self.row_iter().enumerate() {
			if y > 0 { ret += ";" }
			for (x, cel) in row.iter().enumerate() {
				if x > 0 { ret += "," }
				ret += &format!("{:.2}", cel);
			}
		}
		
		ret
	}
	
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<()> {
		if let Some(str) = matches.opt_str(path) {
			let rows: Vec<Vec<T>> = str.split(";")
			                           .map(|row| row.split(",")
			                                         .map(T::from_str)
			                                         .collect::<Result<Vec<_>, _>>())
			                           .collect::<Result<_, _>>()
			                           .map_err(|err| Error::new(err).context(format!("Failed to parse cli argument {}", path.to_string())))?;
			
			if rows.len() != R::dim() {
				return Err(Error::msg(format!("Wrong row count (expected {}, got {})", R::dim(), rows.len())));
			}
			
			for row in &rows {
				if row.len() != rows[0].len() {
					return Err(Error::msg("Inconsistent row length"));
				}
			}
			
			if rows[0].len() != C::dim() {
				return Err(Error::msg(format!("Wrong column count (expected {}, got {})", C::dim(), rows[0].len())));
			}
			
			*self = Self::from_iterator(rows.into_iter().flatten());
		}
		
		Ok(())
	}
}

macro_rules! args_terminals {
	{ $( $typ:ty )* } => {
		$(
			impl FromArgs for $typ {
				fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<()> {
					if let Some(str) = matches.opt_str(path) {
						*self = str.parse::<Self>()
						           .map_err(|err| anyhow::Error::new(err).context(format!("Failed to parse cli argument {}", path.to_string())))?;
					}
					Ok(())
				}
			}
		)*
	}
}

pub(crate) use args_terminals;

args_terminals! { f32 f64 u8 i8 u16 i16 u32 i32 u64 i64 usize isize }
