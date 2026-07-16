pub mod config;
pub mod db;

use std::{collections::HashMap, sync::{Arc, OnceLock}, thread, time::Duration};

use anyhow::{Result, anyhow};
use futures::executor::block_on;
use regex::Regex;
use shared::{self, init_logging, queue::MessageQueue, receiver::start_otel_listener, types::{DbField, DbValue}, usage};
use tracing::{debug, error, info};

use crate::config::DbTable;

fn main() {
	init_logging();

	let conf_file = match usage() {
		Ok(file) => file,
		Err(e) => {
			error!(%e);
			return
		}
	};

	let r_conf: anyhow::Result<config::Config> = shared::load_config(conf_file);
	let conf = match r_conf {
		Ok(c) => Arc::new(c.config),
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	start_exporter(conf.clone(), queue.clone());

	let c = Arc::new(conf.listener.clone());
	match start_otel_listener(c, queue.clone()) {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

/// Starting DB-Exporter
///
/// # Arguments
///
/// * `conf` - Database configuration
/// * `queue` - Queue to fetch messages from
fn start_exporter(conf: Arc<config::DbConf>, queue: Arc<MessageQueue<String>>) {
	thread::spawn(move || {
		let max_time = Duration::from_secs_f32(conf.queue.max_seconds as f32);
		let max_messages = conf.queue.max_messages;
		let db = Arc::new(db::Db::new(conf.clone()));

		precompile_regex(&conf.database.tables);

		loop {
			// If the database is not reachable, pause for max_time
			if block_on(db.alive()).inspect_err(|e| error!(%e)).is_err() {
				thread::sleep(max_time);
				continue;
			}

			// Read a message from the qeue and insert
			if let Err(e) = process_queue(queue.clone(), max_time, max_messages, db.clone()) {
				error!(%e)
			}
		}
	});
}

/// Read out a message from the queue and insert it into the Database
///
/// # Arguments
///
/// * `queue` - The queue to read a message from
/// * `max_time` - Number of seconds to wait for a message in the queue
/// * `_max_messages` - not used yet
/// * `db` - The Database-implementation
///
/// # Returns
///
/// An empty Ok Result or an Error with a string identifying what went wrong
fn process_queue<DB: db::DbAccess + 'static>(queue: Arc<MessageQueue<String>>, max_time: Duration, _max_messages: u16, db: Arc<DB>) -> Result<()> {
	let msg = match queue.pull(max_time) {
		Some(m) => m.to_owned().trim().to_string(),
		None => {
			info!(message="queue empty", waited=%max_time.as_secs_f32());
			return Ok(());
		}
	};
	debug!(message="processing message", message=%msg);

	match find_matching_table_config(db.tables_config(), &msg) {
		Ok(table) => {
			let json: serde_json::Value = serde_json::from_str(&msg).unwrap_or_default();
			let mut fields = Vec::new();
			for field in &table.fields {
				match field {
					DbField::String { name, origin } => fields.push((
						String::from(name),
						DbValue::String( json.get(origin.as_ref().unwrap_or(name)).unwrap_or_default().as_str().unwrap_or_default().to_string() )
					)),
					DbField::Float { name, origin } => fields.push((
						String::from(name),
						DbValue::F64( json.get(origin.as_ref().unwrap_or(name)).unwrap_or_default().as_f64().unwrap_or_default() )
					)),
					DbField::Bool { name, origin } => fields.push((
						String::from(name),
						DbValue::Bool( json.get(origin.as_ref().unwrap_or(name)).unwrap_or_default().as_bool().unwrap_or_default() )
					)),
					DbField::Int { name, origin } => fields.push((
						String::from(name),
						DbValue::I64( json.get(origin.as_ref().unwrap_or(name)).unwrap_or_default().as_i64().unwrap_or_default() )
					)),
					_ => {}
				}
			}
			db.insert(&table.name, &fields)
		},
		Err(e) => Err(e),
	}
}

/// A static HashMap to cache Regular Expressions to match a message with a table config
static TABLES_REGEXES: OnceLock<HashMap<String, Regex>> = OnceLock::new();

/// Precompiles all Regular Expressions for the Message-Matching
///
/// The HashMap has the Regular-Expression String as key and the Regex-Impl as value.
///
/// # Arguments
///
/// * `tables` - List of all Table-Configurations
///
/// # Returns
///
/// The HashMap with all precompiled matches
fn precompile_regex(tables: &[DbTable]) -> &HashMap<String, Regex> {
	TABLES_REGEXES.get_or_init(|| {
		let mut rm = HashMap::new();
		for table in tables {
			match Regex::new(&table.for_messages) {
				Ok(re) => {
					info!(message="regex compile", regex=%table.for_messages);
					rm.insert(table.for_messages.to_owned(), re);
				},
				Err(e) => error!(message="regex compile", regex=%table.for_messages, error=%e),
			};
		}
		rm
	})
}

/// Finds a matching Table-Configuration for a given message
///
/// # Arguments
///
/// * `tables` - List of all Table Coonfigurations
/// * `msg` - Message to find a configuration for
///
/// # Returns
///
/// A matching Table Configuration or an Error
/// * If there is no Regular Expression which matches the emssage
/// * If the Regular Expressions where not yet cached via `precompile_regex()`
fn find_matching_table_config(tables: &[DbTable], msg: &str) -> Result<DbTable> {
	if let Some(reg) = TABLES_REGEXES.get() {
		if let Some(Some(table)) = reg.iter()
			.find(|(_, re)| re.is_match(msg))
			.map(|(for_message, _)| {
				tables.iter().find(|table| table.for_messages == *for_message)
			}) {
			return Ok(table.clone());
		}
		return Err(anyhow!("no suitable table configuration found"));
	}
	Err(anyhow!("tables regex need to be precompiled before accessed: precompile_regex(&[DbTable])"))
}


#[cfg(test)]
mod test {
	use super::*;
	use crate::{config::DbTable, db::DbAccess};

	struct DbTest {
		pub tables: Vec<DbTable>,
		pub expected: Vec<(String, DbValue)>,
	}
	impl DbAccess for DbTest {
		fn tables_config(&self) -> &[DbTable] {
			&self.tables
		}

		fn insert(&self, _table: &str, fields: &[(String, DbValue)]) -> anyhow::Result<()> {
			let diff: Vec<_> = fields.into_iter().filter(|item| !self.expected.contains(item)).collect();
			if diff.is_empty() {
				Ok(())
			} else {
				assert_eq!(fields, self.expected);
				Err(anyhow!("inserted fields are different to expected"))
			}
		}
	}
	impl DbTest {
		fn new(expected: Vec<(String, DbValue)>) -> Self {
			let tables = vec!(
				config::DbTable{
					name: "first".to_string(),
					for_messages: "\"match\":\"first\"".to_string(),
					fields: vec!(
						DbField::Int    { name: "db_int1".to_string(),   origin: Some("int1".to_string()) },
						DbField::Bool   { name: "db_bool1".to_string(),  origin: Some("bool1".to_string()) },
						DbField::Float  { name: "db_float1".to_string(), origin: Some("float1".to_string()) },
						DbField::String { name: "db_foo1".to_string(),   origin: Some("foo1".to_string()) },
						DbField::String { name: "db_foo2".to_string(),   origin: Some("foo2".to_string()) },
					),
				},
				config::DbTable{
					name: "second".to_string(),
					for_messages: "\"match\":\"second\"".to_string(),
					fields: vec!(
						DbField::String { name: "db_foo1".to_string(),   origin: Some("foo1".to_string()) },
						DbField::String { name: "db_foo2".to_string(),   origin: Some("foo2".to_string()) },
					),
				},
			);
			precompile_regex(&tables);

			Self {
				expected,
				tables,
			}
		}
	}

	#[test]
	fn test_process_queue_no_messages() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let testdb = Arc::new(DbTest::new(Vec::new()));

		let res = process_queue(queue, Duration::from_millis(1), 0, testdb);
		assert!(res.is_ok());
	}

	#[test]
	fn test_process_queue_message() {
		let queue = Arc::new(MessageQueue::<String>::new());
		queue.push(String::from("{\"match\":\"first\",\"foo1\":\"bar1\",\"foo2\":\"bar2\",\"int1\":666,\"float1\":666.666,\"bool1\":true}"));
		let testdb = Arc::new(DbTest::new(
			vec!(
				(String::from("db_foo1"),   DbValue::String(String::from("bar1"))),
				(String::from("db_foo2"),   DbValue::String(String::from("bar2"))),
				(String::from("db_int1"),   DbValue::I64(666)),
				(String::from("db_float1"), DbValue::F64(666.666)),
				(String::from("db_bool1"),  DbValue::Bool(true)),
			)
		));

		let res = process_queue(queue, Duration::from_millis(1), 0, testdb);
		assert!(res.is_ok());
	}

	#[test]
	fn test_process_queue_no_match() {
		let queue = Arc::new(MessageQueue::<String>::new());
		queue.push(String::from("{\"foo1\":\"bar1\",\"foo2\":\"bar2\"}"));
		let testdb = Arc::new(DbTest::new(
			vec!(
				(String::from("db_foo1"),   DbValue::String(String::from("bar1"))),
				(String::from("db_foo2"),   DbValue::String(String::from("bar2"))),
				(String::from("db_int1"),   DbValue::I64(666)),
				(String::from("db_float1"), DbValue::F64(666.666)),
				(String::from("db_bool1"),  DbValue::Bool(true)),
			)
		));

		let res = process_queue(queue, Duration::from_millis(1), 0, testdb);
		assert_eq!(res.unwrap_err().to_string(), "no suitable table configuration found");
	}
}
