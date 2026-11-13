use serde::{Deserialize, Serialize};
use shared::types::{ConfStruct, ConfType, leak_str};
use shared::types::{Parser, Queue};

// Default-Wrapper Functions for Serde::Deserialize
fn default_path() -> String {
	"/".to_string()
}
fn default_kind() -> String {
	"POST".to_string()
}
fn default_port() -> u16 {
	8080
}

// The main configuration for a webhook
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Webhook,
}

#[cfg(feature = "types")]
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		ConfStruct::from([("config".to_string(), ConfType::Struct(val.config.into()))])
	}
}

/// Represents a Webhook Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Webhook {
	/// Name of the webhook
	pub name: String,

	/// Listener Configuration
	pub listen: Server,

	/// Different routes on the listener with different parsers etc.
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub routes: Vec<Route>,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub parser: Vec<Parser>,
}

#[cfg(feature = "types")]
impl From<Webhook> for ConfStruct {
	fn from(val: Webhook) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak_str(&val.name))),
			("listen".to_string(), ConfType::Struct(val.listen.into())),
			(
				"routes".to_string(),
				ConfType::Option(Box::new(ConfType::Vec(Box::new(ConfType::Struct(
					Route::default().into(),
				))))),
			),
			(
				"queue".to_string(),
				ConfType::Option(Box::new(ConfType::Struct(val.queue.into()))),
			),
			(
				"parser".to_string(),
				ConfType::Option(Box::new(ConfType::Vec(Box::new(ConfType::Struct(
					Parser::default().into(),
				))))),
			),
		])
	}
}

/// Server Listener Configuration
#[cfg(feature = "types")]
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

#[cfg(feature = "types")]
impl Default for Server {
	fn default() -> Self {
		Self {
			address: String::from("0.0.0.0"),
			port: default_port(),
		}
	}
}

#[cfg(feature = "types")]
impl From<Server> for ConfStruct {
	fn from(val: Server) -> Self {
		ConfStruct::from([
			(
				"address".to_string(),
				ConfType::String(leak_str(&val.address)),
			),
			(
				"port".to_string(),
				ConfType::Option(Box::new(ConfType::UInt(val.port.into()))),
			),
		])
	}
}

/// A Route-Configuration for a Webhook/Webserver
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
pub struct Route {
	/// Path where to listen
	#[serde(default = "default_path")]
	pub path: String,

	/// How to listen: GET or POST
	#[serde(default = "default_kind")]
	pub kind: String,

	/// Authentication when calling this webhook
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub auth: Option<Authentication>,
}

#[cfg(feature = "types")]
impl Default for Route {
	fn default() -> Self {
		Self {
			path: default_path(),
			kind: default_kind(),
			auth: None,
		}
	}
}

#[cfg(feature = "types")]
impl From<Route> for ConfStruct {
	fn from(val: Route) -> Self {
		let auth = val.auth.unwrap_or_default();
		ConfStruct::from([
			(
				"path".to_string(),
				ConfType::Option(Box::new(ConfType::String(leak_str(&val.path)))),
			),
			(
				"kind".to_string(),
				ConfType::Option(Box::new(ConfType::String(leak_str(&val.kind)))),
			),
			(
				"auth".to_string(),
				ConfType::Option(Box::new(ConfType::Enum(auth.clone().into(), auth.name()))),
			),
		])
	}
}

/// Authentication on an Endpoint
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	/// No Authenticaton
	#[default]
	None,

	/// Basic-Auth with a User and Password
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Basic { user: String, pass: String },

	/// A Bearer Token
	/// With/out 'Bearer' prefix
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Bearer(String),

	/// A Simple Header
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Header { name: String, value: String },
}

#[cfg(feature = "types")]
impl Authentication {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::None => "None",
			Self::Basic { .. } => "Basic",
			Self::Bearer(_) => "Bearer",
			Self::Header { .. } => "Header",
		}
	}
}

#[cfg(feature = "types")]
impl From<Authentication> for ConfStruct {
	fn from(_val: Authentication) -> Self {
		ConfStruct::from([
			("None".to_string(), ConfType::EnumValue),
			(
				"Basic".to_string(),
				ConfType::Struct(ConfStruct::from([
					("user".to_string(), ConfType::String("")),
					("pass".to_string(), ConfType::String("")),
				])),
			),
			("Bearer".to_string(), ConfType::String("")),
			(
				"Header".to_string(),
				ConfType::Struct(ConfStruct::from([
					("name".to_string(), ConfType::String("")),
					("value".to_string(), ConfType::String("")),
				])),
			),
		])
	}
}

#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	fn string_of(conf: &ConfStruct, name: &str) -> Option<&'static str> {
		match conf.get(name) {
			Some(ConfType::String(s)) => Some(s),
			Some(ConfType::Option(inner)) => match inner.as_ref() {
				ConfType::String(s) => Some(s),
				_ => None,
			},
			_ => None,
		}
	}

	#[test]
	fn test_server_uses_values() {
		let conf: ConfStruct = Server {
			address: "127.0.0.1".to_string(),
			port: 9000,
		}
		.into();
		assert_eq!(string_of(&conf, "address"), Some("127.0.0.1"));
		assert_eq!(
			conf.get("port"),
			Some(&ConfType::Option(Box::new(ConfType::UInt(9000))))
		);
	}

	#[test]
	fn test_server_default_values() {
		let conf: ConfStruct = Server::default().into();
		assert_eq!(string_of(&conf, "address"), Some("0.0.0.0"));
		assert_eq!(
			conf.get("port"),
			Some(&ConfType::Option(Box::new(ConfType::UInt(8080))))
		);
	}

	#[test]
	fn test_route_uses_values() {
		let conf: ConfStruct = Route {
			path: "/hook".to_string(),
			kind: "GET".to_string(),
			auth: Some(Authentication::Bearer("token".to_string())),
		}
		.into();
		assert_eq!(string_of(&conf, "path"), Some("/hook"));
		assert_eq!(string_of(&conf, "kind"), Some("GET"));
		assert!(
			matches!(conf.get("auth"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "Bearer")))
		);
	}

	#[test]
	fn test_route_default_values() {
		let conf: ConfStruct = Route::default().into();
		assert_eq!(string_of(&conf, "path"), Some("/"));
		assert_eq!(string_of(&conf, "kind"), Some("POST"));
		assert!(
			matches!(conf.get("auth"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "None")))
		);
	}

	#[test]
	fn test_authentication_does_not_expose_secrets() {
		let auth = Authentication::Basic {
			user: "me".to_string(),
			pass: "secret".to_string(),
		};
		assert_eq!(auth.name(), "Basic");

		let conf: ConfStruct = auth.into();
		let Some(ConfType::Struct(basic)) = conf.get("Basic") else {
			panic!("missing Basic variant")
		};
		assert_eq!(string_of(basic, "user"), Some(""));
		assert_eq!(string_of(basic, "pass"), Some(""));
	}

	#[test]
	fn test_webhook_uses_nested_values() {
		let conf: ConfStruct = Config {
			config: Webhook {
				name: "hook".to_string(),
				..Webhook::default()
			},
		}
		.into();

		let Some(ConfType::Struct(webhook)) = conf.get("config") else {
			panic!("missing config struct")
		};
		assert_eq!(string_of(webhook, "name"), Some("hook"));
	}
}
