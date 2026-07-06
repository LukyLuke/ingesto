use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub database: Database,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Database {
	pub name: String,
}
