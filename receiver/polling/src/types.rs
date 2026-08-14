use serde::{Deserialize, Serialize};
use shared::types;

// Default-Wrapper Functions for Serde::Deserialize
fn default_method() -> Method { Method::GET }
fn default_cron_timer() -> String { String::from("* */5 * * * *") }

/// The main Polling-Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Polling,
}

/// A Polling Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Polling {
	/// Name of the instance
	pub name: String,

	/// Where to send the requests to
	pub api: Vec<Endpoint>,

	/// Timeout between the requests
	#[serde(default = "default_cron_timer")]
	pub timer: String,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: types::Queue,

	/// Message-Parser Configuration
	#[serde(default)]
	pub parser: Vec<types::Parser>,
}

/// An Endpoint where and how to send a request to
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
	/// FQDN where to send a request to.
	/// Can contain Template-Parameters: {{ $uuid }}, {{ $date([$response/json/pointer/value]#FORMAT) }}, {{ $response/json/pointer/value }}
	pub uri: String,

	/// In case of a POST, the Body to send.
	/// Can contain Template-Parameters
	#[serde(default)]
	pub body: Option<String>,

	/// Method to use to send a request
	/// Can be GET, POST, HEAD, OPTION
	#[serde(default = "default_method")]
	pub method: Method,

	/// Authentication Configuration
	#[serde(default)]
	pub auth: Option<Authentication>,

	/// Custom Header Pairs
	#[serde(default)]
	pub header: Vec<Param>,

	/// Paging-Request Configuration
	#[serde(default)]
	pub paging: Option<PagingRequest>,
}

/// Request-Methods
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Method {
	GET,
	POST,
	HEAD,
	OPTION,
}

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

	/// Header Authentication with a key and a value.
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Header(Param),
}

/// A simple Key-Value pair used for different representations
/// The Value can be a Template-Param in most constructs: {{ ... }}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Param {
	pub name: String,
	pub value: String,
}

/// Paging Requests can be used if an Endpoint sends a lot of data which are split over multiple requests and responses
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PagingRequest {
	/// Name and Value for the parameter which is added on the Endpoints URI
	/// The Value can/should be a Template-Value which normally contains a value from the response, like: `{{ $response/paging/cursor }}`
	pub param: Option<Param>,

	/// Defines how to check if there is no more pages
	pub until: Option<PagingRequestUntil>,

	/// Timeout between paging requests in milliseconds
	#[serde(default)]
	pub timeout: u32,

	/// Maximum number of paging requests
	/// Exit-Strategy to avoid too many requests
	#[serde(default)]
	pub max_pages: u16,
}


/// Defines the paging
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum PagingRequestUntil {
	/// No Paging
	None,

	/// An empty response
	Empty,

	/// A defined status code
	StatusCode(u16),

	/// An empty value inside the json-response
	EmptyValue(String),

	/// Two values from inside the json-response or static strings have to match
	Equals(String, String),
}
