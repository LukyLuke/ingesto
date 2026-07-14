use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};

/// The main File-Reader Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Reader,
}

/// A File-Reader Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Reader {
	/// Name for this File-Reader
	pub name: String,

	/// The File to read and listen on
	pub file: File,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default)]
	pub parser: Vec<Parser>,
}

/// A File-Reader Configuration
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

// Default-Wrapper Functions for Serde::Deserialize
fn default_interval() -> f32 { 3600.0 }
