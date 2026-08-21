use serde::{Deserialize, Serialize};
use shared::types::{DbField, OtelReceiver, Queue};
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
pub(crate) fn default_for_messages() -> String { String::from(".*") }
pub(crate) fn default_postgres_port() -> u16 { 5432 }
pub(crate) fn default_ssl_mode() -> SslMode { SslMode::Disable }


// The main configuration for a Database
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: DbConf,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Config {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(DbConf::default().into())),
		])
	}
}

/// main Configuration to start a Database-Exporter and listen for messages
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
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

#[cfg(feature = "types")]
impl Into<ConfStruct> for DbConf {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("listener".to_string(), ConfType::Struct(OtelReceiver::default().into())),
			("database".to_string(), ConfType::Struct(Database::default().into())),
			("queue".to_string(), ConfType::Struct(Queue::default().into())),
		])
	}
}

/// A Database Connection Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Database {
	/// Name of the Database/Schema
	pub database: String,

	/// Database type to connect to
	pub kind: DbKind,

	/// List of Tables and Field-Matches to insert messages
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	pub tables: Vec<DbTable>,

	/// Authentication for the Database
	#[serde(default)]
	pub auth: Authentication,

	/// Database-Specific Connection Settings
	#[serde(default)]
	pub connection: Connection,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Database {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("database".to_string(), ConfType::String),
			("kind".to_string(), ConfType::Enum( DbKind::default().into() )),
			("tables".to_string(), ConfType::Vec( Box::new(ConfType::Struct( DbTable::default().into() )) )),
			("auth".to_string(), ConfType::Enum( Authentication::None.into() )),
			("connection".to_string(), ConfType::Struct(Connection::default().into())),
		])
	}
}

/// How to authenticate against the Database
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	#[default]
	None,

	/// Use the system default passfile (~/.pgpass or ~/.mysql)
	Passfile,

	/// Use a Username and Password
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Simple { user: String, pass: String },
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Authentication {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("None".to_string(), ConfType::EnumValue),
			("Passfile".to_string(), ConfType::EnumValue),
			("Simple".to_string(), ConfType::EnumParams("user", "pass")),
		])
	}
}

/// A Database-Connection Configuration
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
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
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub root_cert: Option<String>,

	/// An SSL-Client Certificate
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub ssl_cert: Option<String>,

	/// An SSL-Key for the connection
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub ssl_key: Option<String>,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for Connection {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("host".to_string(), ConfType::String),
			("port".to_string(), ConfType::UInt),
			("ssl_mode".to_string(), ConfType::Enum( SslMode::default().into() )),
			("root_cert".to_string(), ConfType::Option( Box::new(ConfType::String) )),
			("ssl_cert".to_string(), ConfType::Option( Box::new(ConfType::String) )),
			("ssl_key".to_string(), ConfType::Option( Box::new(ConfType::String) )),
		])
	}
}

/// What kind of Database should be conencted
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum DbKind {
	#[default]
	PostgreSQL,
	MariaDB,
	SQLite,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for DbKind {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("PostgreSQL".to_string(), ConfType::EnumValue),
			("MariaDB".to_string(), ConfType::EnumValue),
			("SQLite".to_string(), ConfType::EnumValue),
		])
	}
}

/// SSL-Mode for the Database-Connection
/// This varries from Postgres to MySQL/MariaDB and SQLite
#[cfg(feature = "types")]
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub enum SslMode {
	#[default]
	Disable,
	Allow,
	Prefer,
	Require,
	VerifyCa,
	VerifyFull,
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for SslMode {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("Disable".to_string(), ConfType::EnumValue),
			("Allow".to_string(), ConfType::EnumValue),
			("Prefer".to_string(), ConfType::EnumValue),
			("Require".to_string(), ConfType::EnumValue),
			("VerifyCa".to_string(), ConfType::EnumValue),
			("VerifyFull".to_string(), ConfType::EnumValue),
		])
	}
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

#[cfg(feature = "types")]
impl Default for DbTable {
	fn default() -> Self {
		Self {
			name: String::from("undefined"),
			for_messages: default_for_messages(),
			fields: Vec::new(),
		}
	}
}

#[cfg(feature = "types")]
impl Into<ConfStruct> for DbTable {
	fn into(self) -> ConfStruct {
		ConfStruct::from([
			("name".to_string(), ConfType::String),
			("for_messages".to_string(), ConfType::RegEx),
			("fields".to_string(), ConfType::Vec( Box::new( ConfType::Enum( DbField::default().into() ) ) )),
		])
	}
}
