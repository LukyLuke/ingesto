use crate::config::{self, DbKind, DbTable};

use std::sync::Arc;

use anyhow::{Result, anyhow};
use futures::executor::block_on;
use shared::{self, types::DbValue};
use sqlx::{Connection, Database, MySql, MySqlPool, PgPool, Postgres, QueryBuilder, SqlitePool, Sqlite};
use tracing::{info, error};

static ERR_POOL_DIED: &str = "database pool is not alive any more";
static ERR_NO_CONN: &str = "unable to acquire a db connection";
static ERR_NOT_REACHABLE: &str = "database not reachable:";

pub trait DbAccess: Send + Sync {
	fn tables_config(&self) -> &[DbTable];
	fn insert(&self, table: &str, fields: &[(String, DbValue)]) -> Result<()>;
}

pub(crate) struct Db {
	pub(crate) kind: config::DbKind,
	pub(crate) tables: Vec<DbTable>,
	pub(crate) postgres: Option<PgPool>,
	pub(crate) mariadb: Option<MySqlPool>,
	pub(crate) sqlite: Option<SqlitePool>,
}
impl Drop for Db {
	fn drop(&mut self) {
		block_on(self.shutdown()).inspect_err(|e| error!(%e)).ok();
	}
}
impl Db {
	pub(crate) fn new(conf: Arc<config::DbConf>) -> Self {
		info!(message="initialize database connection", kind=%conf.database.kind);
		match conf.database.kind {
			DbKind::PostgreSQL => {
				Self {
					kind: DbKind::PostgreSQL,
					tables: conf.database.tables.clone(),
					postgres: Some(PgPool::connect_lazy_with(conf.database.get_postgres_options())),
					mariadb: None,
					sqlite: None,
				}
			},
			DbKind::MariaDB => {
				Self {
					kind: DbKind::MariaDB,
					tables: conf.database.tables.clone(),
					postgres: None,
					mariadb: Some(MySqlPool::connect_lazy_with(conf.database.get_mysql_options())),
					sqlite: None,
				}
			},
			DbKind::SQLite => {
				Self {
					kind: DbKind::SQLite,
					tables: conf.database.tables.clone(),
					postgres: None,
					mariadb: None,
					sqlite: Some(SqlitePool::connect_lazy_with(conf.database.get_sqlite_options())),
				}
			},
		}
	}

	pub(crate) async fn shutdown(&self) -> Result<()> {
		match self.kind {
			DbKind::PostgreSQL => {
				let pool = self.postgres.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				pool.close().await;
				Ok(())
			},
			DbKind::MariaDB => {
				let pool = self.mariadb.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				pool.close().await;
				Ok(())
			},
			DbKind::SQLite => {
				let pool = self.sqlite.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				pool.close().await;
				Ok(())
			},
		}
	}

	pub(crate) async fn alive(&self) -> Result<()> {
		match self.kind {
			DbKind::PostgreSQL => {
				let pool = self.postgres.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				let mut conn = pool.try_acquire().ok_or_else(|| anyhow!(ERR_NO_CONN))?;
				conn.ping().await.map_err(|e| anyhow!("{}: {:?}", ERR_NOT_REACHABLE, e))?;
				Ok(())
			},
			DbKind::MariaDB => {
				let pool = self.mariadb.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				let mut conn = pool.try_acquire().ok_or_else(|| anyhow!(ERR_NO_CONN))?;
				conn.ping().await.map_err(|e| anyhow!("{}: {:?}", ERR_NOT_REACHABLE, e))?;
				Ok(())
			},
			DbKind::SQLite => {
				let pool = self.sqlite.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				let mut conn = pool.try_acquire().ok_or_else(|| anyhow!(ERR_NO_CONN))?;
				conn.ping().await.map_err(|e| anyhow!("{}: {:?}", ERR_NOT_REACHABLE, e))?;
				Ok(())
			},
		}
	}

	fn build_insert_query<DB: Database>(table: &str, kind: &DbKind, fields: &[(String, DbValue)]) -> QueryBuilder::<DB> {
		let mut builder = QueryBuilder::<DB>::new(format!("INSERT INTO {} (", table));
		for (field, _) in fields {
			builder.push(Self::quoted_field_format(kind, field));
		}
		builder.push(") VALUES ");
		builder
	}

	fn quoted_field_format(kind: &DbKind, value: &str) -> String {
		let value = value.replace(['"', '`'], "");
		match kind {
			DbKind::PostgreSQL | DbKind::SQLite => format!("\"{}\"", value),
			DbKind::MariaDB => format!("`{}`", value),
		}
	}

	async fn execute_postgres(pool: &PgPool, mut builder: QueryBuilder<Postgres>, fields: &[(String, DbValue)]) -> Result<()> {
		builder.push("(");
		let mut values = builder.separated(",");
		for (_, value) in fields {
			match value {
				DbValue::Bool(v) => values.push_bind(*v),
				DbValue::I64(v) => values.push_bind(*v),
				DbValue::F64(v) => values.push_bind(*v),
				DbValue::String(v) => values.push_bind(v),
				DbValue::DateTimeUtc(v) => values.push_bind(v),
				DbValue::Bytes(v) => values.push_bind(v),
			};
		}
		builder.push(")");
		builder.build().execute(pool).await?;
		Ok(())
	}

	async fn execute_mariadb(pool: &MySqlPool, mut builder: QueryBuilder<MySql>, fields: &[(String, DbValue)]) -> Result<()> {
		builder.push("(");
		let mut values = builder.separated(",");
		for (_, value) in fields {
			match value {
				DbValue::Bool(v) => values.push_bind(*v),
				DbValue::I64(v) => values.push_bind(*v),
				DbValue::F64(v) => values.push_bind(*v),
				DbValue::String(v) => values.push_bind(v),
				DbValue::DateTimeUtc(v) => values.push_bind(v),
				DbValue::Bytes(v) => values.push_bind(v),
			};
		}
		builder.push(")");
		builder.build().execute(pool).await?;
		Ok(())
	}

	async fn execute_sqlite(pool: &SqlitePool, mut builder: QueryBuilder<Sqlite>, fields: &[(String, DbValue)]) -> Result<()> {
		builder.push("(");
		let mut values = builder.separated(",");
		for (_, value) in fields {
			match value {
				DbValue::Bool(v) => values.push_bind(*v),
				DbValue::I64(v) => values.push_bind(*v),
				DbValue::F64(v) => values.push_bind(*v),
				DbValue::String(v) => values.push_bind(v),
				DbValue::DateTimeUtc(v) => values.push_bind(v),
				DbValue::Bytes(v) => values.push_bind(v),
			};
		}
		builder.push(")");
		builder.build().execute(pool).await?;
		Ok(())
	}
}

impl DbAccess for Db {
	fn tables_config(&self) -> &[DbTable] {
		&self.tables
	}

	fn insert(&self, table: &str, fields: &[(String, DbValue)]) -> Result<()> {
		match self.kind {
			DbKind::PostgreSQL => {
				let pool = self.postgres.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				let builder: QueryBuilder<Postgres> = Self::build_insert_query(table, &self.kind, &fields);
				block_on(Self::execute_postgres(pool, builder, &fields))
			},
			DbKind::MariaDB => {
				let pool = self.mariadb.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				let builder: QueryBuilder<MySql> = Self::build_insert_query(table, &self.kind, &fields);
				block_on(Self::execute_mariadb(pool, builder, &fields))
			},
			DbKind::SQLite => {
				let pool = self.sqlite.as_ref().ok_or_else(|| anyhow!(ERR_POOL_DIED))?;
				let builder: QueryBuilder<Sqlite> = Self::build_insert_query(table, &self.kind, &fields);
				block_on(Self::execute_sqlite(pool, builder, &fields))
			},
		}
	}
}
