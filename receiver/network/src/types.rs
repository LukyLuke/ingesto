use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
fn default_udp() -> String { String::from("UDP") }
fn u16_default_514() -> u16 { 514 }


/// The main network listener Configuraiton
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Receiver,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Config {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(Receiver::default().into())),
		])
	}
}

/// A network-Receiver Configuraiton
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Receiver {
	/// Name of the listener
	pub name: String,

	/// Network-Listener Configuraiton
	pub listen: Server,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub parser: Vec<Parser>,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Receiver {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("listen".to_string(), ConfType::Struct(Server::default().into())),
			("queue".to_string(), ConfType::Struct(Queue::default().into())),
			("parser".to_string(), ConfType::Struct(Parser::default().into())),
		])
	}
}

/// A Network-Listener Configuraiton
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
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

#[cfg(feature = "types")]
impl Into<ConfStruct> for Server {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("address".to_string(), ConfType::String),
			("port".to_string(), ConfType::UInt),
			("kind".to_string(), ConfType::String),
		])
	}
}
