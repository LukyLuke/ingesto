use std::fmt::Display;

use serde::{Deserialize, Serialize};
use shared::types::{DbField, OtelReceiver, Queue};
use sqlx::{mysql::MySqlConnectOptions, postgres::PgConnectOptions, sqlite::SqliteConnectOptions};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: DbConf,
}

/// main Configuration to start a Database-Exporter and listen for messages
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
impl Default for Database {
	fn default() -> Self {
		Self {
			database: String::new(),
			kind: DbKind::PostgreSQL,
			tables: Vec::new(),
			auth: None,
			connection: Connection::default(),
		}
	}
}
impl Database {
	/// Returns postgres connection options
	pub(crate) fn get_postgres_options(&self) -> PgConnectOptions {
		// If authentication is not set to Pgpass
		let mut opt = match self.auth.as_ref() {
			Some(Authentication::Passfile) => {
				tracing::info!("Using 'Passfile' requires a '~/.pgpass' file or the env 'PGPASSFILE' pointing to a different loaction.");
				PgConnectOptions::new()
			}
			_ => PgConnectOptions::new_without_pgpass(),
		};

		// Default settings
		opt = opt.host(&self.connection.host)
			.port(self.connection.port)
			.database(&self.database);

		opt = match &self.auth {
			Some(auth) => {
				match auth {
					Authentication::Simple { user, pass } => opt.username(user).password(pass),
					_ => opt,
				}
			},
			None => opt,
		};

		// SSL Related
		opt = match &self.connection.mode {
			SslMode::Disable => opt.ssl_mode(sqlx::postgres::PgSslMode::Disable),
			SslMode::Allow => opt.ssl_mode(sqlx::postgres::PgSslMode::Allow),
			SslMode::Prefer => opt.ssl_mode(sqlx::postgres::PgSslMode::Prefer),
			SslMode::Require => opt.ssl_mode(sqlx::postgres::PgSslMode::Require),
			SslMode::VerifyCa => opt.ssl_mode(sqlx::postgres::PgSslMode::VerifyCa),
			SslMode::VerifyFull => opt.ssl_mode(sqlx::postgres::PgSslMode::VerifyFull),
		};
		if let Some(val) = &self.connection.root_cert { opt = opt.ssl_root_cert(val); }
		if let Some(val) = &self.connection.ssl_cert { opt = opt.ssl_client_cert(val); }
		if let Some(val) = &self.connection.ssl_key { opt = opt.ssl_client_key(val); }

		opt
	}

	/// Initiate a new MySQL-Connection
	pub(crate) fn get_mysql_options(&self) -> MySqlConnectOptions {
		MySqlConnectOptions::new()
	}

	/// Initiate a new SQLite Connection
	pub(crate) fn get_sqlite_options(&self) -> SqliteConnectOptions {
		SqliteConnectOptions::new()
	}
}

/// How to authenticate against the Database
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	/// Use the system default passfile (~/.pgpass or ~/.mysql)
	Passfile,

	/// Use a Username and Password
	Simple { user: String, pass: String },
}

/// A Database-Connection Configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Connection {
	/// Host-Name, IP-Address or FileName to use as the database
	pub host: String,

	/// Port to connect to; default is postgres 5432
	#[serde(default = "default_postgres_port")]
	pub port: u16,

	/// SSL-Mode to connect to; Default is SSL-Disabled
	#[serde(default = "default_ssl_mode")]
	pub mode: SslMode,

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
fn default_postgres_port() -> u16 { 5432 }
fn default_ssl_mode() -> SslMode { SslMode::Disable }

impl Default for Connection {
	fn default() -> Self {
		Self {
			host: String::new(),
			port: default_postgres_port(),
			mode: default_ssl_mode(),
			root_cert: None,
			ssl_cert: None,
			ssl_key: None,
		}
	}
}

/// What kind of Database should be conencted
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum DbKind {
	PostgreSQL,
	MariaDB,
	SQLite,
}
impl Display for DbKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::PostgreSQL => f.write_str("PostgreSQL"),
			Self::MariaDB => f.write_str("MariaDB"),
			Self::SQLite => f.write_str("SQLite"),
		}
	}
}

/// SSL-Mode for the Database-Connection
/// This varries from Postgres to MySQL/MariaDB and SQLite
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
fn default_for_messages() -> String { String::from(".*") }

impl Default for DbTable {
	fn default() -> Self {
		Self {
			name: String::from("undefined"),
			for_messages: default_for_messages(),
			fields: Vec::new(),
		}
	}
}
