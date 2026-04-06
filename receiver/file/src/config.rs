use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shared::{parser::Parser, queue::Queue};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub reader: Reader,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Reader {
	pub name: String,
	pub file: File,
	pub queue: Queue,
	pub parser: Vec<Parser>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
	pub path: PathBuf,

	#[serde(default)]
	pub follow: bool,

	#[serde(default = "default_interval")]
	pub interval: f32,
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_interval() -> f32 { 3600.0 }
