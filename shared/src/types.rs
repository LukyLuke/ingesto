use core::fmt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct Queue {
	#[serde(default = "default_messages")]
	pub max_messages: u16,

	#[serde(default = "default_time")]
	pub max_seconds: u16,

	#[serde(default = "default_size")]
	pub max_size: usize,
}
fn default_messages() -> u16 { 1024 }
fn default_size() -> usize { 65535 }
fn default_time() -> u16 { 60 }

impl Default for Queue {
	fn default() -> Self {
		Self {
			max_messages: default_messages(),
			max_seconds: default_time(),
			max_size: default_size(),
		}
	}
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Parser {
	#[serde(default)]
	pub name: String,

	#[serde(default)]
	pub matcher: String,

	#[serde(default = "default_parser_kind")]
	pub kind: ParserKind,

	#[serde(default = "default_parser_setting")]
	pub settings: ParserSettings,

	#[serde(default)]
	pub mapping: Vec<FieldMapping>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ParserKind {
	RAW,
	REGEX,
	JSON,
	CSV,
	CEF,
	LEEF,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ParserSettings {
	Nothing,
	Regex(String),
	Jpath(String),
}
fn default_parser_setting() ->ParserSettings { ParserSettings::Nothing }
impl fmt::Display for ParserSettings {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldMapping {
	pub name: String,

	#[serde(default)]
	pub source: String,

	#[serde(default)]
	pub index: usize,

	#[serde(default)]
	pub parser: String,
}
