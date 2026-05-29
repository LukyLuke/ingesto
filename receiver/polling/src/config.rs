use core::fmt;

use serde::{Deserialize, Serialize};
use shared::types::{Parser, Queue};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub polling: Polling,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Polling {
	pub name: String,
	pub api: Endpoint,

	#[serde(default = "default_cron_timer")]
	pub timer: String,

	#[serde(default)]
	pub queue: Queue,

	#[serde(default)]
	pub parser: Vec<Parser>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
	pub uri: String,

	#[serde(default)]
	pub params: Vec<Param>,

	#[serde(default)]
	pub body: Option<String>,

	#[serde(default = "default_method")]
	pub method: Method,

	#[serde(default)]
	pub auth: Option<Authentication>,

	#[serde(default)]
	pub header: Vec<Param>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Method {
	GET,
	POST,
	HEAD,
	OPTION,
}
impl fmt::Display for Method {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	None,
	Basic { user: String, pass: String },
	Bearer(String),
	Header(Param),
}
impl fmt::Display for Authentication {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Authentication::None => write!(f, "None"),
			Authentication::Basic { user: u, pass: _ } => write!(f, "Basic '{}: ****'", u),
			Authentication::Bearer(_) => write!(f, "Bearer ****"),
			Authentication::Header(param) => write!(f, "Header '{}: ****'", param.name),
		}
	}
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Param {
	pub name: String,
	pub value: String,
}
impl fmt::Display for Param {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "[{:?}]='{:?}'", self.name, self.value)
	}
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_method() -> Method { Method::GET }
fn default_cron_timer() -> String { String::from("*/5 * * * *") }
