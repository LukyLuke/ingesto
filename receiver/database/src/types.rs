use serde::{Deserialize, Serialize};
use shared::types::{ConfStruct, ConfType};

// The main configuration for a Azure DCR Export
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: String,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Config {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("config".to_string(), ConfType::String),
		])
	}
}
