use std::{collections::HashMap, sync::Arc, thread::{self}, time::{Duration, Instant}};
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
							_ => None
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
				results.insert(fld.name.clone(), val);
			}
		}
		serde_json::to_string(&results).map_or(String::new(), |s| s)
	}

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
					val = obj[&fld.source].as_str().map_or("", |s| s).to_string();
				}

				if !fld.parser.is_empty() {
					// TODO
				}
				results.insert(fld.name.clone(), val);
			}
		}

		serde_json::to_string(&results).map_or(String::new(), |s| s)
	}

}
