use std::fmt;

use base64::Engine;
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

	/// Authentication when calling this webhook
	pub auth: Option<Authentication>,
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_path() -> String { "/".to_string() }
fn default_kind() -> String { "POST".to_string() }
fn default_port() -> u16 { 8080 }

/// Authentication on an Endpoint
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	/// No Authenticaton
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
impl fmt::Display for Authentication {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Authentication::None => write!(f, "None"),
			Authentication::Basic { user, pass } => write!(f, "Basic '{}: {}****'", user, pass.get(0..3).unwrap_or_default()),
			Authentication::Bearer(_) => write!(f, "Bearer ****"),
			Authentication::Header { name, value } => write!(f, "Header '{}: {}****'", name, value.get(0..3).unwrap_or_default()),
		}
	}
}
impl Authentication {
	pub fn check(&self, auth: Option<&String>) -> bool {
		match &self {
			&Self::None => {
				let empty = String::new();
				auth.unwrap_or(&empty).is_empty()
			},

			&Self::Basic { user, pass } if let Some(val) = auth => {
				let parts: Vec<_> = val.split(':').collect();
				if parts.len() >= 2 {
					let val_name = parts.get(0).unwrap().to_string();
					let val_value = parts.get(1..).unwrap().join(":");

					let value = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user, pass));
					val_name.trim().to_lowercase() == "authorization".to_string() && format!("Basic {}", value) == val_value.trim()
				} else {
					false
				}
			},

			&Self::Bearer(token) if let Some(val) = auth => {
				let parts: Vec<_> = val.split(':').collect();
				if parts.len() >= 2 {
					let val_name = parts.get(0).unwrap().to_string();
					let val_value = parts.get(1..).unwrap().join(":");
					let val_token = val_value.trim().strip_prefix("Bearer ").unwrap_or(val_value.as_str()).trim();

					let check_token = token.strip_prefix("Bearer ").unwrap_or(token);
					val_name.trim().to_lowercase() == "authorization".to_string() && val_token == check_token
				} else {
					false
				}
			},

			&Self::Header { name, value } if let Some(val) = auth => {
				let parts: Vec<_> = val.split(':').collect();
				if parts.len() >= 2 {
					let val_name = parts.get(0).unwrap().to_string();
					let val_value = parts.get(1..).unwrap().join(":");

					*name.to_lowercase() == val_name.trim().to_lowercase() && *value == val_value.trim()
				} else {
					false
				}
			},

			_ => false,
		}
	}
}

#[cfg(test)]
mod test {
	use base64::Engine;

use crate::config::Authentication;

	#[test]
	fn test_auth_none() {
		let check = None;
		let res = Authentication::None.check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_ok_none() {
		let check = Some("".to_string());
		let res = Authentication::None.check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_nok_none() {
		let check = Some("nok".to_string());
		let res = Authentication::None.check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_ok_bearer_no_prefix() {
		let token = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("Authorization: Bearer {}", token).to_string());
		let res = Authentication::Bearer(token.to_string()).check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_ok_bearer_prefix() {
		let token = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("Authorization: Bearer {}", token).to_string());
		let res = Authentication::Bearer(format!("Bearer {}", token).to_string()).check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_nok_bearer_no_prefix() {
		let token = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("Authorization: Bearer XX{}", token).to_string());
		let res = Authentication::Bearer(token.to_string()).check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_nok_bearer_prefix() {
		let token = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("Authorization: Bearer XX{}", token).to_string());
		let res = Authentication::Bearer(format!("Bearer {}", token).to_string()).check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_ok_header() {
		let name = "X-Auth-Header";
		let value = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("{}: {}", name, value).to_string());
		let res = Authentication::Header{ name: name.to_string(), value: value.to_string() }.check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_ok_header_lower() {
		let name = "X-Auth-Header";
		let value = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("{}: {}", name.to_lowercase(), value).to_string());
		let res = Authentication::Header{ name: name.to_string(), value: value.to_string() }.check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_nok_header_value() {
		let name = "X-Auth-Header";
		let value = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("{}: XX{}", name, value).to_string());
		let res = Authentication::Header{ name: name.to_string(), value: value.to_string() }.check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_nok_header_name() {
		let name = "X-Auth-Header";
		let value = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("{}-X: {}", name, value).to_string());
		let res = Authentication::Header{ name: name.to_string(), value: value.to_string() }.check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_nok_header_format() {
		let name = "X-Auth-Header";
		let value = "JustSomeRandomFoobarTokenString";
		let check = Some(format!("{}={}", name, value).to_string());
		let res = Authentication::Header{ name: name.to_string(), value: value.to_string() }.check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_ok_basic() {
		let name = "user@example.com";
		let value = "JustSomeRandomFoobarTokenString";
		let enc = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", name, value));
		let check = Some(format!("Authorization: Basic {}", enc).to_string());
		let res = Authentication::Basic{ user: name.to_string(), pass: value.to_string() }.check(check.as_ref());
		assert!(res);
	}

	#[test]
	fn test_auth_nok_basic_header() {
		let name = "user@example.com";
		let value = "JustSomeRandomFoobarTokenString";
		let enc = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", name, value));
		let check = Some(format!("X-Authorization: Basic {}", enc).to_string());
		let res = Authentication::Basic{ user: name.to_string(), pass: value.to_string() }.check(check.as_ref());
		assert!(!res);
	}

	#[test]
	fn test_auth_nok_basic_auth() {
		let name = "valid@example.com";
		let value = "JustSomeRandomFoobarTokenString";
		let enc = base64::engine::general_purpose::STANDARD.encode(format!("in{}:{}", name, value));
		let check = Some(format!("Authorization: Basic {}", enc).to_string());
		let res = Authentication::Basic{ user: name.to_string(), pass: value.to_string() }.check(check.as_ref());
		assert!(!res);
	}

}
