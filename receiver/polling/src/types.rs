use serde::{Deserialize, Serialize};
use shared::types;
#[cfg(feature = "types")]
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
fn default_method() -> Method { Method::GET }
fn default_cron_timer() -> String { String::from("* */5 * * * *") }

/// The main Polling-Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Polling,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Config {
	fn into(self) -> ConfStruct {
		let mut map = ConfStruct::new();
		map.insert("config".to_string(), ConfType::Struct(Polling::default().into()));
		map
	}
}

/// A Polling Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
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
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub parser: Vec<types::Parser>,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Polling {
	fn into(self) -> ConfStruct {
		let mut map = ConfStruct::new();
		map.insert("name".to_string(), ConfType::String);
		map.insert("api".to_string(), ConfType::Vec( Box::new( ConfType::Struct( Endpoint::default().into() ) ) ));
		map.insert("timer".to_string(), ConfType::String);
		map.insert("queue".to_string(), ConfType::Struct( types::Queue::default().into() ));
		map.insert("parser".to_string(), ConfType::Vec( Box::new( ConfType::Struct( types::Parser::default().into() ) ) ));
		map
	}
}

/// An Endpoint where and how to send a request to
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
	/// FQDN where to send a request to.
	/// Can contain Template-Parameters: {{ $uuid }}, {{ $date([$response/json/pointer/value]#FORMAT) }}, {{ $response/json/pointer/value }}
	pub uri: String,

	/// In case of a POST, the Body to send.
	/// Can contain Template-Parameters
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub body: Option<String>,

	/// Method to use to send a request
	/// Can be GET, POST, HEAD, OPTION
	#[serde(default = "default_method")]
	pub method: Method,

	/// Authentication Configuration
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub auth: Option<Authentication>,

	/// Custom Header Pairs
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub header: Vec<Param>,

	/// Paging-Request Configuration
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub paging: Option<PagingRequest>,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Endpoint {
	fn into(self) -> ConfStruct {
		let mut map = ConfStruct::new();
		map.insert("uri".to_string(), ConfType::String);
		map.insert("body".to_string(), ConfType::Option( Box::new(ConfType::String) ));
		map.insert("method".to_string(), ConfType::Enum(Method::GET.into()));
		map.insert("auth".to_string(), ConfType::Enum(Authentication::None.into()));
		map.insert("header".to_string(), ConfType::Vec( Box::new(ConfType::Struct( Param::default().into() )) ));
		map.insert("paging".to_string(), ConfType::Option( Box::new(ConfType::Struct( PagingRequest::default().into() )) ));
		map
	}
}

/// Request-Methods
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum Method {
	#[default]
	GET,
	POST,
	HEAD,
	OPTION,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Method {
	fn into(self) -> ConfStruct {
		let mut map = ConfStruct::new();
		map.insert("GET".to_string(), ConfType::EnumValue);
		map.insert("POST".to_string(), ConfType::EnumValue);
		map.insert("HEAD".to_string(), ConfType::EnumValue);
		map.insert("OPTION".to_string(), ConfType::EnumValue);
		map
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

	/// Header Authentication with a key and a value.
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Header { header: String, value: String },
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

/// A simple Key-Value pair used for different representations
/// The Value can be a Template-Param in most constructs: {{ ... }}
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Param {
	pub name: String,
	pub value: String,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Param {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("string".to_string(), ConfType::String),
			("value".to_string(), ConfType::String),
		])
	}
}

/// Paging Requests can be used if an Endpoint sends a lot of data which are split over multiple requests and responses
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct PagingRequest {
	/// Name and Value for the parameter which is added on the Endpoints URI
	/// The Value can/should be a Template-Value which normally contains a value from the response, like: `{{ $response/paging/cursor }}`
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub param: Option<Param>,

	/// Defines how to check if there is no more pages
	#[serde(default)]
	pub until: PagingRequestUntil,

	/// Timeout between paging requests in milliseconds
	#[serde(default)]
	pub timeout: u32,

	/// Maximum number of paging requests
	/// Exit-Strategy to avoid too many requests
	#[serde(default)]
	pub max_pages: u16,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for PagingRequest {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("param".to_string(), ConfType::Option( Box::new(ConfType::Struct( Param::default().into() )) )),
			("until".to_string(), ConfType::Enum(PagingRequestUntil::None.into())),
			("timeout".to_string(), ConfType::UInt),
			("max_pages".to_string(), ConfType::UInt),
		])
	}
}

/// Defines the paging
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize, PartialEq)]
pub enum PagingRequestUntil {
	/// No Paging
	#[default]
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

#[cfg(feature = "types")]
impl Into<ConfStruct> for PagingRequestUntil {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("None".to_string(), ConfType::EnumValue),
			("Empty".to_string(), ConfType::EnumValue),
			("StatusCode".to_string(), ConfType::UInt),
			("EmptyValue".to_string(), ConfType::String),
			("Equals".to_string(), ConfType::EnumParams("left", "right")),
		])
	}
}
