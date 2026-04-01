use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub webhook: Webhook,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Webhook {
	pub name: String,
	pub listen: Server,
	pub routes: Vec<Route>,
	pub queue: Queue,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
	pub address: String,

	#[serde(default = "default_port")]
	pub port: u16,
}

impl Server {
	pub fn get_address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}
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

#[derive(Debug, Deserialize, Serialize)]
pub struct Route {
	#[serde(default = "default_path")]
	pub path: String,

	#[serde(default = "default_kind")]
	pub kind: String,
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_path() -> String { "/".to_string() }
fn default_kind() -> String { "POST".to_string() }
fn default_port() -> u16 { 8080 }
fn u16_default_100() -> u16 { 100 }
fn u32_default_100() -> u32 { 100 }
