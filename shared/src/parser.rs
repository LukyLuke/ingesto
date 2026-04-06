use core::fmt;
use std::{collections::HashMap, sync::Arc, thread::{self}, time::{Duration, Instant}};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, error};

use crate::queue;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Parser {
	#[serde(default)]
	pub name: String,

	#[serde(default)]
	pub matcher: String,

	#[serde(default = "default_parser_kind")]
	pub kind: ParserKind,
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

pub struct MessageParser<T> {
	queue: Arc<queue::MessageQueue<T>>,
	conf: queue::Queue,
	parser: Vec<Parser>,
	regexes: HashMap<String, Regex>,
}

impl<T: Send + 'static + Into<String> + From<String>> MessageParser<T> {
	pub fn new(queue: Arc<queue::MessageQueue<T>>, conf: queue::Queue, parser: Vec<Parser>) -> Self {
		// Precompile Regex
		let regexes = Self::precompile_regex(&parser);

		Self {
			queue,
			conf,
			parser,
			regexes,
		}
	}

	fn precompile_regex(parser: &Vec<Parser>) -> HashMap<String, Regex> {
		let errkey = "error".to_string();
		let mut regexes: HashMap<String, Regex> = HashMap::new();
		for p in parser {
			let (k, re) = match Regex::new(&p.matcher) {
				Ok(re) => (&p.matcher, re),
				Err(e) => {
					error!(message="regex compile", regex=%p.matcher, error=%e);
					(&errkey, Regex::new("^$").unwrap())
				}
			};
			regexes.insert(k.to_owned(), re);
		}
		regexes
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
		let mut parser: Option<&Parser> = None;
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
				match parser.kind {
					ParserKind::REGEX => {
						raw.to_owned()
					},
					ParserKind::JSON => {
						raw.to_owned()
					},
					ParserKind::CSV => {
						raw.to_owned()
					},
					ParserKind::LEEF => {
						raw.to_owned()
					},
					ParserKind::CEF => {
						raw.to_owned()
					},
					ParserKind::STRUCTURED => {
						raw.to_owned()
					},
					ParserKind::RAW => {
						raw.to_owned()
					},
					//_ => {
					//	error!(message="not implemented parser", parser=%parser.kind.to_string());
					//	raw.to_owned()
					//}
				}
			},
			None => raw.to_owned()
		}
	}

}
