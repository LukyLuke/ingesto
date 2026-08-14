use std::fmt::Display;

use shared::{secrets_string};
use sqlx::{mysql::MySqlConnectOptions, postgres::PgConnectOptions, sqlite::SqliteConnectOptions};

use crate::types::{self, Authentication, Database, DbTable, DbKind, SslMode, Connection};

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
					Authentication::Simple { user, pass } => {
						opt.username(secrets_string(user).unwrap_or_default().as_ref())
							.password(secrets_string(pass).unwrap_or_default().as_ref())
					},
					_ => opt,
				}
			},
			None => opt,
		};

		// SSL Related
		opt = match &self.connection.ssl_mode {
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

impl Default for Connection {
	fn default() -> Self {
		Self {
			host: String::new(),
			port: types::default_postgres_port(),
			ssl_mode: types::default_ssl_mode(),
			root_cert: None,
			ssl_cert: None,
			ssl_key: None,
		}
	}
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

impl Default for DbTable {
	fn default() -> Self {
		Self {
			name: String::from("undefined"),
			for_messages: types::default_for_messages(),
			fields: Vec::new(),
		}
	}
}

