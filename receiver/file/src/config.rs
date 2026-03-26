use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub reader: Reader,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Reader {
	pub name: String,
	pub queue: Queue,
	pub file: File,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct File {
	pub path: PathBuf,

	#[serde(default)]
	pub follow: bool,

	#[serde(default = "default_interval")]
	pub interval: f32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Queue {
	#[serde(default = "u16_default_100")]
	pub max_messages: u16,

	#[serde(default = "u16_default_100")]
	pub max_seconds: u16,

	#[serde(default = "u32_default_100")]
	pub max_size: u32,
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_interval() -> f32 { 3600.0 }
fn u16_default_100() -> u16 { 100 }
fn u32_default_100() -> u32 { 100 }
