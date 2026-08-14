use serde::{Deserialize, Serialize};

// The main configuration for a Azure DCR Export
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: String,
}
