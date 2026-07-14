pub mod errors;
pub mod parser;
pub mod queue;
pub mod receiver;
pub mod types;

use serde::de::DeserializeOwned;
use tracing_subscriber::EnvFilter;
use std::{fs, path::Path, path::PathBuf};
use anyhow::Context;
use clap::{Arg, Command, builder::{PathBufValueParser}};
use toml;

/// Initialize global logging
/// Set the environment `RUST_LOG` to `debug|info|error`
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

/// Load the configuration file and parses it into the given structure
///
/// The Variant `T` must be a `serde::DeserializeOwned` type (`#[derive(serde::Deserialize)]`).
///
/// # Arguments
///
/// * `path` - The Configuration-File to load (TOML or YAML)
///
/// # Results
///
/// A Result with the config file structure and values from the config file
pub fn load_config<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> anyhow::Result<T> {
	let path_ref = path.as_ref();
	let content = fs::read_to_string(path_ref).with_context(|| format!("reading config file {}", path_ref.display()))?;
	let conf: T = toml::from_str(&content).with_context(|| format!("parsing config file {}", path_ref.display()))?;
	Ok(conf)
}

/// Checks a string if it is a file or environment and returns the first line or variable.
/// Used for secrets in configuraton.
///
/// If the requested string starts with `file:/`, the first line of the file is returned.
/// If the reauested string starts with `env:`, the environment variable is read out and returned.
///
/// If neither, the file nor the environment can be read or does not exist, the value is returned as-is.
///
/// # Arguments
///
/// * `val` - The value to check
///
/// # Examples
///
/// ```
/// let file = shared::secrets_string("file:/LICENSE"); // Returns Ok("MIT License")
/// let env  = shared::secrets_string("env:/PATH");     // Returns Ok("PATH Variable Content")
/// let val  = shared::secrets_string("any string");    // Returns Ok("any string")
/// ```
///
/// # Returns
///
/// The first line of the file, the environment value or the requested string as-is
pub fn secrets_string(val: &str) -> anyhow::Result<String> {
	if val.starts_with("file:/") && let Some(file) = val.get(6..) {
		if let Ok(content) = fs::read_to_string(file) && !content.is_empty() {
			return Ok(content.lines().next().unwrap_or_default().to_string());
		}
	} else if val.starts_with("env:") && let Some(env) = val.get(4..) {
		let e = env::var(env);
		println!("{:?}", e);
	}
	Ok(val.to_owned())
}

pub fn usage() -> anyhow::Result<PathBuf> {
	let matches = Command::new("Ingesto")
		.about("Log-Ingestion from various sources into various destinations in various formats.")
		.arg(Arg::new("config_file")
			.default_value("config.toml")
			.value_parser(PathBufValueParser::default())
			.short('c')
			.long("config")
			.help("Configuration file to use (toml or yaml)"))
		.get_matches();

	let f: &PathBuf = matches.get_one("config_file").unwrap();
	return Ok(f.to_path_buf())
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_secrets_string_file() {
		let res = secrets_string("file:/../LICENSE");

		assert!(res.is_ok());
		assert_eq!(res.unwrap(), "MIT License");
	}

	#[test]
	fn test_secrets_string_env() {
		let res = secrets_string("env:PATH");

		assert!(res.is_ok());
		assert!(!res.unwrap().is_empty());
	}

	#[test]
	fn test_secrets_string_nok() {
		let not_a_file = secrets_string("file:/LICENSE");
		let only_string = secrets_string("LICENSE");

		assert!(not_a_file.is_ok());
		assert!(not_a_file.unwrap().starts_with("file:/"));

		assert!(only_string.is_ok());
		assert!(only_string.unwrap().starts_with("LICENSE"));
	}
}
