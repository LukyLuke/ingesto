use core::fmt;
use serde::{Deserialize, Serialize};

/// Message-Queue configuration
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

/// Defines a parser which is used for
/// * Parsing the main message
/// * Parsing a field value which references the parser by it's name
#[derive(Debug, Clone, Deserialize, Serialize)]
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

/// Defines how the message should be parsed
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ParserKind {
	// Takes the message as-is
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
fn default_parser_kind() ->ParserKind { ParserKind::RAW }
impl fmt::Display for ParserKind {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}
impl Default for Parser {
	fn default() -> Self {
		Self {
			name: String::new(),
			matcher: String::new(),
			kind: default_parser_kind(),
			settings: default_parser_setting(),
			mapping: vec![]
		}
	}
}

/// Based on the parser, either a string which represents a RegularExpression or a JsonPath
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ParserSettings {
	// No Setting
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
fn default_parser_setting() -> ParserSettings { ParserSettings::Nothing }
impl fmt::Display for ParserSettings {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

/// Represents a universal mapping of a field from the source message in the final message
#[derive(Debug, Clone, Deserialize, Serialize)]
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
}

/// Represents an OpenTelemetry Endpoint, where Metrics and/or Logs can be sent to
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
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OtelReceiver {
	pub address: String,
	pub port: u16,

	#[serde(default = "default_logs_path")]
	pub path: String,
}
fn default_logs_path() -> String { String::from("/v1/logs") }
impl OtelReceiver {
	pub fn get_address(&self) -> String {
		format!("{}:{}", self.address, self.port)
	}
}
