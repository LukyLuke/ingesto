use std::{collections::HashMap, sync::Arc, thread::{self}, time::{Duration, Instant}};
use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use serde_json_path::JsonPath;
use tracing::{debug, info, error};

use crate::{queue, types::FieldMapping};
use crate::types;

pub struct MessageParser<T> {
	queue: Arc<queue::MessageQueue<T>>,
	conf: types::Queue,
	parser: Vec<types::Parser>,
	regexes: HashMap<String, Regex>,
	jsonpath: HashMap<String, JsonPath>,
}

impl<T: Send + 'static + Into<String> + From<String>> MessageParser<T> {
	pub fn new(queue: Arc<queue::MessageQueue<T>>, conf: types::Queue, parser: Vec<types::Parser>) -> Self {
		// Precompile
		let regexes = Self::precompile_regex(&parser);
		let jsonpath = Self::precompile_jsonpath(&parser);

		Self {
			queue,
			conf,
			parser,
			regexes,
			jsonpath,
		}
	}

	fn precompile_regex(parser: &Vec<types::Parser>) -> HashMap<String, Regex> {
		let mut regexes: HashMap<String, Regex> = HashMap::new();
		for p in parser {
			// Precompile the matcher Regex
			let re = match Regex::new(&p.matcher) {
				Ok(re) => {
					info!(message="regex compile", regex=%p.matcher);
					re
				},
				Err(e) => {
					error!(message="regex compile", regex=%p.matcher, error=%e);
					Regex::new("^$").unwrap()
				}
			};
			regexes.insert(p.matcher.to_owned(), re);

			// Precompile the Parser-Regex
			match p.settings.clone() {
				types::ParserSettings::Regex(setting) => {
					let re = match Regex::new(&setting) {
						Ok(re) => {
							info!(message="regex compile", regex=%setting);
							re
						},
						Err(e) => {
							error!(message="regex compile", regex=%setting, error=%e);
							Regex::new("^$").unwrap()
						}
					};
					regexes.insert(setting.to_owned(), re);
				},
				_ => {},
			};
		}
		regexes
	}

	fn precompile_jsonpath(parser: &Vec<types::Parser>) -> HashMap<String, JsonPath> {
		let mut jsonpath: HashMap<String, JsonPath> = HashMap::new();
		jsonpath.insert(String::from("$"), JsonPath::parse("$").unwrap());

		for p in parser {
			match p.settings.clone() {
				types::ParserSettings::Jpath(setting) => {
					let jpath = match JsonPath::parse(&setting) {
						Ok(jpath) => {
							info!(message="json path compile", jsonpath=%setting);
							jpath
						},
						Err(e) => {
							error!(message="json path compile", jsonpath=%setting, error=%e);
							JsonPath::parse("$").unwrap()
						}
					};
					jsonpath.insert(setting.to_owned(), jpath);
				},
				_ => {},
			};
		}
		jsonpath
	}

	pub fn run(self: Arc<Self>) {
		let me = Arc::clone(&self);
		let max_size = self.conf.max_size - 2; // -2 for the [] around the messages
		let max_msg = self.conf.max_messages;
		let max_time = Duration::from_secs_f32(self.conf.max_seconds as f32);

		info!(message="start processing", max_time=%max_time.as_secs_f32(), max_messages=%max_msg, max_message_size=%max_size);
		thread::spawn(move || {
			loop {
				let start = Instant::now();
				let mut msg = String::with_capacity(max_size);
				let mut count: u16 = 0;
				let mut chars:usize = 0;

				msg.push('[');
				while chars < max_size {
					let elapsed = start.elapsed();
					let remaining = max_time - elapsed;
					let q_msg = match me.queue.pull(remaining) {
						Some(m) => m.into().trim().to_string(),
						None => {
							info!(message="queue empty", waited=%remaining.as_secs_f32());
							break;
						}
					};

					// Parse and return Structured JSON-String
					let p_msg = me.parse_message(&q_msg);

					// If the final message would be too long, close the old message and push the current one back to the front
					// But if this is the first message and that one is already too long, add it and anyways
					if (count > 0) && (chars + p_msg.chars().count() > max_size) {
						me.queue.push_front(q_msg.into());
						break;
					}

					if count > 0 {
						msg.push(',');
					}
					msg.push_str(&p_msg);
					chars = msg.chars().count();

					count += 1;
					if count >= max_msg {
						break;
					}
				}
				msg.push(']');

				// TODO: Send the message out
				info!(message="messages processed", count=%count, size=%chars);
				debug!(message="messages", processed=%msg);
			}
		});
	}

	/// Tries to find an appropriate parser for the given message and applies it then
	/// If no parser can be found, the raw message is returned
	///
	/// # Arguments
	///
	/// * `raw` - The raw message as a string to parse
	///
	/// # Results
	///
	/// Returns the either the parsed or the raw message
	fn parse_message(&self, raw: &String) -> String {
		// First find the right parser
		let mut parser: Option<&types::Parser> = None;
		for p in &self.parser {
			let re = match self.regexes.get(&p.matcher) {
				Some(re) => re,
				None => continue
			};
			if re.is_match(raw) {
				parser = Some(&p);
			}
		}
		self.apply_parser(raw, parser)
	}

	/// Finds a parser by its name
	///
	/// # Arguments
	///
	/// * `name` - Name of the parser
	///
	/// # Results
	///
	/// The parser or None
	fn parser_by_name(&self, name: &str) -> Result<&types::Parser> {
		for p in &self.parser {
			if p.name.as_str() == name {
				return Ok(&p);
			}
		}
		Err(anyhow::anyhow!("no parser found by the name {}", name))
	}

	/// Tries to apply a parser to a message
	fn apply_parser(&self, raw: &String, parser: Option<&types::Parser>) -> String {
		// Parse the Message if possible
		match parser {
			Some(parser) => {
				debug!(message="parser", parser=%parser.name, matcher=%parser.matcher, kind=%parser.kind, settings=%parser.settings);

				match parser.kind {
					types::ParserKind::REGEX => {
						let re = match parser.settings.clone() {
							types::ParserSettings::Regex(s) => self.regexes.get(&s),
							_ => None
						};
						re.and_then(|re| Some(self.parse_regex_string(&parser.mapping, raw, re)))
							.unwrap_or_else(|| raw.to_owned())
					},

					types::ParserKind::JSON => {
						let jpath = match parser.settings.clone() {
							types::ParserSettings::Jpath(s) => self.jsonpath.get(&s),
							_ => self.jsonpath.get("$"),
						};
						jpath.and_then(|jpath| Some(self.parse_json_string(&parser.mapping, raw, &jpath)))
							.unwrap_or_else(|| raw.to_owned())
					},

					types::ParserKind::CSV => {
						raw.to_owned()
					},

					types::ParserKind::LEEF => {
						raw.to_owned()
					},

					types::ParserKind::CEF => {
						raw.to_owned()
					},

					types::ParserKind::STRUCTURED => {
						raw.to_owned()
					},

					types::ParserKind::RAW => {
						raw.to_owned()
					},

					//_ => {
					//	error!(message="not implemented parser", parser=%parser.kind.to_string());
					//	raw.to_owned()
					//}
				}
			},
			None => {
				debug!(message="no parser found");
				raw.to_owned()
			}
		}
	}

	fn parse_regex_string(&self, mapping: &Vec<FieldMapping>, raw: &String, re: &Regex) -> String {
		// See https://docs.rs/regex/latest/regex/
		let mut results: HashMap<String, String> = HashMap::new();
		for capture in re.captures_iter(raw) {
			for fld in mapping {
				let mut val: String = String::new();
				if !fld.source.is_empty() {
					val = capture.name(&fld.source).map_or("", |v| v.as_str()).to_owned();
				}
				if val.is_empty() && fld.index > 0 {
					val = capture.get(fld.index).map_or("", |v| v.as_str()).to_owned();
				}

				if !fld.parser.is_empty() {
					// TODO
				}

				if !val.is_empty() || fld.empty {
					results.insert(fld.name.clone(), val);
				}
			}
		}
		serde_json::to_string(&results).map_or(String::new(), |s| s)
	}

	/// Parses the given String as JSON and applies the JsonPath to get the main object.
	///
	/// See serde_json_path docs: https://docs.rs/serde_json_path/latest/serde_json_path/
	/// Test JsonPath on: https://serdejsonpath.live/
	///
	/// # Arguments
	///
	/// * `mapping` - All FieldMappings from the Configuration
	/// * `raw` - The raw message which should be a json string
	/// * `jpath` - A JsonPath to mark the root object inside the json object
	///
	/// # Returns
	///
	/// A JSON serializes string with all the fields and values as defined in the `mapping` Configuration
	fn parse_json_string(&self, mapping: &Vec<FieldMapping>, raw: &String, jpath: &JsonPath) -> String {
		// See https://docs.rs/serde_json_path/latest/serde_json_path/
		// Test: https://serdejsonpath.live/
		let mut results: HashMap<String, String> = HashMap::new();
		let json: Value = serde_json::from_str(raw.as_str()).map_or_else(|e|{
			error!(message="json parsing error", json=%raw, error=%e);
			Value::Null
		}, |v| v);

		for obj in jpath.query(&json).iter() {
			for fld in mapping {
				let mut val: String = String::new();
				if !fld.source.is_empty() {
					// If the source field nale starts with a / a JsonPointer is given,
					// Otherwise a direct field name
					let field_val = match fld.source.get(0..1) {
						Some(c) if c == "/" => &obj.pointer(&fld.source).unwrap_or_default(),
						_ => &obj[&fld.source],
					};
					// Extract the JsonValue form the field
					val = match field_val {
						Value::String(s) => String::from(s),
						Value::Bool(b) => format!("{}", b),
						Value::Number(n) => format!("{}", n),
						Value::Array(v) => serde_json::to_string(v).map_or(String::new(), |s| s),
						Value::Object(v) => serde_json::to_string(v).map_or(String::new(), |s| s),
						_ => String::new(),
					};
				}

				// Sub-Parser values are returned directly
				// All other values are checked and added to the hashmap below
				if !fld.parser.is_empty() && !val.is_empty() {
					return match self.parser_by_name(fld.parser.as_str()).map(|p| self.apply_parser(&val, Some(p))) {
						Ok(s) => s,
						Err(e) => {
							error!("{:?}", e);
							String::new()
						},
					};
				}

				if !val.is_empty() || fld.empty {
					results.insert(fld.name.clone(), val);
				}
			}
		}

		serde_json::to_string(&results).map_or(String::new(), |s| s)
	}

}


#[cfg(test)]
mod tests {
	use super::*;
	use crate::queue::MessageQueue;

	fn prepare_field_mapping() -> Vec<FieldMapping> {
		vec![
			// Empty value in result
			FieldMapping {
				name: String::from("map1"),
				source: String::from("grp1"),
				index: 0,
				parser: String::new(),
				empty: true,
			},

			// Source Field by name
			FieldMapping {
				name: String::from("map2"),
				source: String::from("grp2"),
				index: 0,
				parser: String::new(),
				empty: false,
			},

			// Source Field by index
			FieldMapping {
				name: String::from("map3"),
				source: String::new(),
				index: 3,
				parser: String::new(),
				empty: false,
			},

			// Json Sub-Parser
			FieldMapping {
				name: String::from("map4"),
				source: String::from("grp4"),
				index: 0,
				parser: String::from("jsonsub"),
				empty: false,
			},
			FieldMapping {
				name: String::from("map5"),
				source: String::from("grp5"),
				index: 0,
				parser: String::from("jsonsub"),
				empty: false,
			},
			FieldMapping {
				name: String::from("map1"),
				source: String::from("/result/grp1"),
				index: 0,
				parser: String::new(),
				empty: false,
			},
		]
	}

	fn get_parser() -> Vec<types::Parser> {
		vec![
			types::Parser{
				name: String::from("regex"),
				matcher: String::from("^regex.*"),
				kind: types::ParserKind::REGEX,
				settings: types::ParserSettings::Nothing,
				mapping: prepare_field_mapping(),
			},
			types::Parser{
				name: String::from("regexsub"),
				matcher: String::from("^regexsub.*"),
				kind: types::ParserKind::REGEX,
				settings: types::ParserSettings::Nothing,
				mapping: prepare_field_mapping(),
			},
			types::Parser{
				name: String::from("json"),
				matcher: String::from("^json.*"),
				kind: types::ParserKind::JSON,
				settings: types::ParserSettings::Nothing,
				mapping: prepare_field_mapping(),
			},
			types::Parser{
				name: String::from("jsonsub"),
				matcher: String::from("^jsonsub.*"),
				kind: types::ParserKind::JSON,
				settings: types::ParserSettings::Nothing,
				mapping: prepare_field_mapping(),
			},
		]
	}

	#[test]
	fn test_parse_json_string_empty() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp0\":\"foobar\" } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_string(&mapping, &message, &jpath);

		assert_eq!(res, String::from("{\"map1\":\"\"}"));
	}

	#[test]
	fn test_parse_json_string_simple() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp1\":\"foobar\" } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_string(&mapping, &message, &jpath);

		assert_eq!(res, String::from("{\"map1\":\"foobar\"}"));
	}

	#[test]
	fn test_parse_json_string_pointer() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp1\":\"foobar\" } }");
		let jpath = JsonPath::parse("$").unwrap();

		let res = parser.parse_json_string(&mapping, &message, &jpath);

		assert_eq!(res, String::from("{\"map1\":\"foobar\"}"));
	}

	#[test]
	fn test_parse_json_string_parser() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp4\":{\"grp5\":{\"grp2\":\"foobar\"}} } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_string(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.as_str()).unwrap();

		assert_eq!(json["map1"], String::from(""));
		assert_eq!(json["map2"], String::from("foobar"));
	}

	#[test]
	fn test_parse_json_string_parser_override() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp4\":{\"grp5\":{\"grp1\":\"foobar\"}} } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_string(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.as_str()).unwrap();

		assert_eq!(json["map1"], String::from("foobar"));
	}


}
