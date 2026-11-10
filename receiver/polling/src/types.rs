use serde::{Deserialize, Serialize};
use shared::types;
#[cfg(feature = "types")]
use shared::types::{ConfStruct, ConfType, leak_str};

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
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		let mut map = ConfStruct::new();
		map.insert("config".to_string(), ConfType::Struct(val.config.into()));
		map
	}
}

/// A Polling Configuration
#[cfg(feature = "types")]
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
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub parser: Vec<types::Parser>,
}

#[cfg(feature = "types")]
impl Default for Polling {
	fn default() -> Self {
		Self {
			name: String::new(),
			api: Vec::new(),
			timer: default_cron_timer(),
			queue: types::Queue::default(),
			parser: Vec::new(),
		}
	}
}

#[cfg(feature = "types")]
impl From<Polling> for ConfStruct {
	fn from(val: Polling) -> Self {
		let mut map = ConfStruct::new();
		map.insert("name".to_string(), ConfType::String(leak_str(&val.name)));
		map.insert("api".to_string(), ConfType::Vec( Box::new( ConfType::Struct( Endpoint::default().into() ) ) ));
		map.insert("timer".to_string(), ConfType::Option( Box::new( ConfType::String(leak_str(&val.timer)) ) ));
		map.insert("queue".to_string(), ConfType::Option( Box::new( ConfType::Struct( val.queue.into() ) ) ));
		map.insert("parser".to_string(), ConfType::Option( Box::new( ConfType::Vec( Box::new( ConfType::Struct( types::Parser::default().into() ) ) ) ) ));
		map
	}
}

/// An Endpoint where and how to send a request to
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
	/// Name of the API, to know what this endpoint is for. It is used in logs.
	pub name: String,

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
impl From<Endpoint> for ConfStruct {
	fn from(val: Endpoint) -> Self {
		let auth = val.auth.unwrap_or_default();
		let mut map = ConfStruct::new();
		map.insert("name".to_string(), ConfType::String(leak_str(&val.name)));
		map.insert("uri".to_string(), ConfType::String(leak_str(&val.uri)));
		map.insert("body".to_string(), ConfType::Option( Box::new(ConfType::String(leak_str(val.body.as_deref().unwrap_or_default()))) ));
		map.insert("method".to_string(), ConfType::Option( Box::new(ConfType::Enum(val.method.clone().into(), val.method.name())) ));
		map.insert("auth".to_string(), ConfType::Option( Box::new(ConfType::Enum(auth.clone().into(), auth.name())) ));
		map.insert("header".to_string(), ConfType::Option( Box::new(ConfType::Vec( Box::new(ConfType::Struct( Param::default().into() )) )) ));
		map.insert("paging".to_string(), ConfType::Option( Box::new(ConfType::Struct( val.paging.unwrap_or_default().into() )) ));
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
impl Method {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::GET => "GET",
			Self::POST => "POST",
			Self::HEAD => "HEAD",
			Self::OPTION => "OPTION",
		}
	}
}

#[cfg(feature = "types")]
impl From<Method> for ConfStruct {
	fn from(_val: Method) -> Self {
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
			("Basic".to_string(), ConfType::Struct(ConfStruct::from([
				("user".to_string(), ConfType::String("")),
				("pass".to_string(), ConfType::String("")),
			]))),
			("Bearer".to_string(), ConfType::String("")),
			("Header".to_string(), ConfType::Struct(ConfStruct::from([
				("header".to_string(), ConfType::String("")),
				("value".to_string(), ConfType::String("")),
			]))),
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
impl From<Param> for ConfStruct {
	fn from(val: Param) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak_str(&val.name))),
			("value".to_string(), ConfType::String(leak_str(&val.value))),
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
impl From<PagingRequest> for ConfStruct {
	fn from(val: PagingRequest) -> Self {
		ConfStruct::from([
			("param".to_string(), ConfType::Option( Box::new(ConfType::Struct( val.param.unwrap_or_default().into() )) )),
			("until".to_string(), ConfType::Option( Box::new(ConfType::Enum(val.until.clone().into(), val.until.name())) )),
			("timeout".to_string(), ConfType::Option( Box::new(ConfType::UInt(val.timeout.into())) )),
			("max_pages".to_string(), ConfType::Option( Box::new(ConfType::UInt(val.max_pages.into())) )),
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
impl PagingRequestUntil {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::None => "None",
			Self::Empty => "Empty",
			Self::StatusCode(_) => "StatusCode",
			Self::EmptyValue(_) => "EmptyValue",
			Self::Equals(_, _) => "Equals",
		}
	}
}

#[cfg(feature = "types")]
impl From<PagingRequestUntil> for ConfStruct {
	fn from(_val: PagingRequestUntil) -> Self {
		ConfStruct::from([
			("None".to_string(), ConfType::EnumValue),
			("Empty".to_string(), ConfType::EnumValue),
			("StatusCode".to_string(), ConfType::UInt(0)),
			("EmptyValue".to_string(), ConfType::String("")),
			("Equals".to_string(), ConfType::Tuple(vec![ ConfType::String(""), ConfType::String("") ])),
		])
	}
}

#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	#[test]
	fn endpoint_name_is_read_from_the_config() {
		let endpoint: Endpoint = serde_json::from_str(r#"{ "name": "SentinelOne Agents", "uri": "http://x" }"#).unwrap_or_default();
		assert_eq!(endpoint.name, "SentinelOne Agents");
		assert_eq!(endpoint.uri, "http://x");
	}

	#[test]
	fn endpoint_name_is_required() {
		assert!(serde_json::from_str::<Endpoint>(r#"{ "uri": "http://x" }"#).is_err());
		assert!(serde_json::from_str::<Endpoint>(r#"{ "name": "x" }"#).is_err());
	}

	#[test]
	fn endpoint_name_may_be_empty_or_special() {
		for name in ["", "Ünï \"quoted\" # 日本"] {
			let json = serde_json::json!({ "name": name, "uri": "http://x" });
			let endpoint: Endpoint = serde_json::from_value(json).unwrap_or_default();
			assert_eq!(endpoint.name, name);
		}
	}

	#[test]
	fn endpoint_schema_has_a_required_name_first() {
		let schema: ConfStruct = Endpoint::default().into();
		assert_eq!(schema.iter().next().map(|(name, ty)| (name.as_str(), matches!(ty, ConfType::String(_)))), Some(("name", true)));
	}

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
	fn endpoint_schema_uses_values() {
		let conf: ConfStruct = Endpoint {
			name: "agents".to_string(),
			uri: "http://example.org".to_string(),
			body: Some("{}".to_string()),
			method: Method::POST,
			auth: Some(Authentication::Bearer("token".to_string())),
			header: Vec::new(),
			paging: Some(PagingRequest { timeout: 250, max_pages: 7, ..PagingRequest::default() }),
		}.into();

		assert_eq!(string_of(&conf, "name"), Some("agents"));
		assert_eq!(string_of(&conf, "uri"), Some("http://example.org"));
		assert_eq!(string_of(&conf, "body"), Some("{}"));
		assert!(matches!(conf.get("method"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "POST"))));
		assert!(matches!(conf.get("auth"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "Bearer"))));

		let Some(ConfType::Option(paging)) = conf.get("paging") else { panic!("missing paging") };
		let ConfType::Struct(paging) = paging.as_ref() else { panic!("paging is no struct") };
		assert_eq!(paging.get("timeout"), Some(&ConfType::Option(Box::new(ConfType::UInt(250)))));
		assert_eq!(paging.get("max_pages"), Some(&ConfType::Option(Box::new(ConfType::UInt(7)))));
	}

	#[test]
	fn endpoint_schema_without_optionals() {
		let conf: ConfStruct = Endpoint::default().into();
		assert_eq!(string_of(&conf, "body"), Some(""));
		assert!(matches!(conf.get("method"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "GET"))));
		assert!(matches!(conf.get("auth"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "None"))));
	}

	#[test]
	fn polling_uses_values_and_default_timer() {
		let conf: ConfStruct = Polling { name: "poller".to_string(), timer: "0 * * * * *".to_string(), ..Polling::default() }.into();
		assert_eq!(string_of(&conf, "name"), Some("poller"));
		assert_eq!(string_of(&conf, "timer"), Some("0 * * * * *"));

		let conf: ConfStruct = Polling::default().into();
		assert_eq!(string_of(&conf, "timer"), Some("* */5 * * * *"));
	}

	#[test]
	fn paging_until_names() {
		assert_eq!(PagingRequestUntil::None.name(), "None");
		assert_eq!(PagingRequestUntil::StatusCode(200).name(), "StatusCode");
		assert_eq!(PagingRequestUntil::Equals("a".to_string(), "b".to_string()).name(), "Equals");

		let conf: ConfStruct = PagingRequest { until: PagingRequestUntil::Empty, ..PagingRequest::default() }.into();
		assert!(matches!(conf.get("until"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "Empty"))));
	}

	#[test]
	fn param_uses_values() {
		let conf: ConfStruct = Param { name: "cursor".to_string(), value: "{{ $response/paging/cursor }}".to_string() }.into();
		assert_eq!(string_of(&conf, "name"), Some("cursor"));
		assert_eq!(string_of(&conf, "value"), Some("{{ $response/paging/cursor }}"));
	}

	#[test]
	fn authentication_does_not_expose_secrets() {
		let auth = Authentication::Header { header: "X-Key".to_string(), value: "secret".to_string() };
		assert_eq!(auth.name(), "Header");

		let conf: ConfStruct = auth.into();
		let Some(ConfType::Struct(header)) = conf.get("Header") else { panic!("missing Header variant") };
		assert_eq!(string_of(header, "value"), Some(""));
	}
}
