use serde::{Deserialize, Serialize};
use shared::types::{DbField, OtelReceiver, Queue};
use shared::types::{ConfStruct, ConfType};

// Default-Wrapper Functions for Serde::Deserialize
pub(crate) fn default_for_messages() -> String { String::from(".*") }
pub(crate) fn default_postgres_port() -> u16 { 5432 }
pub(crate) fn default_ssl_mode() -> SslMode { SslMode::Disable }

/// Turns a value into a `'static` str, as `ConfType` only holds static strings.
/// The configuration structure is built once, so the leaked memory is negligible.
#[cfg(feature = "types")]
fn leak(val: &str) -> &'static str {
	Box::leak(val.to_owned().into_boxed_str())
}


// The main configuration for a Database
#[cfg(feature = "types")]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: DbConf,
}

#[cfg(feature = "types")]
impl From<Config> for ConfStruct {
	fn from(val: Config) -> Self {
		ConfStruct::from([
			("config".to_string(), ConfType::Struct(val.config.into())),
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
impl From<DbConf> for ConfStruct {
	fn from(val: DbConf) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak(&val.name))),
			("listener".to_string(), ConfType::Option( Box::new(ConfType::Struct(val.listener.into())) )),
			("database".to_string(), ConfType::Option( Box::new(ConfType::Struct(val.database.into())) )),
			("queue".to_string(), ConfType::Option( Box::new(ConfType::Struct(val.queue.into())) )),
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
impl From<Database> for ConfStruct {
	fn from(val: Database) -> Self {
		ConfStruct::from([
			("database".to_string(), ConfType::String(leak(&val.database))),
			("kind".to_string(), ConfType::Enum( val.kind.clone().into(), val.kind.name() )),
			("tables".to_string(), ConfType::Option( Box::new(ConfType::Vec( Box::new(ConfType::Struct( DbTable::default().into() )) )) )),
			("auth".to_string(), ConfType::Option( Box::new(ConfType::Enum( val.auth.clone().into(), val.auth.name() )) )),
			("connection".to_string(), ConfType::Option( Box::new(ConfType::Struct(val.connection.into())) )),
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
impl Authentication {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::None => "None",
			Self::Passfile => "Passfile",
			Self::Simple { .. } => "Simple",
		}
	}
}

#[cfg(feature = "types")]
impl From<Authentication> for ConfStruct {
	fn from(_val: Authentication) -> Self {
		ConfStruct::from([
			("None".to_string(), ConfType::EnumValue),
			("Passfile".to_string(), ConfType::EnumValue),
			("Simple".to_string(), ConfType::Struct(ConfStruct::from([
				("user".to_string(), ConfType::String("")),
				("pass".to_string(), ConfType::String("")),
			]))),
		])
	}
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
impl Default for Connection {
	fn default() -> Self {
		Self {
			host: String::new(),
			port: default_postgres_port(),
			ssl_mode: default_ssl_mode(),
			root_cert: None,
			ssl_cert: None,
			ssl_key: None,
		}
	}
}

#[cfg(feature = "types")]
impl From<Connection> for ConfStruct {
	fn from(val: Connection) -> Self {
		ConfStruct::from([
			("host".to_string(), ConfType::String(leak(&val.host))),
			("port".to_string(), ConfType::Option( Box::new(ConfType::UInt( val.port.into() )) )),
			("ssl_mode".to_string(), ConfType::Option( Box::new(ConfType::Enum( val.ssl_mode.clone().into(), val.ssl_mode.name() )) )),
			("root_cert".to_string(), ConfType::Option( Box::new(ConfType::String(leak(val.root_cert.as_deref().unwrap_or_default()))) )),
			("ssl_cert".to_string(), ConfType::Option( Box::new(ConfType::String(leak(val.ssl_cert.as_deref().unwrap_or_default()))) )),
			("ssl_key".to_string(), ConfType::Option( Box::new(ConfType::String(leak(val.ssl_key.as_deref().unwrap_or_default()))) )),
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
impl DbKind {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::PostgreSQL => "PostgreSQL",
			Self::MariaDB => "MariaDB",
			Self::SQLite => "SQLite",
		}
	}
}

#[cfg(feature = "types")]
impl From<DbKind> for ConfStruct {
	fn from(_val: DbKind) -> Self {
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
impl SslMode {
	/// The name of the variant as it is written in a configuration file
	pub fn name(&self) -> &'static str {
		match self {
			Self::Disable => "Disable",
			Self::Allow => "Allow",
			Self::Prefer => "Prefer",
			Self::Require => "Require",
			Self::VerifyCa => "VerifyCa",
			Self::VerifyFull => "VerifyFull",
		}
	}
}

#[cfg(feature = "types")]
impl From<SslMode> for ConfStruct {
	fn from(_val: SslMode) -> Self {
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
impl From<DbTable> for ConfStruct {
	fn from(val: DbTable) -> Self {
		ConfStruct::from([
			("name".to_string(), ConfType::String(leak(&val.name))),
			("for_messages".to_string(), ConfType::Option( Box::new(ConfType::RegEx(leak(&val.for_messages))) )),
			("fields".to_string(), ConfType::Option( Box::new(ConfType::Vec( Box::new( DbField::conf_type() )) ) )),
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
	fn test_connection_uses_values() {
		let conf: ConfStruct = Connection {
			host: "db.example.org".to_string(),
			port: 3306,
			ssl_mode: SslMode::VerifyFull,
			root_cert: Some("/ca.pem".to_string()),
			ssl_cert: Some("/cert.pem".to_string()),
			ssl_key: Some("/key.pem".to_string()),
		}.into();

		assert_eq!(string_of(&conf, "host"), Some("db.example.org"));
		assert_eq!(conf.get("port"), Some(&ConfType::Option(Box::new(ConfType::UInt(3306)))));
		assert!(matches!(conf.get("ssl_mode"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "VerifyFull"))));
		assert_eq!(string_of(&conf, "root_cert"), Some("/ca.pem"));
		assert_eq!(string_of(&conf, "ssl_cert"), Some("/cert.pem"));
		assert_eq!(string_of(&conf, "ssl_key"), Some("/key.pem"));
	}

	#[test]
	fn test_connection_default_values() {
		let conf: ConfStruct = Connection::default().into();

		assert_eq!(string_of(&conf, "host"), Some(""));
		assert_eq!(conf.get("port"), Some(&ConfType::Option(Box::new(ConfType::UInt(5432)))));
		assert!(matches!(conf.get("ssl_mode"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "Disable"))));
		assert_eq!(string_of(&conf, "root_cert"), Some(""));
		assert_eq!(string_of(&conf, "ssl_cert"), Some(""));
		assert_eq!(string_of(&conf, "ssl_key"), Some(""));
	}

	#[test]
	fn test_database_uses_values() {
		let conf: ConfStruct = Database {
			database: "logs".to_string(),
			kind: DbKind::MariaDB,
			tables: Vec::new(),
			auth: Authentication::Passfile,
			connection: Connection::default(),
		}.into();

		assert_eq!(string_of(&conf, "database"), Some("logs"));
		assert!(matches!(conf.get("kind"), Some(ConfType::Enum(_, "MariaDB"))));
		assert!(matches!(conf.get("auth"), Some(ConfType::Option(inner)) if matches!(inner.as_ref(), ConfType::Enum(_, "Passfile"))));
	}

	#[test]
	fn test_authentication_simple_does_not_expose_secrets() {
		let auth = Authentication::Simple { user: "me".to_string(), pass: "secret".to_string() };
		assert_eq!(auth.name(), "Simple");

		let conf: ConfStruct = auth.into();
		let Some(ConfType::Struct(simple)) = conf.get("Simple") else {
			panic!("missing Simple variant");
		};
		assert_eq!(string_of(simple, "user"), Some(""));
		assert_eq!(string_of(simple, "pass"), Some(""));
	}

	#[test]
	fn test_db_table_uses_values() {
		let conf: ConfStruct = DbTable {
			name: "example".to_string(),
			for_messages: "^foo.*".to_string(),
			fields: Vec::new(),
		}.into();

		assert_eq!(string_of(&conf, "name"), Some("example"));
		assert_eq!(string_of(&conf, "for_messages"), Some("^foo.*"));
	}

	#[test]
	fn test_config_uses_nested_values() {
		let conf: ConfStruct = Config {
			config: DbConf {
				name: "my-exporter".to_string(),
				..DbConf::default()
			},
		}.into();

		let Some(ConfType::Struct(inner)) = conf.get("config") else {
			panic!("missing config struct");
		};
		assert_eq!(string_of(inner, "name"), Some("my-exporter"));
		assert!(inner.get("database").is_some());
	}

	#[test]
	fn test_variant_names() {
		assert_eq!(DbKind::SQLite.name(), "SQLite");
		assert_eq!(SslMode::VerifyCa.name(), "VerifyCa");
		assert_eq!(Authentication::None.name(), "None");
	}
}
