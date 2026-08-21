use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
fn default_path() -> String { "/".to_string() }
fn default_kind() -> String { "POST".to_string() }
fn default_port() -> u16 { 8080 }


// The main configuration for a webhook
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Webhook,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Config {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(Webhook::default().into())),
		])
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
impl Into<ConfStruct> for Webhook {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("listen".to_string(), ConfType::Struct( Server::default().into() )),
			("routes".to_string(), ConfType::Vec( Box::new( ConfType::Struct( Route::default().into() ) ) )),
			("queue".to_string(), ConfType::Struct( Queue::default().into() )),
			("parser".to_string(), ConfType::Vec( Box::new( ConfType::Struct( Parser::default().into() ) ) )),
		])
	}
}

/// Server Listener Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
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
impl Into<ConfStruct> for Server {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("address".to_string(), ConfType::String),
			("port".to_string(), ConfType::UInt),
		])
	}
}


/// A Route-Configuration for a Webhook/Webserver
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
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
impl Into<ConfStruct> for Route {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("path".to_string(), ConfType::String),
			("kind".to_string(), ConfType::String),
			("auth".to_string(), ConfType::Option( Box::new( ConfType::Enum(Authentication::None.into()) ) )),
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
impl Into<ConfStruct> for Authentication {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("None".to_string(), ConfType::EnumValue),
			("Basic".to_string(), ConfType::EnumParams("user", "pass")),
			("Bearer".to_string(), ConfType::String),
			("Header".to_string(), ConfType::EnumParams("header", "value")),
		])
	}
}
