pub mod errors;
pub mod types;

#[cfg(feature = "runtime")]
pub mod parser;
#[cfg(feature = "runtime")]
pub mod queue;
#[cfg(feature = "runtime")]
pub mod receiver;
#[cfg(feature = "runtime")]
pub mod template;

#[cfg(feature = "runtime")]
use clap::{Arg, Command, builder::{PathBufValueParser}};

use serde::de::DeserializeOwned;
use tracing::{debug, error};
use std::{fs, path::Path};
use anyhow::{Context, anyhow};
use toml;


/// Initialize global logging
/// Set the environment `RUST_LOG` to `debug|info|error` for the loglevel
#[cfg(feature = "runtime")]
pub fn init_logging() {
	let filter = tracing_subscriber::EnvFilter::try_from_default_env()
		.unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"));

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

	let file_ext = path_ref.extension()
		.and_then(|s| s.to_str())
		.map(|s| s.to_ascii_lowercase());

	match file_ext.as_deref() {
		Some("toml") => serde_path_to_error::deserialize(toml::Deserializer::parse(&content)?)
			.map_err(|err| anyhow!("parsing config at '{}' with error {}", err.path().to_string(), err.inner().message())),

		Some("yaml") | Some("yml") => serde_path_to_error::deserialize(serde_yaml::Deserializer::from_str(&content))
			.map_err(|err| anyhow!("parsing config at '{}' with error {}", err.path().to_string(), err.inner())),

		_ => Err(anyhow!("Unknown configuration format: {:?}", path_ref.extension())),
	}
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
		match fs::read_to_string(file) {
			Ok(content) if !content.is_empty() => {
				let line = content.lines().next().unwrap_or_default().to_string();
				debug!(message="secrets_string", variant="file", key=val, val=mask(&line, 3) );
				return Ok(line);
			},
			Err(e) => { error!(message="secrets_string", variant="file", key=val, err=%e ); },
			_ => { error!(message="secrets_string", variant="file", key=val, err="empty file" ); },
		}

	} else if val.starts_with("env:") && let Some(env) = val.get(4..) {
		if let Ok(line) = env::var(env) {
			debug!(message="secrets_string", variant="env", key=val, val=mask(&line, 3) );
			return Ok(line.to_string());
		}
	}
	debug!(message="secrets_string", variant="none", key=val );
	Ok(val.to_owned())
}

/// Simply shows the usage of the program and returns the path to a possible given config file
#[cfg(feature = "runtime")]
pub fn usage() -> anyhow::Result<std::path::PathBuf> {
	let matches = Command::new("Ingesto")
		.about("Log-Ingestion from various sources into various destinations in various formats.")
		.arg(Arg::new("config_file")
			.default_value("config.toml")
			.value_parser(PathBufValueParser::default())
			.short('c')
			.long("config")
			.help("Configuration file to use (toml or yaml)"))
		.get_matches();

	let f: &std::path::PathBuf = matches.get_one("config_file").unwrap();
	return Ok(f.to_path_buf())
}

/// Masks a string with '*' and only shows the first couple chars
///
/// # Arguments
///
/// * `val` - value to mask
/// * `num` - number of chars to show as plain text at the start
///
/// # Examples
///
/// ```
/// let val = "This is a Secret String";
/// let masked = shared::mask(val, 6);
/// println!("Masked: {}", masked); // Prints `Masked: This i*****************`
/// ```
///
/// # Returns
///
/// A masked string like "Password" -> "Pas****"
pub fn mask(val: &str, num: usize) -> String {
	val.chars()
		.enumerate()
		.map(|(i, c)| { if i > num { '*' } else { c } })
		.collect::<String>()
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_secrets_string_file() {
		let res = secrets_string("file:/../LICENSE-MIT");

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
		let not_a_file = secrets_string("file:/LICENSE-MIT");
		let only_string = secrets_string("LICENSE");

		assert!(not_a_file.is_ok());
		assert!(not_a_file.unwrap().starts_with("file:/"));

		assert!(only_string.is_ok());
		assert!(only_string.unwrap().starts_with("LICENSE"));
	}
}
