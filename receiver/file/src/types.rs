use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};
use shared::types::{ConfStruct, ConfType, leak_str};

// Default-Wrapper Functions for Serde::Deserialize
fn default_interval() -> f32 { 3600.0 }


/// The main File-Reader Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Reader,
}

#[cfg(feature = "types")]
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(val.config.into())),
		])
	}
}

/// A File-Reader Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Reader {
	/// Name for this File-Reader
	pub name: String,

	/// The File to read and listen on
	pub file: File,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub parser: Vec<Parser>,
}

#[cfg(feature = "types")]
impl From<Reader> for ConfStruct {
	fn from(val: Reader) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak_str(&val.name))),
			("file".to_string(), ConfType::Struct(val.file.into())),
			("queue".to_string(), ConfType::Option( Box::new( ConfType::Struct( val.queue.into() ) ) )),
			("parser".to_string(), ConfType::Option( Box::new( ConfType::Vec( Box::new( ConfType::Struct( Parser::default().into() ) ) ) ) )),
		])
	}
}

/// A File-Reader Configuration
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
pub struct File {
	/// File name and path to open, read or listen
	pub path: PathBuf,

	/// Open and listen for new content or read the file at once.
	/// If true, the file is opened and only new lines are processed (tail -f style)
	/// If false, the whole file is read and all lines are processed
	#[serde(default)]
	pub follow: bool,

	/// If follow is false, this defines the interval to open and read the file in seconds
	/// Default to 3600 (1 hour)
	#[serde(default = "default_interval")]
	pub interval: f32,
}

#[cfg(feature = "types")]
impl Default for File {
	fn default() -> Self {
		Self {
			path: PathBuf::new(),
			follow: false,
			interval: default_interval(),
		}
	}
}

#[cfg(feature = "types")]
impl From<File> for ConfStruct {
	fn from(val: File) -> Self {
		ConfStruct::from([
			("path".to_string(), ConfType::String(leak_str(&val.path.to_string_lossy()))),
			("follow".to_string(), ConfType::Option( Box::new( ConfType::Bool(val.follow) ) )),
			("interval".to_string(), ConfType::Option( Box::new( ConfType::Float( val.interval.into() ) ) )),
		])
	}
}

#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	#[test]
	fn test_file_uses_values() {
		let conf: ConfStruct = File { path: PathBuf::from("/var/log/syslog"), follow: true, interval: 12.5 }.into();
		assert_eq!(conf.get("path"), Some(&ConfType::String("/var/log/syslog")));
		assert_eq!(conf.get("follow"), Some(&ConfType::Option(Box::new(ConfType::Bool(true)))));
		assert_eq!(conf.get("interval"), Some(&ConfType::Option(Box::new(ConfType::Float(12.5)))));
	}

	#[test]
	fn test_file_default_values() {
		let conf: ConfStruct = File::default().into();
		assert_eq!(conf.get("path"), Some(&ConfType::String("")));
		assert_eq!(conf.get("follow"), Some(&ConfType::Option(Box::new(ConfType::Bool(false)))));
		assert_eq!(conf.get("interval"), Some(&ConfType::Option(Box::new(ConfType::Float(3600.0)))));
	}

	#[test]
	fn test_reader_uses_nested_values() {
		let conf: ConfStruct = Config {
			config: Reader {
				name: "my-reader".to_string(),
				file: File { path: PathBuf::from("a.log"), ..File::default() },
				..Reader::default()
			},
		}.into();

		let Some(ConfType::Struct(reader)) = conf.get("config") else { panic!("missing config struct") };
		assert_eq!(reader.get("name"), Some(&ConfType::String("my-reader")));
		let Some(ConfType::Struct(file)) = reader.get("file") else { panic!("missing file struct") };
		assert_eq!(file.get("path"), Some(&ConfType::String("a.log")));
	}
}
