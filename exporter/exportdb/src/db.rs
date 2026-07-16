use crate::config::{self, DbKind, DbTable};

use std::sync::Arc;

use anyhow::{Result, anyhow};
use futures::executor::block_on;
use shared::{self, types::DbValue};
use sqlx::{Connection, Database, MySql, MySqlPool, PgPool, Postgres, QueryBuilder, Sqlite, SqlitePool};
use tracing::{info, error};

static ERR_NO_CONN: &str = "unable to acquire a db connection";
static ERR_NOT_REACHABLE: &str = "database not reachable:";

/// Trait to be able to have different implementations (for tests mainly)
pub trait DbAccess: Send + Sync {
	fn tables_config(&self) -> &[DbTable];
	fn insert(&self, table: &str, fields: &[(String, DbValue)]) -> Result<()>;
}

/// Identifies a Database Backend with a conneciton pool to it
enum DbBackend {
	Postgres(PgPool),
	MadiaDb(MySqlPool),
	SqLite(SqlitePool),
}

/// The main Database conenction and data handling
pub(crate) struct Db {
	/// List of table configuration
	tables: Vec<DbTable>,

	/// The Database Backend
	db: DbBackend,
}

/// The Drop-Trait is for shuttingDown/Closing the DB connections gracefully
impl Drop for Db {
	fn drop(&mut self) {
		block_on(self.shutdown()).inspect_err(|e| error!(%e)).ok();
	}
}

impl Db {
	/// Create a new Database Instance
	///
	/// # Arguments
	///
	/// * `conf` - The Database Configuration
	///
	/// # Returns
	///
	/// A Db Instance
	pub fn new(conf: Arc<config::DbConf>) -> Self {
		info!(message="initialize database connection", kind=%conf.database.kind);
		match conf.database.kind {
			DbKind::PostgreSQL => {
				Self {
					tables: conf.database.tables.clone(),
					db: DbBackend::Postgres(PgPool::connect_lazy_with(conf.database.get_postgres_options())),
				}
			},
			DbKind::MariaDB => {
				Self {
					tables: conf.database.tables.clone(),
					db: DbBackend::MadiaDb(MySqlPool::connect_lazy_with(conf.database.get_mysql_options())),
				}
			},
			DbKind::SQLite => {
				Self {
					tables: conf.database.tables.clone(),
					db: DbBackend::SqLite(SqlitePool::connect_lazy_with(conf.database.get_sqlite_options())),
				}
			},
		}
	}

	/// Shutdown the pool which should close all open connections to the Database
	///
	/// # Results
	///
	/// Ok if the pool was shutdown, an Error otherwise
	pub(crate) async fn shutdown(&self) -> Result<()> {
		match &self.db {
			DbBackend::Postgres(pool) => pool.close().await,
			DbBackend::MadiaDb(pool) => pool.close().await,
			DbBackend::SqLite(pool) => pool.close().await,
		}
		Ok(())
	}

	/// Checks if a DB Connection (Pool) is still alive
	///
	/// # Results
	///
	/// Ok if the pool is still alive, otherwise an error
	pub async fn alive(&self) -> Result<()> {
		match &self.db {
			DbBackend::Postgres(pool) => {
				let mut conn = pool.acquire().await.map_err(|e| anyhow!("{}: {:?}", ERR_NO_CONN, e))?;
				conn.ping().await.map_err(|e| anyhow!("{}: {:?}", ERR_NOT_REACHABLE, e))
			},
			DbBackend::MadiaDb(pool) => {
				let mut conn = pool.acquire().await.map_err(|e| anyhow!("{}: {:?}", ERR_NO_CONN, e))?;
				conn.ping().await.map_err(|e| anyhow!("{}: {:?}", ERR_NOT_REACHABLE, e))
			},
			DbBackend::SqLite(pool) => {
				let mut conn = pool.acquire().await.map_err(|e| anyhow!("{}: {:?}", ERR_NO_CONN, e))?;
				conn.ping().await.map_err(|e| anyhow!("{}: {:?}", ERR_NOT_REACHABLE, e))
			},
		}
	}

	/// Builds the start of an INSERT-Query as a QueryBuilder
	///
	/// # Arguments
	///
	/// * `table` - Tablename insert values into
	/// * `kind` - Query-Flavor for which Database (for escaping the table and fields)
	/// * `fields` - List of fields to insert values into
	///
	/// # Returns
	///
	/// An INSERT-QueryBuilder with all fields set.
	/// The Query after this looks like 'INSERT INTO `table` (`fld1`,`fld2`,...) VALUES '
	fn build_insert_query<DB: Database>(table: &str, kind: &DbKind, fields: &[(String, DbValue)]) -> QueryBuilder::<DB> {
		let mut builder = QueryBuilder::<DB>::new(format!("INSERT INTO {} (", Self::quoted_field_format(kind, table)));
		let mut sep = builder.separated(",");
		for (field, _) in fields {
			sep.push(Self::quoted_field_format(kind, field));
		}
		builder.push(") VALUES ");
		builder
	}

	/// Quote a field to be added in an SQL-Query
	///
	/// TODO: Check if cache these values is better for for performance
	///
	/// # Arguments
	///
	/// * `kind` - The Database Type to quote the field for
	/// * `value` - The FieldName to quote
	///
	/// # Returns
	///
	/// The given field name quoted for the given Database
	fn quoted_field_format(kind: &DbKind, value: &str) -> String {
		let value = value.replace(['"', '`'], "");
		match kind {
			DbKind::PostgreSQL | DbKind::SQLite => format!("\"{}\"", value),
			DbKind::MariaDB => format!("`{}`", value),
		}
	}

	/// Finalizes an INSERT-Query for POSTGRESQL by enrich it with all values and funally runs it in the pool
	///
	/// # Arguments
	///
	/// * `pool` - The Database Pool to run the Query
	/// * `builder` - The already prepared INSERT-Query where only the VALUES have to be added to
	/// * `fields` - All FieldName-Value pairs to add
	///
	/// # Returns
	///
	/// An Ok if everything went well, otherwise an Error for the Database
	async fn execute_postgres(pool: &PgPool, mut builder: QueryBuilder<Postgres>, fields: &[(String, DbValue)]) -> Result<()> {
		Self::build_postgres_insert(&mut builder, fields);
		builder.build().execute(pool).await?;
		Ok(())
	}

	/// Add all fields and values to an POSTGRESQL Insert-Query
	///
	/// # Arguments
	///
	/// * `builder` - The already prepared INSERT-Query where only the VALUES have to be added to
	/// * `fields` - All FieldName-Value pairs to add
	fn build_postgres_insert(builder: &mut QueryBuilder<Postgres>, fields: &[(String, DbValue)]) {
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
	}

	/// Finalizes an INSERT-Query for MYSQL/MARIADB by enrich it with all values and funally runs it in the pool
	///
	/// # Arguments
	///
	/// * `pool` - The Database Pool to run the Query
	/// * `builder` - The already prepared INSERT-Query where only the VALUES have to be added to
	/// * `fields` - All FieldName-Value pairs to add
	///
	/// # Returns
	///
	/// An Ok if everything went well, otherwise an Error for the Database
	async fn execute_mariadb(pool: &MySqlPool, mut builder: QueryBuilder<MySql>, fields: &[(String, DbValue)]) -> Result<()> {
		Self::build_mariadb_insert(&mut builder, fields);
		builder.build().execute(pool).await?;
		Ok(())
	}

	/// Add all fields and values to an MYSQL/MARIADB Insert-Query
	///
	/// # Arguments
	///
	/// * `builder` - The already prepared INSERT-Query where only the VALUES have to be added to
	/// * `fields` - All FieldName-Value pairs to add
	fn build_mariadb_insert(builder: &mut QueryBuilder<MySql>, fields: &[(String, DbValue)]) {
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
	}

	/// Finalizes an INSERT-Query for SQLITE by enrich it with all values and funally runs it in the pool
	///
	/// # Arguments
	///
	/// * `pool` - The Database Pool to run the Query
	/// * `builder` - The already prepared INSERT-Query where only the VALUES have to be added to
	/// * `fields` - All FieldName-Value pairs to add
	///
	/// # Returns
	///
	/// An Ok if everything went well, otherwise an Error for the Database
	async fn execute_sqlite(pool: &SqlitePool, mut builder: QueryBuilder<Sqlite>, fields: &[(String, DbValue)]) -> Result<()> {
		Self::build_sqlite_insert(&mut builder, fields);
		builder.build().execute(pool).await?;
		Ok(())
	}

	/// Add all fields and values to an SQLITE Insert-Query
	///
	/// # Arguments
	///
	/// * `builder` - The already prepared INSERT-Query where only the VALUES have to be added to
	/// * `fields` - All FieldName-Value pairs to add
	fn build_sqlite_insert(builder: &mut QueryBuilder<Sqlite>, fields: &[(String, DbValue)]) {
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
	}
}

/// The DbAccess-Trait if for having different implementations of the direct DB-Connections
impl DbAccess for Db {
	/// Returns the Tables configuration
	///
	/// # Returns
	///
	/// List of all Tables which are configured
	fn tables_config(&self) -> &[DbTable] {
		&self.tables
	}

	/// Insert values into a DB Table
	///
	/// # Arguments
	///
	/// * `table` - Table to insert values into
	/// * `fields` - List of tuples identifying the Column-Name and Value
	///
	/// # Returns
	///
	/// Result if the insert was Ok or not
	fn insert(&self, table: &str, fields: &[(String, DbValue)]) -> Result<()> {
		match &self.db {
			DbBackend::Postgres(pool) => {
				let builder: QueryBuilder<Postgres> = Self::build_insert_query(table, &DbKind::PostgreSQL, &fields);
				block_on(Self::execute_postgres(pool, builder, &fields))
			},
			DbBackend::MadiaDb(pool) => {
				let builder: QueryBuilder<MySql> = Self::build_insert_query(table, &DbKind::PostgreSQL, &fields);
				block_on(Self::execute_mariadb(pool, builder, &fields))
			},
			DbBackend::SqLite(pool) => {
				let builder: QueryBuilder<Sqlite> = Self::build_insert_query(table, &DbKind::PostgreSQL, &fields);
				block_on(Self::execute_sqlite(pool, builder, &fields))
			},
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use sqlx::{Arguments, Execute};

	#[test]
	fn test_field_quote() {
		let postgres = Db::quoted_field_format(&DbKind::PostgreSQL, "field_name");
		let mariadb = Db::quoted_field_format(&DbKind::MariaDB, "field_name");
		let sqlite = Db::quoted_field_format(&DbKind::SQLite, "field_name");

		let replace = Db::quoted_field_format(&DbKind::SQLite, "field\"_`name");

		assert_eq!(sqlite, String::from("\"field_name\""));
		assert_eq!(mariadb, String::from("`field_name`"));
		assert_eq!(postgres, String::from("\"field_name\""));

		assert_eq!(replace, String::from("\"field_name\""));
	}

	#[test]
	fn test_build_insert_query() {
		let table = "table_name";
		let fields = vec!(
			("foo".to_string(), DbValue::String("foo foo".to_string())),
			("bar".to_string(), DbValue::String("bar bar".to_string())),
		);

		let postgres: QueryBuilder<Postgres> = Db::build_insert_query(table, &DbKind::PostgreSQL, &fields);
		let mariadb: QueryBuilder<MySql> = Db::build_insert_query(table, &DbKind::MariaDB, &fields);
		let sqlite: QueryBuilder<Sqlite> = Db::build_insert_query(table, &DbKind::SQLite, &fields);

		assert_eq!(postgres.into_string(), "INSERT INTO \"table_name\" (\"foo\",\"bar\") VALUES ");
		assert_eq!(mariadb.into_string(), "INSERT INTO `table_name` (`foo`,`bar`) VALUES ");
		assert_eq!(sqlite.into_string(), "INSERT INTO \"table_name\" (\"foo\",\"bar\") VALUES ");
	}

	#[test]
	fn test_build_postgres_insert() {
		let table = "table_name";
		let fields = vec!(
			("foo".to_string(), DbValue::String("foo foo".to_string())),
			("bar".to_string(), DbValue::String("bar bar".to_string())),
		);
		let mut builder: QueryBuilder<Postgres> = Db::build_insert_query(table, &DbKind::PostgreSQL, &fields);

		Db::build_postgres_insert(&mut builder, &fields);
		let mut query = builder.build();
		let args = query.take_arguments().unwrap().unwrap();

		// Test the SQL, Sql-Flavor (escaping)
		// Test only for the number of Arguments - not possible to fetch them
		assert_eq!(query.sql().as_str(), "INSERT INTO \"table_name\" (\"foo\",\"bar\") VALUES ($1,$2)");
		assert_eq!(args.len(), 2);
	}

	#[test]
	fn test_build_mariadb_insert() {
		let table = "table_name";
		let fields = vec!(
			("foo".to_string(), DbValue::String("foo foo".to_string())),
			("bar".to_string(), DbValue::String("bar bar".to_string())),
		);
		let mut builder: QueryBuilder<MySql> = Db::build_insert_query(table, &DbKind::MariaDB, &fields);

		Db::build_mariadb_insert(&mut builder, &fields);
		let mut query = builder.build();
		let args = query.take_arguments().unwrap().unwrap();

		// Test the SQL, Sql-Flavor (escaping)
		// Test only for the number of Arguments - not possible to fetch them
		assert_eq!(query.sql().as_str(), "INSERT INTO `table_name` (`foo`,`bar`) VALUES (?,?)");
		assert_eq!(args.len(), 2);
	}

	#[test]
	fn test_build_sqlite_insert() {
		let table = "table_name";
		let fields = vec!(
			("foo".to_string(), DbValue::String("foo foo".to_string())),
			("bar".to_string(), DbValue::String("bar bar".to_string())),
		);
		let mut builder: QueryBuilder<Sqlite> = Db::build_insert_query(table, &DbKind::SQLite, &fields);

		Db::build_sqlite_insert(&mut builder, &fields);
		let mut query = builder.build();
		let args = query.take_arguments().unwrap().unwrap();

		// Test the SQL, Sql-Flavor (escaping)
		// Test only for the number of Arguments - not possible to fetch them
		assert_eq!(query.sql().as_str(), "INSERT INTO \"table_name\" (\"foo\",\"bar\") VALUES (?,?)");
		assert_eq!(args.len(), 2);
	}
}
