
use serde::{Deserialize, Serialize};
use shared::types::{DbField, OtelReceiver, Queue};

// Default-Wrapper Functions for Serde::Deserialize
pub(crate) fn default_for_messages() -> String { String::from(".*") }
pub(crate) fn default_postgres_port() -> u16 { 5432 }
pub(crate) fn default_ssl_mode() -> SslMode { SslMode::Disable }


// The main configuration for a Database
#[cfg(feature = "types")]
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: DbConf,
}

/// main Configuration to start a Database-Exporter and listen for messages
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbConf {
	/// Just a name for this instance for logging and identification
	pub name: String,

	/// Opentelemetry Listener configuration
	#[serde(default)]
	pub listener: OtelReceiver,

	/// Database Exporter Configuration
	#[serde(default)]
	pub database: Database,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,
}

/// A Database Connection Configuration
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Database {
	/// Name of the Database/Schema
	pub database: String,

	/// Database type to connect to
	pub kind: DbKind,

	/// List of Tables and Field-Matches to insert messages
	#[serde(default)]
	pub tables: Vec<DbTable>,

	/// Authentication for the Database
	#[serde(default)]
	pub auth: Option<Authentication>,

	/// Database-Specific Connection Settings
	#[serde(default)]
	pub connection: Connection,

}

/// How to authenticate against the Database
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	/// Use the system default passfile (~/.pgpass or ~/.mysql)
	Passfile,

	/// Use a Username and Password
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Simple { user: String, pass: String },
}

/// A Database-Connection Configuration
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Connection {
	/// Host-Name, IP-Address or FileName to use as the database
	pub host: String,

	/// Port to connect to; default is postgres 5432
	#[serde(default = "default_postgres_port")]
	pub port: u16,

	/// SSL-Mode to connect to; Default is SSL-Disabled
	#[serde(default = "default_ssl_mode")]
	pub ssl_mode: SslMode,

	/// Path to the ROOT-Certificate if not the system defaults should be used
	#[serde(default)]
	pub root_cert: Option<String>,

	/// An SSL-Client Certificate
	#[serde(default)]
	pub ssl_cert: Option<String>,

	/// An SSL-Key for the connection
	#[serde(default)]
	pub ssl_key: Option<String>,
}

/// What kind of Database should be conencted
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DbKind {
	PostgreSQL,
	MariaDB,
	SQLite,
}

/// SSL-Mode for the Database-Connection
/// This varries from Postgres to MySQL/MariaDB and SQLite
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SslMode {
	Disable,
	Allow,
	Prefer,
	Require,
	VerifyCa,
	VerifyFull,
}

/// Represents a Database-Table with a simple field-mapping from a message to the table schema
///
/// ```toml
/// [[config.database.tables]]
/// name = "example"
/// for_messages = ".*"
/// fields = [
///   { kind = "String", name = "dbfield", origin = "message" },
///   { kind = "Int",    name = "dbint",   origin = "severity" },
///   { kind = "Float",  name = "dbfloat", origin = "some_float" },
///   { kind = "Bool",   name = "dbbool", origin = "some_boolean" },
/// ]
/// ```
#[cfg(feature = "types")]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DbTable {
	/// Table-Name
	pub name: String,

	/// Regular Expression to select messages which should be converted into this schema
	#[serde(default = "default_for_messages")]
	pub for_messages: String,

	/// Define the Database-Fields with a type to convert the values into
	#[serde(default)]
	pub fields: Vec<DbField>,
}

