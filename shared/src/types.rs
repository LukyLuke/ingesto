use std::{fmt, collections::HashMap};

use chrono::{DateTime, Utc};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "runtime")]
use {
	std::net::Ipv4Addr,
	ipnetwork::Ipv4Network,
};

#[cfg(feature = "types")]
pub type ConfStruct = HashMap<String, ConfType>;

#[cfg(feature = "types")]
#[derive(Debug, Serialize)]
pub enum ConfType {
	Bool,
	UInt, Int, Float,
	String, RegEx,
	Enum(ConfStruct),
	EnumValue,
	EnumParams(&'static str, &'static str),
	Struct(ConfStruct),
	Option(Box<ConfType>),
	Vec(Box<ConfType>),
}
impl fmt::Display for ConfType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

/// Message-Queue configuration
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Queue {
	// Max number of message in the queue before the processor (parser) is reading out and clearing the queue
	#[serde(default = "default_messages")]
	pub max_messages: u16,

	// Maximum time in seconds between processing messages from the queue
	#[serde(default = "default_time")]
	pub max_seconds: u16,

	// Maximum length of the final message (cummulated json strings as an array)
	#[serde(default = "default_size")]
	pub max_size: usize,

	// Collect messages and sent out a list of logs (true) or send each one separate (false)
	#[serde(default = "default_collect")]
	pub collect_messages: bool,

	// Where to send the log messages to
	#[serde(default)]
	pub otel_logger: Option<OtelLogger>,
}
fn default_messages() -> u16 { 1024 }
fn default_size() -> usize { 65535 }
fn default_time() -> u16 { 60 }
fn default_collect() -> bool { false }

impl Default for Queue {
	fn default() -> Self {
		Self {
			max_messages: default_messages(),
			max_seconds: default_time(),
			max_size: default_size(),
			collect_messages: default_collect(),
			otel_logger: None,
		}
	}
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Queue {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("max_messages".to_string(), ConfType::UInt),
			("max_seconds".to_string(), ConfType::UInt),
			("max_size".to_string(), ConfType::UInt),
			("collect_messages".to_string(), ConfType::Bool),
			("otel_logger".to_string(), ConfType::Option( Box::new(ConfType::Struct(OtelLogger::default().into())) )),
		])
	}
}

/// Defines a parser which is used for
/// * Parsing the main message
/// * Parsing a field value which references the parser by it's name
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Parser {
	// Name of the parser for referencing in a field mapping
	#[serde(default)]
	pub name: String,

	// Simple Regular Expression to match on the message
	// The Matcher is just to select the parser, not to parse the fields
	#[serde(default)]
	pub matcher: String,

	// How to parse the message
	#[serde(default = "default_parser_kind")]
	pub kind: ParserKind,

	// Settings for the different parsers
	#[serde(default = "default_parser_setting")]
	pub settings: ParserSettings,

	// Field-Mapping from the source to the resulting structured mesage
	// A FieldMapper can reference to a Parser
	#[serde(default)]
	pub mapping: Vec<FieldMapping>,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Parser {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("matcher".to_string(), ConfType::RegEx),
			("kind".to_string(), ConfType::Enum( ParserKind::RAW.into() )),
			("setting".to_string(), ConfType::Enum( ParserSettings::Nothing.into() )),
			("mapping".to_string(), ConfType::Vec( Box::new(ConfType::Struct(FieldMapping::default().into())) )),
		])
	}
}

/// Defines how the message should be parsed
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum ParserKind {
	// Takes the message as-is
	#[default]
	RAW,

	// Applies a regular expression to extract values
	REGEX,

	// Parses the message as JSON and applies a possible JsonPath to extract just a part of the object
	JSON,

	// Simple CSV-Parser
	CSV,

	// CEF and LEEF are quite similar SyslogMessages
	CEF,
	LEEF,

	// Structured Syslog Messages are similar to CEF/LEEF but have a different Key-Value pair format
	STRUCTURED,
}
fn default_parser_kind() -> ParserKind { ParserKind::default() }

#[cfg(feature = "types")]
impl Into<ConfStruct> for ParserKind {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("RAW".to_string(), ConfType::EnumValue),
			("REGEX".to_string(), ConfType::EnumValue),
			("JSON".to_string(), ConfType::EnumValue),
			("CSV".to_string(), ConfType::EnumValue),
			("CEF".to_string(), ConfType::EnumValue),
			("FEEL".to_string(), ConfType::EnumValue),
			("STRUCTURED".to_string(), ConfType::EnumValue),
		])
	}
}

impl fmt::Display for ParserKind {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

/// Based on the parser, either a string which represents a RegularExpression or a JsonPath
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum ParserSettings {
	// No Setting
	#[default]
	Nothing,

	// Regular Expression to extract all values from the whole message
	// Use Idexed Groups `(\w+)` or Named Groups `(?<Name>\w+)`
	Regex(String),

	// JsonPath to extract the main message
	// Use `$` for the whole message or `$.foo.bar` for a sub-message struct
	Jpath(String),

	// Defines if the first line is a header or not
	// If it is a header, the `source` can be used in the matcher for the column name, otherwise the `index` defines the column number
	Csv(bool),
}
fn default_parser_setting() -> ParserSettings { ParserSettings::default() }

#[cfg(feature = "types")]
impl Into<ConfStruct> for ParserSettings {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("Nothing".to_string(), ConfType::EnumValue),
			("Regex".to_string(), ConfType::String),
			("Jpath".to_string(), ConfType::String),
			("Csv".to_string(), ConfType::Bool),
		])
	}
}

impl fmt::Display for ParserSettings {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

/// Represents a universal mapping of a field from the source message in the final message
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct FieldMapping {
	/// Name of the field in the final struct
	pub name: String,

	/// The name of the field from the source message
	/// For Regex: The GroupName defined by `(?<GroupName>...)` in the regex
	/// For Json: The FieldName directly on the struct; A JsonPointer value line `/foo/bar/0/fieldname` to extract a value
	#[serde(default)]
	pub source: String,

	/// The index of the group from a regex match - better use CaptureGroup Names
	#[serde(default)]
	pub index: usize,

	/// Name of a parser to apply to the extracted value
	#[serde(default)]
	pub parser: String,

	/// Shall an empty value be added to the final struct or not
	#[serde(default)]
	pub empty: bool,

	/// Static field value
	// This can be a templated value like {{ $date() }} or {{ $uuid }}
	// The {{ $response/field/value }} is not supported
	#[serde(default, rename="static")]
	pub static_value: String,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for FieldMapping {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("source".to_string(), ConfType::String),
			("index".to_string(), ConfType::UInt),
			("parser".to_string(), ConfType::Option( Box::new(ConfType::String) )),
			("empty".to_string(), ConfType::Bool),
			("static_value".to_string(), ConfType::String),
		])
	}
}

/// Represents an OpenTelemetry Endpoint, where Metrics and/or Logs can be sent to
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtelLogger {
	pub endpoint: String,

	#[serde(default = "default_otel_port")]
	pub port: u16,

	#[serde(default = "default_otel_service")]
	pub service: String,
}
fn default_otel_service() -> String { String::from("ingesto") }
fn default_otel_port() -> u16 { 4318 }

impl Default for OtelLogger {
	fn default() -> Self {
		Self {
			endpoint: String::from("0.0.0.0"),
			port: default_otel_port(),
			service: default_otel_service(),
		}
	}
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for OtelLogger {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("endpoint".to_string(), ConfType::String),
			("port".to_string(), ConfType::UInt),
			("service".to_string(), ConfType::String),
		])
	}
}

impl OtelLogger {
	pub fn get_endpoint(&self, path: &str) -> String {
		let mut p = path.to_owned();
		if let Some(s) = p.get(0..1) && s !=  "/" {
			p.insert_str(0, "/");
		};
		if self.endpoint.starts_with("http") {
			return format!("{}:{}{}", self.endpoint, self.port, p);
		}
		format!("http://{}:{}{}", self.endpoint, self.port, p)
	}
}

/// Represents an OpenTelemetry Endpoint, where Metrics and/or Logs can be received
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtelReceiver {
	pub address: String,

	#[serde(default = "default_otel_port")]
	pub port: u16,

	#[serde(default = "default_logs_path")]
	pub path: String,
}
fn default_logs_path() -> String { String::from("/v1/logs") }

impl Default for OtelReceiver {
	fn default() -> Self {
		Self {
			address: String::from("0.0.0.0"),
			port: default_otel_port(),
			path: default_logs_path(),
		}
	}
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for OtelReceiver {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("address".to_string(), ConfType::String),
			("port".to_string(), ConfType::UInt),
			("path".to_string(), ConfType::String),
		])
	}
}

impl OtelReceiver {
	pub fn get_address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}
}

/// Specific internal type for adding and reading values from a database
#[cfg(feature = "types")]
#[derive(Debug, Clone, PartialEq)]
pub enum DbValue {
	Bool(bool),
	I64(i64),
	F64(f64),
	String(String),
	Bytes(Vec<u8>),
	DateTimeUtc(DateTime<Utc>),
	IpAddress(IpNetwork),
	Json(Value),
}

#[cfg(feature = "runtime")]
impl DbValue {
	pub fn from(fields: &Vec<DbField>, json: &serde_json::Value) -> Vec<(String, DbValue)> {
		fields.iter().map(|field| Self::convert(field, &json)).collect()
	}

	fn convert(val: &DbField, json: &serde_json::Value) -> (String, DbValue) {
		match val {
			DbField::String { name, origin, default } => (
				String::from(name),
				Self::String(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Null, |s| Value::String(s.to_string()) )
						).as_str()
						.unwrap_or(
							&default.as_ref().map_or_else(|| "", |s| s )
						).to_string()
				)
			),
			DbField::Float { name, origin, default } => (
				String::from(name),
				Self::F64(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Null, |s| Value::String(s.to_string()) )
						).as_f64()
						.unwrap_or(
							default.as_ref().map_or_else(|| 0.0, |s| s.parse().unwrap_or_default() )
						)
				)
			),
			DbField::Bool { name, origin, default } => (
				String::from(name),
				Self::Bool(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Bool(false), |s| serde_json::from_str(s).unwrap_or_default() )
						).as_bool()
						.unwrap_or_default()
				)
			),
			DbField::Int { name, origin, default } => (
				String::from(name),
				Self::I64(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Null, |s| Value::String(s.to_string()) )
						).as_i64()
						.unwrap_or(
							default.as_ref().map_or_else(|| 0, |s| s.parse().unwrap_or_default() )
						)
				)
			),
			DbField::DateTimeUtc { name, origin, default } => (
				String::from(name),
				Self::DateTimeUtc( {
					let def = default.as_ref().map_or_else(|| Value::Null, |s| Value::String(s.to_string()) );
					let ds = json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(&def).as_str()
						.unwrap_or_default();
					match dateparser::parse(ds) {
						Ok(dt) => dt.to_utc(),
						Err(_) => chrono::DateTime::<Utc>::MIN_UTC
					}
				} )
			),
			DbField::IpAddress { name, origin, default } => (
				String::from(name),
				Self::IpAddress(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Null, |s| Value::String(s.to_string()) )
						).as_str()
						.unwrap_or_default().parse()
						.unwrap_or(
							IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 1), 32).unwrap())
						)
				)
			),
			DbField::Bytes { name, origin, default } => (
				String::from(name),
				Self::Bytes(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Null, |s| Value::String(s.to_string()) )
						).as_str()
						.unwrap_or_default().as_bytes()
						.to_vec()
				)
			),
			DbField::Json { name, origin, default } => (
				String::from(name),
				Self::Json(
					json.get(origin.as_ref().unwrap_or(name))
						.unwrap_or(
							&default.as_ref().map_or_else(|| Value::Null, |s| serde_json::from_str(s).unwrap_or_default() )
						)
						.clone()
				)
			),
		}
	}
}

/// Defines a field in the Database from a given type, fieldname and message-field name
/// * `name` - Field name in the Database
/// * `origin` - Optional: Field name from the log message; If not defined, the same as `name` is used
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum DbField {
	Bool { name: String, origin: Option<String>, default: Option<String> },
	Int { name: String, origin: Option<String>, default: Option<String> },
	Float { name: String, origin: Option<String>, default: Option<String> },
	String { name: String, origin: Option<String>, default: Option<String> },
	Bytes { name: String, origin: Option<String>, default: Option<String> },
	DateTimeUtc { name: String, origin: Option<String>, default: Option<String> },
	IpAddress { name: String, origin: Option<String>, default: Option<String> },
	Json { name: String, origin: Option<String>, default: Option<String> },
}

impl Default for DbField {
	fn default() -> Self {
		Self::String { name: String::new(), origin: None, default: None }
	}
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for DbField {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("Bool".to_string(), ConfType::EnumParams("name", "origin")),
			("Int".to_string(), ConfType::EnumParams("name", "origin")),
			("Float".to_string(), ConfType::EnumParams("name", "origin")),
			("String".to_string(), ConfType::EnumParams("name", "origin")),
			("bytes".to_string(), ConfType::EnumParams("name", "origin")),
			("DateTimeUtc".to_string(), ConfType::EnumParams("name", "origin")),
			("IpAddress".to_string(), ConfType::EnumParams("name", "origin")),
			("Json".to_string(), ConfType::EnumParams("name", "origin")),
		])
	}
}


#[cfg(test)]
mod test {
	use super::*;

	use std::net::Ipv6Addr;
	use ipnetwork::Ipv6Network;
	use serde_json::json;

	#[test]
	fn test_otellogger() {
		let logger = OtelLogger {
			endpoint: "endpoint".to_string(),
			port: 6543,
			service: "service".to_string(),
		};
		assert_eq!(logger.get_endpoint("/path/value"), "http://endpoint:6543/path/value");

		let http = OtelLogger {
			endpoint: "http://endpoint".to_string(),
			port: 6543,
			service: "service".to_string(),
		};
		assert_eq!(http.get_endpoint("/path/value"), "http://endpoint:6543/path/value");
	}

	#[test]
	fn test_otelreceiver() {
		let logger = OtelReceiver {
			address: "endpoint".to_string(),
			port: 6543,
			path: "/path/value".to_string(),
		};
		assert_eq!(logger.get_address(), "endpoint:6543");
	}


	// DbValue test Values
	fn json() -> Value {
		let value: Value = json!({
			"string": "unknown", // for invalid check all except String-values
			"number": 123.45,    // for invalid check string values

			"bool_t": true,
			"bool_f": false,
			"int_1": 666,
			"float_1": 666.66,
			"str": "string value",
			"json": { "json":"value" },
			"bytes": "bytes value",
			"datetime": "2020-01-02T13:14:15.1234Z",
			"date": "2020-01-02",
			"time": "13:14:15",
			"ipv4": "10.11.12.13",
			"ipv6": "fe80::bad:dead:beef",
		});
		value
	}

	#[test]
	fn test_dbvalue_bool() {
		let value = json();

		// valid values
		{
			let from = &DbField::Bool { name: "name".to_string(), origin: Some("bool_t".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bool(true));
		}
		{
			let from = &DbField::Bool { name: "name".to_string(), origin: Some("bool_f".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bool(false));
		}

		// invalid values
		{
			let from = &DbField::Bool { name: "name".to_string(), origin: Some("string".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bool(false));
		}

		// no values
		{
			let from = &DbField::Bool { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bool(false));
		}

		// no values with default
		{
			let from = &DbField::Bool { name: "name".to_string(), origin: Some("none".to_string()), default: Some("true".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bool(true));
		}
	}

	#[test]
	fn test_dbvalue_numbers() {
		let value = json();

		// valid values
		{
			let from = &DbField::Int { name: "name".to_string(), origin: Some("int_1".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::I64(666) );
		}
		{
			let from = &DbField::Float { name: "name".to_string(), origin: Some("float_1".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::F64(666.66));
		}

		// invalid values
		{
			let from = &DbField::Int { name: "name".to_string(), origin: Some("string".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::I64(0));
		}
		{
			let from = &DbField::Float { name: "name".to_string(), origin: Some("string".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::F64(0.0));
		}

		// no values
		{
			let from = &DbField::Int { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::I64(0));
		}
		{
			let from = &DbField::Float { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::F64(0.0));
		}

		// no values with default
		{
			let from = &DbField::Int { name: "name".to_string(), origin: Some("none".to_string()), default: Some("666".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::I64(666));
		}
		{
			let from = &DbField::Float { name: "name".to_string(), origin: Some("none".to_string()), default: Some("666.66".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::F64(666.66));
		}
	}

	#[test]
	fn test_dbvalue_strings() {
		let value = json();

		// valid values
		{
			let from = &DbField::String { name: "name".to_string(), origin: Some("str".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::String("string value".to_string()) );
		}
		{
			let from = &DbField::Json { name: "name".to_string(), origin: Some("json".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Json(json!({ "json":"value" })));
		}
		{
			let from = &DbField::Bytes { name: "name".to_string(), origin: Some("bytes".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bytes("bytes value".as_bytes().into()));
		}

		// invalid values
		{
			let from = &DbField::String { name: "name".to_string(), origin: Some("number".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::String("".to_string()) );
		}
		{
			// There is no "invalid" json possible in a json::Value value
		}
		{
			let from = &DbField::Bytes { name: "name".to_string(), origin: Some("number".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bytes("".as_bytes().into()));
		}

		// no values
		{
			let from = &DbField::String { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::String("".to_string()) );
		}
		{
			let from = &DbField::Json { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Json(Value::Null));
		}
		{
			let from = &DbField::Bytes { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bytes("".as_bytes().into()));
		}

		// no values with default
		{
			let from = &DbField::String { name: "name".to_string(), origin: Some("none".to_string()), default: Some("default".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::String("default".to_string()) );
		}
		{
			let from = &DbField::Json { name: "name".to_string(), origin: Some("none".to_string()), default: Some("{\"json\":\"value\"}".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Json(json!({ "json":"value" })));
		}
		{
			let from = &DbField::Bytes { name: "name".to_string(), origin: Some("none".to_string()), default: Some("default".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::Bytes("default".as_bytes().into()));
		}
	}

	#[test]
	fn test_dbvalue_dates() {
		let value = json();

		// valid values
		{
			let from = &DbField::DateTimeUtc { name: "name".to_string(), origin: Some("datetime".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::DateTimeUtc(dateparser::parse("2020-01-02T13:14:15.1234Z").unwrap().to_utc()) );
		}

		// invalid values
		{
			let from = &DbField::DateTimeUtc { name: "name".to_string(), origin: Some("string".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::DateTimeUtc(chrono::DateTime::<Utc>::MIN_UTC) );
		}

		// no values
		{
			let from = &DbField::DateTimeUtc { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::DateTimeUtc(chrono::DateTime::<Utc>::MIN_UTC) );
		}

		// no values with default
		{
			let from = &DbField::DateTimeUtc { name: "name".to_string(), origin: Some("none".to_string()), default: Some("2020-01-02T13:14:15.1234Z".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::DateTimeUtc(dateparser::parse("2020-01-02T13:14:15.1234Z").unwrap().to_utc()) );
		}
	}

	#[test]
	fn test_dbvalue_ipaddress() {
		let value = json();
		let ipv4 = IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(10, 11, 12, 13), 32).unwrap());
		let ipv6 = IpNetwork::V6(Ipv6Network::new(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0xbad, 0xdead, 0xbeef), 128).unwrap());
		let localhost = IpNetwork::V4(Ipv4Network::new(Ipv4Addr::new(127, 0, 0, 1), 32).unwrap());

		// valid values
		{
			let from = &DbField::IpAddress { name: "name".to_string(), origin: Some("ipv4".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::IpAddress(ipv4) );
		}
		{
			let from = &DbField::IpAddress { name: "name".to_string(), origin: Some("ipv6".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::IpAddress(ipv6) );
		}

		// invalid values
		{
			let from = &DbField::IpAddress { name: "name".to_string(), origin: Some("string".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::IpAddress(localhost) );
		}

		// no values
		{
			let from = &DbField::IpAddress { name: "name".to_string(), origin: Some("none".to_string()), default: None };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::IpAddress(localhost) );
		}

		// no values wuth default
		{
			let from = &DbField::IpAddress { name: "name".to_string(), origin: Some("none".to_string()), default: Some("10.11.12.13".to_string()) };
			let (field_name, db_value) = DbValue::convert(from, &value);
			assert_eq!(field_name, "name");
			assert_eq!(db_value, DbValue::IpAddress(ipv4) );
		}
	}

}
