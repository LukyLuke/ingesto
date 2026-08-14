
use std::{collections::HashMap, sync::{Arc, OnceLock}, thread::{self}, time::Duration};

use anyhow::Result;
use opentelemetry::logs::{Logger, LoggerProvider, LogRecord, Severity, AnyValue};
use opentelemetry_otlp::{LogExporter, Protocol, WithExportConfig};
use opentelemetry_sdk::{ Resource, logs::{BatchConfigBuilder, BatchLogProcessor, SdkLogger, SdkLoggerProvider} };
use regex::Regex;
use serde_json::Value;
use serde_json_path::JsonPath;
use tracing::{debug, info, error};

use crate::{queue, types, template::template_string};

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
		let max_msg = self.conf.max_messages;
		let max_time = Duration::from_secs_f32(self.conf.max_seconds as f32);

		info!(message="start processing", max_time=%max_time.as_secs_f32(), max_messages=%max_msg);
		thread::spawn(move || {
			if let Some(otlp) = me.conf.otel_logger.as_ref() {
				loop {
					let q_msg = match me.queue.pull(max_time) {
						Some(m) => m.into().trim().to_string(),
						None => {
							info!(message="queue empty", waited=%max_time.as_secs_f32());
							continue;
						}
					};

					// Parse and return Structured JSON-String
					let msgs = me.parse_message(&q_msg);
					debug!(message="processed message", original=%q_msg);
					info!(message="processed message", size=q_msg.len(), messages=msgs.len());

					for msg in msgs {
						debug!(message="extracted message", msg=%msg);
						match me.send_message(&otlp, &msg, max_msg, max_time) {
							Ok(_) => debug!(message="enqueued message for otlp endpoint", endpoint=%otlp.endpoint, port=%otlp.port, service=%otlp.service),
							Err(e) => {
								me.queue.push_front(msg.into());
								error!(message="failed to enqueue message for otlp endpoint", endpoint=%otlp.endpoint, port=%otlp.port, service=%otlp.service, error=%e)
							}
						};
					}
				}

			} else {
				// Do not process any logs if there is no log receiver
				info!(message="no log processing due to no otel_logger in queue-configuration");
				loop {
					// drop all messages and wait
					self.queue.pull_all();
					thread::sleep(max_time);
				}
			}
		});
	}

	/// Sends out a message as an OTLP Log-Message to one ore more configured receivers
	///
	/// # Arguments
	///
	/// * `conf` - The OTLP Configuration
	/// * `message` - The log emssage to send out
	/// * `count` - Number of messages to enqueue
	/// * `duration` - Duration to wait until the queued messages are sent
	///
	/// # Results
	///
	/// Returns a Result indication wether the message was sent or not.
	fn send_message(&self, conf: &types::OtelLogger, message: &String, count: u16, duration: Duration) -> Result<()> {
		match self.get_logger(conf, count, duration) {
			Ok(logger) => {
				let msg = message.to_owned();
				let mut record = logger.create_log_record();
				record.set_severity_number(Severity::Info);
				record.set_severity_text("INFO");
				record.set_body(AnyValue::String(msg.into()));
				logger.emit(record);
				Ok(())
			},
			Err(e) => Err(e),
		}
	}

	/// Creates and returns an OpenTelemetry Resource
	///
	/// # Arguments
	///
	/// * `conf` - The OTLP Configuration
	/// * `count` - Number of messages to enqueue
	/// * `duration` - Duration to wait until the queued messages are sent
	///
	/// # Returns
	///
	/// An OpenTelemetry Logger
	fn get_logger(&self, conf: &types::OtelLogger, count: u16, duration: Duration) -> Result<SdkLogger> {
		static RESOURCE: OnceLock<Result<SdkLogger>> = OnceLock::new();
		let res = RESOURCE.get_or_init(|| {
			let exporter = LogExporter::builder()
			.with_http()
			.with_protocol(Protocol::HttpBinary)
			.with_endpoint(&conf.get_endpoint("/v1/logs"))
			.build()?;

			// Queue size and time handled by OTEL
			let processor = BatchLogProcessor::builder(exporter)
				.with_batch_config(
					BatchConfigBuilder::default()
						.with_max_queue_size(2048)
						.with_max_export_batch_size(count.into())
						.with_scheduled_delay(duration)
						.build(),
				)
				.build();

			Ok(SdkLoggerProvider::builder()
				.with_resource(
					Resource::builder().with_service_name(conf.service.clone()).build()
				)
				.with_log_processor(processor)
				.build()
				.logger(conf.service.clone())
			)
		});

		match res.as_ref() {
			Ok(logger) => Ok(logger.clone()),
			Err(e) => Err(anyhow::anyhow!(e)),
		}
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
	/// Returns the either the parsed or the raw message as a list
	fn parse_message(&self, raw: &String) -> Vec<String> {
		let parser = self.parser.iter()
			.find_map(|parser| self.regexes.get(&parser.matcher)
				.and_then(|re| if re.is_match(raw) { Some(parser) } else { None }) );
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

	/// Tries to apply a parser to a message and returns all parsed messages as a list of JSON-serialized messages
	fn apply_parser(&self, raw: &String, parser: Option<&types::Parser>) -> Vec<String> {
		match parser {
			Some(parser) => {
				debug!(message="parser", parser=%parser.name, matcher=%parser.matcher, kind=%parser.kind, settings=%parser.settings);

				match parser.kind {
					types::ParserKind::REGEX => {
						let re = match parser.settings.clone() {
							types::ParserSettings::Regex(s) => self.regexes.get(&s),
							_ => None
						};
						re.and_then(|re| Some(self.parse_regex_message(&parser.mapping, raw, re)))
							.unwrap_or_else(|| vec![raw.to_owned()])
					},

					types::ParserKind::JSON => {
						let jpath = match parser.settings.clone() {
							types::ParserSettings::Jpath(s) => self.jsonpath.get(&s),
							_ => self.jsonpath.get("$"),
						};
						jpath.and_then(|jpath| Some(self.parse_json_message(&parser.mapping, raw, &jpath)))
							.unwrap_or_else(|| vec![raw.to_owned()])
					},

					types::ParserKind::CSV => {
						vec![raw.to_owned()]
					},

					types::ParserKind::LEEF => {
						vec![raw.to_owned()]
					},

					types::ParserKind::CEF => {
						vec![raw.to_owned()]
					},

					types::ParserKind::STRUCTURED => {
						vec![raw.to_owned()]
					},

					types::ParserKind::RAW => {
						vec![raw.to_owned()]
					},

					//_ => {
					//	error!(message="not implemented parser", parser=%parser.kind.to_string());
					//	raw.to_owned()
					//}
				}
			},
			None => {
				debug!(message="no parser found");
				vec![raw.to_owned()]
			}
		}
	}

	/// Parses the given String with a regular expression and returns a list of structured messages.
	/// For Regex-Messages there is only one structured message possible for each raw message.
	///
	/// See https://docs.rs/regex/latest/regex/ for regex formats and functions
	///
	/// # Arguments
	///
	/// * `mapping` - All FieldMappings from the Configuration
	/// * `raw` - The raw message which should be a json string
	/// * `re` - The regular expression to apply
	///
	/// # Returns
	///
	/// A list of JSON serialized strings whith all the fields and values as defined in the `mapping` Configuration
	fn parse_regex_message(&self, mapping: &Vec<types::FieldMapping>, raw: &String, re: &Regex) -> Vec<String> {
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

				// Sub-Parser values are returned directly
				// All other values are checked and added to the hashmap below
				if !fld.parser.is_empty() && !val.is_empty() {
					return match self.parser_by_name(fld.parser.as_str()).map(|p| self.apply_parser(&val, Some(p))) {
						Ok(s) => s,
						Err(e) => {
							error!("{:?}", e);
							Vec::new()
						},
					};
				}

				if !val.is_empty() || fld.empty {
					results.insert(fld.name.clone(), val);
				}
			}
		}

		// If nothing was parsed/found, return just an empty list
		if results.is_empty() {
			vec![]
		} else {
			vec![serde_json::to_string(&results).map_or(String::new(), |s| s)]
		}
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
	/// A list of JSON serialized strings with all the fields and values as defined in the `mapping` Configuration
	fn parse_json_message(&self, mapping: &Vec<types::FieldMapping>, raw: &String, jpath: &JsonPath) -> Vec<String> {
		// See https://docs.rs/serde_json_path/latest/serde_json_path/
		// Test: https://serdejsonpath.live/
		let json_root: Value = serde_json::from_str(raw.as_str()).map_or_else(|e|{
			error!(message="json parsing error", json=%raw, error=%e);
			Value::Null
		}, |v| v);

		jpath.query(&json_root)
			.iter()
			.map(|obj| self.parse_json_string(mapping, Arc::new((**obj).clone())))
			.collect()
	}

	/// Applies the configured mappings on the given JSON Values
	///
	/// # Arguments
	///
	/// * `mapping` - All FieldMappings from the Configuration
	/// * `json` - The JSON-Value to apply the mapping to
	///
	/// # Returns
	///
	/// A JSON serialized string with all the fields and values as defined in the `mapping` Configuration
	fn parse_json_string(&self, mapping: &Vec<types::FieldMapping>, json: Arc<Value>) -> String {
		let mut results: HashMap<String, String> = HashMap::new();
		for fld in mapping {
			let mut val: String = String::new();
			if !fld.source.is_empty() {
				// If the source field nale starts with a / a JsonPointer is given,
				// Otherwise a direct field name
				let field_val = match fld.source.get(0..1) {
					Some(c) if c == "/" => &json.pointer(&fld.source).unwrap_or_default(),
					_ => &json[&fld.source],
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

			// Static value via templating
			// Only apply the template if no value is assigned yet. So the configuration can read a value and apply a static value if no value is evaluated yet
			if !fld.static_value.is_empty() && val.is_empty() {
				val = template_string(&fld.static_value, json.clone());
			}

			// Sub-Parser values are returned directly
			// All other values are checked and added to the hashmap below
			if !fld.parser.is_empty() && !val.is_empty() {
				return match self.parser_by_name(fld.parser.as_str()).map(|p| self.apply_parser(&val, Some(p))) {
					Ok(s) => s.join(""),
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

		serde_json::to_string(&results).map_or(String::new(), |s| s)
	}

}


#[cfg(test)]
mod tests {
	use super::*;
	use crate::queue::MessageQueue;

	fn prepare_field_mapping() -> Vec<types::FieldMapping> {
		vec![
			// Empty value in result
			types::FieldMapping {
				name: String::from("map1"),
				source: String::from("grp1"),
				index: 1,
				parser: String::new(),
				empty: true,
				static_value: String::new(),
			},

			// Source Field by name
			types::FieldMapping {
				name: String::from("map2"),
				source: String::from("grp2"),
				index: 2,
				parser: String::new(),
				empty: false,
				static_value: String::new(),
			},

			// Source Field by index
			types::FieldMapping {
				name: String::from("map3"),
				source: String::new(),
				index: 3,
				parser: String::new(),
				empty: false,
				static_value: String::new(),
			},

			// Json Sub-Parser
			types::FieldMapping {
				name: String::from("map4"),
				source: String::from("grp4"),
				index: 0,
				parser: String::from("jsonsub"),
				empty: false,
				static_value: String::new(),
			},
			types::FieldMapping {
				name: String::from("map5"),
				source: String::from("grp5"),
				index: 0,
				parser: String::from("jsonsub"),
				empty: false,
				static_value: String::new(),
			},
			types::FieldMapping {
				name: String::from("map1"),
				source: String::from("/result/grp1"),
				index: 0,
				parser: String::new(),
				empty: false,
				static_value: String::new(),
			},
			types::FieldMapping {
				name: String::from("static"),
				source: String::new(),
				index: 0,
				parser: String::new(),
				empty: false,
				static_value: String::from("UUID: {{ $uuid }}"),
			},
			types::FieldMapping {
				name: String::from("static_response"),
				source: String::new(),
				index: 0,
				parser: String::new(),
				empty: false,
				static_value: String::from("Response: {{ $response/grp/grp/grp }}"),
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

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(json["map1"], String::from(""));
	}

	#[test]
	fn test_parse_json_string_simple() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp1\":\"foobar\" } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(json["map1"], String::from("foobar"));
	}

	#[test]
	fn test_parse_json_string_multiple() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": [ { \"grp1\":\"foobar1\" }, { \"grp1\":\"foobar2\" }, { \"grp1\":\"foobar3\" } ] }");
		let jpath = JsonPath::parse("$.result.*").unwrap();

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		assert_eq!(res.len(), 3);

		let mut json: Value = serde_json::from_str(res[0].as_str()).unwrap();
		assert_eq!(json["map1"], String::from("foobar1"));

		json = serde_json::from_str(res[1].as_str()).unwrap();
		assert_eq!(json["map1"], String::from("foobar2"));

		json = serde_json::from_str(res[2].as_str()).unwrap();
		assert_eq!(json["map1"], String::from("foobar3"));
	}

	#[test]
	fn test_parse_json_string_pointer() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp1\":\"foobar\" } }");
		let jpath = JsonPath::parse("$").unwrap();

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(json["map1"], String::from("foobar"));
	}

	#[test]
	fn test_parse_json_string_parser() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp4\":{\"grp5\":{\"grp2\":\"foobar\"}} } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

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

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(json["map1"], String::from("foobar"));
	}

	#[test]
	fn test_parse_json_string_static() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp\":{\"grp\":{\"grp\":\"foobar\"}} } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		// Check for the static prefix and the length of a result like "UUID: b654bd71-0c3c-4ae1-a32f-662b2d5fb947"
		assert_eq!(json["static"].as_str().unwrap().starts_with("UUID: "), true);
		assert_eq!(json["static"].as_str().unwrap().len(), 42);
	}

	#[test]
	fn test_parse_json_string_static_response() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("{ \"result\": { \"grp\":{\"grp\":{\"grp\":\"foobar\"}} } }");
		let jpath = JsonPath::parse("$.result").unwrap();

		let res = parser.parse_json_message(&mapping, &message, &jpath);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(json["static_response"], String::from("Response: foobar"));
	}


	#[test]
	fn test_parse_regex_message_empty() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("");
		let re = Regex::new(r"^grp1=(?<grp1>\w+) grp2=(?<grp2>\w+) grp3=(?<grp3>\w+)").unwrap();

		let res = parser.parse_regex_message(&mapping, &message, &re);
		assert_eq!(res.len(), 0);
	}

	#[test]
	fn test_parse_regex_message_named() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("grp1=foobar1 grp2=foobar2 grp3=foobar3");
		let re = Regex::new(r"^grp1=(?<grp1>\w+) grp2=(?<grp2>\w+) grp3=(?<grp3>\w+)").unwrap();

		let res = parser.parse_regex_message(&mapping, &message, &re);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(res.len(), 1);
		assert_eq!(json["map1"], String::from("foobar1"));
		assert_eq!(json["map2"], String::from("foobar2"));
		assert_eq!(json["map3"], String::from("foobar3"));
	}

	#[test]
	fn test_parse_regex_message_indexed() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let parser = MessageParser::<String>::new(queue.clone(), types::Queue::default(), get_parser());
		let mapping = prepare_field_mapping();

		let message = String::from("grp1=foobar1 grp2=foobar2 grp3=foobar3");
		let re = Regex::new(r"^grp1=(\w+) grp2=(\w+) grp3=(\w+)").unwrap();

		let res = parser.parse_regex_message(&mapping, &message, &re);
		let json: Value = serde_json::from_str(res.first().unwrap().as_str()).unwrap();

		assert_eq!(res.len(), 1);
		assert_eq!(json["map1"], String::from("foobar1"));
		assert_eq!(json["map2"], String::from("foobar2"));
		assert_eq!(json["map3"], String::from("foobar3"));
	}

}
