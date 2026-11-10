use serde::{Deserialize, Serialize};
use shared::types::{ConfStruct, ConfType};

// The main configuration for a Azure DCR Export
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: String,
}

#[cfg(feature = "types")]
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		ConfStruct::from([
			("config".to_string(), ConfType::String(Box::leak(val.config.into_boxed_str()))),
		])
	}
}

#[cfg(all(test, feature = "types"))]
mod tests {
	use super::*;

	#[test]
	fn test_config_uses_value() {
		let conf: ConfStruct = Config { config: "some value".to_string() }.into();
		assert_eq!(conf.get("config"), Some(&ConfType::String("some value")));
	}

	#[test]
	fn test_config_empty_value() {
		let conf: ConfStruct = Config::default().into();
		assert_eq!(conf.get("config"), Some(&ConfType::String("")));
	}
}
