pub mod errors;
pub mod parser;
pub mod queue;

use serde::de::DeserializeOwned;
use tracing_subscriber::EnvFilter;
use std::{fs, path::Path};
use anyhow::Context;
use toml;

pub fn init_logging() {
	let filter = EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| EnvFilter::new("debug"));

	tracing_subscriber::fmt()
		.json()
		.with_env_filter(filter)
		.with_file(true)
		.with_line_number(true)
		.with_level(true)
		.with_target(true)
		.init();
}

pub fn load_config<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> anyhow::Result<T> {
	let path_ref = path.as_ref();
	let content = fs::read_to_string(path_ref).with_context(|| format!("reading config file {}", path_ref.display()))?;
	let conf: T = toml::from_str(&content).with_context(|| format!("parsing config file {}", path_ref.display()))?;
	Ok(conf)
}


#[cfg(test)]
mod tests {
	//use super::*;

	#[test]
	fn it_works() {
		let result = 4;
		assert_eq!(result, 4);
	}
}
