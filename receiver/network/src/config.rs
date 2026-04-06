use serde::{Deserialize, Serialize};
use shared::{parser::Parser, queue::Queue};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub receiver: Receiver,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Receiver {
	pub name: String,
	pub listen: Server,
	pub queue: Queue,
	pub parser: Vec<Parser>,
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

// Default-Wrapper Functions for Serde::Deserialize
fn default_udp() -> String { String::from("UDP") }
fn u16_default_514() -> u16 { 514 }
