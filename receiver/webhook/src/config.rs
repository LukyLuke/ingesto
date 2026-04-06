use serde::{Deserialize, Serialize};
use shared::{parser::Parser, queue::Queue};

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
	pub parser: Vec<Parser>,
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
