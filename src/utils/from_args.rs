use std::error::Error;
use err_derive::Error;
use getopts::{Options, Matches};
pub use project39_ar_derive::FromArgs;

pub trait FromArgs {
	fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String;
	fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<(), ArgsError>;
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError>;
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
	
	fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<(), ArgsError> {
		let neg = format!("no-{}", path);
		let short_neg = short.to_uppercase();
		opts.optflag(short, path, doc);
		opts.optflag(&short_neg, &neg, doc);
		Ok(())
	}
	
	fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError> {
		let neg = format!("no-{}", path);
		if matches.opt_present(path) { *self = true; }
		if matches.opt_present(&neg) { *self = false; }
		Ok(())
	}
}

macro_rules! args_terminals {
	{ $( $typ:ty )* } => {
		$(
			impl FromArgs for $typ {
				fn usage_impl(&self, short: &str, path: &str, doc: &str) -> String {
					let short_flag = if short.len() > 0 {
						format!("-{}, ", short)
					} else {
						"    ".to_string()
					};
					
					format!("\t{}--{} {}\t{}", short_flag, path, self, doc)
				}
				
				fn prepare_opts(&mut self, opts: &mut Options, short: &str, path: &str, doc: &str) -> Result<(), ArgsError> {
					opts.optopt(short, path, doc, &self.to_string());
					Ok(())
				}
				
				fn apply_matches(&mut self, matches: &Matches, path: &str) -> Result<(), ArgsError> {
					if let Some(str) = matches.opt_str(path) {
						*self = str.parse::<Self>()
						           .map_err(|err| ArgsError::BadArgument(path.to_string(), err.into()))?;
					}
					Ok(())
				}
			}
		)*
	}
}

pub(crate) use args_terminals;

args_terminals! { f32 f64 u8 i8 u16 i16 u32 i32 u64 i64 usize isize }

#[derive(Debug, Error)]
pub enum ArgsError {
	#[error(display = "{}", _0)] GetoptsError(#[error(source)] getopts::Fail),
	#[error(display = "Failed to parse cli argument {}: {}", _0, _1)] BadArgument(String, Box<dyn Error>),
}

