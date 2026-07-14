use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};

// The main configuration for a webhook
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Webhook,
}

/// Represents a Webhook Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Webhook {
	/// Name of the webhook
	pub name: String,

	/// Listener Configuration
	pub listen: Server,

	/// Different routes on the listener with different parsers etc.
	pub routes: Vec<Route>,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default)]
	pub parser: Vec<Parser>,
}

/// Server Listener Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
	/// Address to listen on: '0.0.0.0'
	pub address: String,

	/// Port to listen on: 8080
	#[serde(default = "default_port")]
	pub port: u16,
}
impl Server {
	/// Returns the address to listen on: IP:PORT
	pub fn get_address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}
}

/// A Route-Configuration for a Webhook/Webserver
#[derive(Debug, Deserialize, Serialize)]
pub struct Route {
	/// Path where to listen
	#[serde(default = "default_path")]
	pub path: String,

	/// How to listen: GET or POST
	#[serde(default = "default_kind")]
	pub kind: String,
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_path() -> String { "/".to_string() }
fn default_kind() -> String { "POST".to_string() }
fn default_port() -> u16 { 8080 }
