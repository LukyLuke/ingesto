use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub receiver: Receiver,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Receiver {
	pub name: String,
	pub listen: Server,
	pub queue: Queue,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
	pub address: String,

	#[serde(default = "u16_default_514")]
	pub port: u16,

	#[serde(default = "default_udp")]
	pub kind: String
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

// Default-Wrapper Functions for Serde::Deserialize
fn default_udp() -> String { String::from("UDP") }
fn u16_default_514() -> u16 { 514 }
fn u16_default_100() -> u16 { 100 }
fn u32_default_100() -> u32 { 100 }
