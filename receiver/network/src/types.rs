#[cfg(feature = "types")]
use std::fmt;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};
use shared::types::{ConfStruct, ConfType, leak_str};

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
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(val.config.into())),
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
impl From<Receiver> for ConfStruct {
	fn from(val: Receiver) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak_str(&val.name))),
			("listen".to_string(), ConfType::Struct(val.listen.into())),
			("queue".to_string(), ConfType::Option( Box::new( ConfType::Struct( val.queue.into() ) ) )),
			("parser".to_string(), ConfType::Option( Box::new( ConfType::Vec( Box::new( ConfType::Struct( Parser::default().into() ) ) ) ) )),
		])
	}
}

/// A Network-Listener Configuraiton
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
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
impl Default for Server {
	fn default() -> Self {
		Self {
			address: String::from("0.0.0.0"),
			port: u16_default_514(),
			kind: default_udp(),
		}
	}
}

#[cfg(feature = "types")]
impl From<Server> for ConfStruct {
	fn from(val: Server) -> Self {
		let kind = val.kind.name();
		ConfStruct::from([
			("address".to_string(), ConfType::String(leak_str(&val.address))),
			("port".to_string(), ConfType::Option( Box::new( ConfType::UInt( val.port.into() ) ) )),
			("kind".to_string(), ConfType::Option( Box::new( ConfType::Enum( val.kind.into(), kind ) ) )),
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
impl Protocol {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::TCP => "TCP",
			Self::UDP => "UDP",
		}
	}
}

#[cfg(feature = "types")]
impl From<Protocol> for ConfStruct {
	fn from(_val: Protocol) -> Self {
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


#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	#[test]
	fn test_server_uses_values() {
		let conf: ConfStruct = Server { address: "10.0.0.1".to_string(), port: 1514, kind: Protocol::TCP }.into();
		assert_eq!(conf.get("address"), Some(&ConfType::String("10.0.0.1")));
		assert_eq!(conf.get("port"), Some(&ConfType::Option(Box::new(ConfType::UInt(1514)))));
		assert!(matches!(conf.get("kind"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "TCP"))));
	}

	#[test]
	fn test_server_default_values() {
		let conf: ConfStruct = Server::default().into();
		assert_eq!(conf.get("address"), Some(&ConfType::String("0.0.0.0")));
		assert_eq!(conf.get("port"), Some(&ConfType::Option(Box::new(ConfType::UInt(514)))));
		assert!(matches!(conf.get("kind"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "UDP"))));
	}

	#[test]
	fn test_receiver_uses_nested_values() {
		let conf: ConfStruct = Config {
			config: Receiver { name: "syslog".to_string(), ..Receiver::default() },
		}.into();

		let Some(ConfType::Struct(receiver)) = conf.get("config") else { panic!("missing config struct") };
		assert_eq!(receiver.get("name"), Some(&ConfType::String("syslog")));
		assert!(receiver.get("listen").is_some());
	}

	#[test]
	fn test_protocol_names() {
		assert_eq!(Protocol::TCP.name(), "TCP");
		assert_eq!(Protocol::UDP.name(), "UDP");
	}
}
