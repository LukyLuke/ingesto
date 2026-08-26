#[cfg(feature = "types")]
use std::fmt;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
fn default_udp() -> Protocol { Protocol::UDP }
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
			("name".to_string(), ConfType::String("")),
			("listen".to_string(), ConfType::Struct(Server::default().into())),
			("queue".to_string(), ConfType::Struct( Queue::default().into() )),
			("parser".to_string(), ConfType::Vec( Box::new( ConfType::Struct( Parser::default().into() ) ) )),
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
	pub kind: Protocol
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Server {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("address".to_string(), ConfType::String("0.0.0.0")),
			("port".to_string(), ConfType::UInt( u16_default_514().into() )),
			("kind".to_string(), ConfType::Enum( Protocol::UDP.into(), "UDP" )),
		])
	}
}

#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub enum Protocol {
	TCP,
	#[default]
	UDP
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Protocol {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("UDP".to_string(), ConfType::EnumValue),
			("TCP".to_string(), ConfType::EnumValue),
		])
	}
}

#[cfg(feature = "types")]
impl Display for Protocol {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

