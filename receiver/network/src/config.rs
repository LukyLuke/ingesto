use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};

/// The main network listener Configuraiton
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Receiver,
}

/// A network-Receiver Configuraiton
#[derive(Debug, Deserialize, Serialize)]
pub struct Receiver {
	/// Name of the listener
	pub name: String,

	/// Network-Listener Configuraiton
	pub listen: Server,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default)]
	pub parser: Vec<Parser>,
}

/// A Network-Listener Configuraiton
#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
	/// Address to listen on: '0.0.0.0'
	pub address: String,

	/// Port to listen on: 514
	#[serde(default = "u16_default_514")]
	pub port: u16,

	/// Listener-Kind: TCP, UDP
	#[serde(default = "default_udp")]
	pub kind: String
}
impl Server {
	/// Returns the address to listen on: IP:PORT
	pub fn get_address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_udp() -> String { String::from("UDP") }
fn u16_default_514() -> u16 { 514 }
