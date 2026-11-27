use serde::{Deserialize, Serialize};
use shared::types;
#[cfg(feature = "types")]
use shared::types::{ConfStruct, ConfType, Endpoint, leak_str};

// Default-Wrapper Functions for Serde::Deserialize
fn default_cron_timer() -> String {
	String::from("* */5 * * * *")
}

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
		map.insert(
			"api".to_string(),
			ConfType::Vec(Box::new(ConfType::Struct(Endpoint::default().into()))),
		);
		map.insert(
			"timer".to_string(),
			ConfType::Option(Box::new(ConfType::String(leak_str(&val.timer)))),
		);
		map.insert(
			"queue".to_string(),
			ConfType::Option(Box::new(ConfType::Struct(val.queue.into()))),
		);
		map.insert(
			"parser".to_string(),
			ConfType::Option(Box::new(ConfType::Vec(Box::new(ConfType::Struct(
				types::Parser::default().into(),
			))))),
		);
		map
	}
}


#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	#[test]
	fn polling_uses_values_and_default_timer() {
		let conf: ConfStruct = Polling {
			name: "poller".to_string(),
			timer: "0 * * * * *".to_string(),
			..Polling::default()
		}
		.into();
		assert_eq!(string_of(&conf, "name"), Some("poller"));
		assert_eq!(string_of(&conf, "timer"), Some("0 * * * * *"));

		let conf: ConfStruct = Polling::default().into();
		assert_eq!(string_of(&conf, "timer"), Some("* */5 * * * *"));
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

}
