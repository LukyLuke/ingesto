use serde::{Deserialize, Serialize};
use shared::types::OtelReceiver;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub database: Database,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Database {
	pub name: String,

	pub listener: OtelReceiver,
}
