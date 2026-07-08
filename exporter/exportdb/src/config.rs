use std::fmt::Display;

use serde::{Deserialize, Serialize};
use shared::types::{OtelReceiver, Queue};
use sqlx::{mysql::MySqlConnectOptions, postgres::PgConnectOptions, sqlite::SqliteConnectOptions};

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
	pub database: String,

	#[serde(default)]
	pub auth: Option<Authentication>,

	#[serde(default)]
	pub connection: Connection,
}
impl Default for Database {
	fn default() -> Self {
		Self {
			database: String::new(),
			auth: None,
			connection: Connection::default(),
		}
	}
}
impl Database {
	/// Returns postgres connection options
	pub fn get_postgres_options(&self) -> PgConnectOptions {
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

	pub fn get_mysql_options(&self) -> MySqlConnectOptions {
		MySqlConnectOptions::new()
	}

	pub fn get_sqlite_options(&self) -> SqliteConnectOptions {
		SqliteConnectOptions::new()
	}
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	Passfile,
	Simple { user: String, pass: String },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Connection {
	pub kind: DbKind,

	pub host: String,

	#[serde(default = "default_postgres_port")]
	pub port: u16,

	#[serde(default = "default_ssl_mode")]
	pub mode: SslMode,

	#[serde(default)]
	pub root_cert: Option<String>,

	#[serde(default)]
	pub ssl_cert: Option<String>,

	#[serde(default)]
	pub ssl_key: Option<String>,
}
fn default_postgres_port() -> u16 { 5432 }
fn default_ssl_mode() -> SslMode { SslMode::Disable }
impl Default for Connection {
	fn default() -> Self {
		Self {
			kind: DbKind::PostgreSQL,
			host: String::new(),
			port: default_postgres_port(),
			mode: default_ssl_mode(),
			root_cert: None,
			ssl_cert: None,
			ssl_key: None,
		}
	}
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum SslMode {
	Disable,
	Allow,
	Prefer,
	Require,
	VerifyCa,
	VerifyFull,
}
