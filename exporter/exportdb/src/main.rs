pub mod config;
pub mod db;

use std::{sync::Arc, thread, time::Duration};

use anyhow::{Result, anyhow};
use futures::executor::block_on;
use shared::{self, init_logging, queue::MessageQueue, receiver::start_otel_listener, types::{DbField, DbValue}, usage};
use tracing::{debug, error, info};

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

fn start_exporter(conf: Arc<config::DbConf>, queue: Arc<MessageQueue<String>>) {
	thread::spawn(move || {
		let max_time = Duration::from_secs_f32(conf.queue.max_seconds as f32);
		let max_messages = conf.queue.max_messages;
		let db = Arc::new(db::Db::new(conf.clone()));

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

fn process_queue<DB: db::DbAccess + 'static>(queue: Arc<MessageQueue<String>>, max_time: Duration, _max_messages: u16, db: Arc<DB>) -> Result<()> {
	let msg = match queue.pull(max_time) {
		Some(m) => m.to_owned().trim().to_string(),
		None => {
			info!(message="queue empty", waited=%max_time.as_secs_f32());
			return Ok(());
		}
	};
	debug!(message="processing message", message=%msg);

	for table in db.tables() {
		// TODO: Check
		if table.for_messages != "" {
			let json: serde_json::Value = serde_json::from_str(&msg).unwrap_or_default();
			println!("{:?}", json);
			let mut fields = Vec::new();
			for field in &table.fields {
				match field {
					DbField::String { name, origin } => fields.push((String::from(name), DbValue::String( json.get(origin).unwrap_or_default().as_str().unwrap_or_default().to_string() ))),
					DbField::Float { name, origin } => fields.push((String::from(name), DbValue::F64( json.get(origin).unwrap_or_default().as_f64().unwrap_or_default() ))),
					DbField::Bool { name, origin } => fields.push((String::from(name), DbValue::Bool( json.get(origin).unwrap_or_default().as_bool().unwrap_or_default() ))),
					DbField::Int { name, origin } => fields.push((String::from(name), DbValue::I64( json.get(origin).unwrap_or_default().as_i64().unwrap_or_default() ))),
					_ => {}
				}
			}
			return db.insert(&table.name, &fields);
		}
	}
	Err(anyhow!("no suitable table configuration found"))
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::db::DbAccess;

	struct DbTest {
		pub tables: Vec<config::DbTable>,
		pub expected: Vec<(String, DbValue)>,
	}
	impl DbAccess for DbTest {
		fn tables(&self) -> &[config::DbTable] {
			return &self.tables;
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

	#[test]
	fn test_process_queue_no_messages() {
		let queue = Arc::new(MessageQueue::<String>::new());
		let testdb = Arc::new(DbTest{
			tables: Vec::new(),
			expected: Vec::new(),
		});

		let res = process_queue(queue, Duration::from_millis(1), 0, testdb);
		assert!(res.is_ok());
	}

	#[test]
	fn test_process_queue_no_tables() {
		let queue = Arc::new(MessageQueue::<String>::new());
		queue.push(String::from("message"));
		let testdb = Arc::new(DbTest{
			tables: Vec::new(),
			expected: Vec::new(),
		});

		let res = process_queue(queue, Duration::from_millis(1), 0, testdb);
		assert_eq!(res.unwrap_err().to_string(), "no suitable table configuration found");
	}

	#[test]
	fn test_process_queue_message() {
		let queue = Arc::new(MessageQueue::<String>::new());
		queue.push(String::from("{\"foo1\":\"bar1\",\"foo2\":\"bar2\",\"int1\":666,\"float1\":666.666,\"bool1\":true}"));

		let testdb = Arc::new(DbTest{
			tables: vec!(
				config::DbTable{
					name: "test".to_string(),
					for_messages: ".*".to_string(),
					fields: vec!(
						shared::types::DbField::Int    { name: "db_int1".to_string(),   origin: "int1".to_string() },
						shared::types::DbField::Bool   { name: "db_bool1".to_string(),  origin: "bool1".to_string() },
						shared::types::DbField::Float  { name: "db_float1".to_string(), origin: "float1".to_string() },
						shared::types::DbField::String { name: "db_foo1".to_string(),   origin: "foo1".to_string() },
						shared::types::DbField::String { name: "db_foo2".to_string(),   origin: "foo2".to_string() },
					),
				},
			),
			expected: vec!(
				(String::from("db_foo1"),   DbValue::String(String::from("bar1"))),
				(String::from("db_foo2"),   DbValue::String(String::from("bar2"))),
				(String::from("db_int1"),   DbValue::I64(666)),
				(String::from("db_float1"), DbValue::F64(666.666)),
				(String::from("db_bool1"),  DbValue::Bool(true)),
			),
		});

		let res = process_queue(queue, Duration::from_millis(1), 0, testdb);
		assert!(res.is_ok());
	}
}
