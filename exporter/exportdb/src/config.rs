use serde::{Deserialize, Serialize};
use shared::types::{OtelReceiver, Queue};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: DbConf,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbConf {
	pub name: String,

	#[serde(default)]
	pub listener: OtelReceiver,

	#[serde(default)]
	pub database: Database,

	#[serde(default)]
	pub queue: Queue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Database {

}
impl Default for Database {
	fn default() -> Self {
		Self {}
	}
}
