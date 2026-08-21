use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
fn default_interval() -> f32 { 3600.0 }


/// The main File-Reader Configuration
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Reader,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Config {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(Reader::default().into())),
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
impl Into<ConfStruct> for Reader {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("file".to_string(), ConfType::Struct(File::default().into())),
			("queue".to_string(), ConfType::Struct(Queue::default().into())),
			("parser".to_string(), ConfType::Struct(Parser::default().into())),
		])
	}
}

/// A File-Reader Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
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
impl Into<ConfStruct> for File {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("path".to_string(), ConfType::String),
			("follow".to_string(), ConfType::Bool),
			("interval".to_string(), ConfType::Float),
		])
	}
}
