use serde::{Deserialize, Serialize};
use shared::types;

#[cfg(feature = "types")]
use shared::types::{leak_str, ConfStruct, ConfType, Endpoint, OtelReceiver};

/// The main Polling-Processor configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Processor,
}

#[cfg(feature = "types")]
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		ConfStruct::from([("config".to_string(), ConfType::Struct(val.config.into()))])
	}
}

/// A Polling-Processor Configuration
///
/// Every received log message is handled as JSON. For each message, and for each configured
/// `api` endpoint, a web request is sent. The answer is forwarded to the configured OpenTelemetry
/// log receiver (`queue.otel_logger`).
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Processor {
	/// Name of the instance
	pub name: String,

	/// Opentelemetry Listener configuration where the messages are received
	#[serde(default)]
	pub listener: OtelReceiver,

	/// Where to send the requests to
	pub api: Vec<Endpoint>,

	/// Message-Queue Configuration, including the OpenTelemetry Log-Receiver to forward the answers to
	#[serde(default)]
	pub queue: types::Queue,

	/// Message-Parser Configuration
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub parser: Vec<types::Parser>,
}

#[cfg(feature = "types")]
impl From<Processor> for ConfStruct {
	fn from(val: Processor) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak_str(&val.name))),
			(
				"listener".to_string(),
				ConfType::Option(Box::new(ConfType::Struct(val.listener.into()))),
			),
			(
				"api".to_string(),
				ConfType::Vec(Box::new(ConfType::Struct(Endpoint::default().into()))),
			),
			(
				"queue".to_string(),
				ConfType::Option(Box::new(ConfType::Struct(val.queue.into()))),
			),
			(
				"parser".to_string(),
				ConfType::Option(Box::new(ConfType::Vec(Box::new(ConfType::Struct(
					types::Parser::default().into(),
				))))),
			),
		])
	}
}

#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	fn string_of(conf: &ConfStruct, name: &str) -> Option<&'static str> {
		match conf.get(name) {
			Some(ConfType::String(s)) | Some(ConfType::RegEx(s)) => Some(s),
			Some(ConfType::Option(inner)) => match inner.as_ref() {
				ConfType::String(s) | ConfType::RegEx(s) => Some(s),
				_ => None,
			},
			_ => None,
		}
	}

	#[test]
	fn processor_uses_values_and_defaults() {
		let json = serde_json::json!({
			"name": "processor",
			"api": [ { "name": "one", "uri": "http://one" }, { "name": "two", "uri": "http://two" } ],
		});
		let processor: Processor = serde_json::from_value(json).unwrap_or_default();
		assert_eq!(processor.name, "processor");
		assert_eq!(processor.api.len(), 2);
		assert_eq!(processor.listener.port, 4318);
		assert_eq!(processor.listener.path, "/v1/logs");
		assert!(processor.queue.otel_logger.is_empty());
		assert!(processor.parser.is_empty());
	}

	#[test]
	fn processor_requires_api() {
		assert!(serde_json::from_str::<Processor>(r#"{ "name": "processor" }"#).is_err());
	}

	#[test]
	fn processor_schema_uses_values() {
		let conf: ConfStruct = Config {
			config: Processor {
				name: "processor".to_string(),
				..Processor::default()
			},
		}
		.into();

		let Some(ConfType::Struct(inner)) = conf.get("config") else {
			panic!("missing config struct")
		};
		assert_eq!(string_of(inner, "name"), Some("processor"));
		assert!(matches!(inner.get("api"), Some(ConfType::Vec(_))));
		assert!(inner.get("listener").is_some());
		assert!(inner.get("queue").is_some());
		assert!(inner.get("parser").is_some());
		assert!(inner.get("timer").is_none());
	}
}
