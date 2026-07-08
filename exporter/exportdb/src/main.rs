pub mod config;
pub mod db;

use std::{sync::Arc, thread, time::Duration};

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
	let max_time = Duration::from_secs_f32(conf.queue.max_seconds as f32);
	let _max_messages = conf.queue.max_messages;
	let db_name = String::from(&conf.database.database);

	thread::spawn(move || {
		let db = db::Db::new(conf.clone());

		loop {
			// If the database is not reachable, pause for max_time
			if block_on(db.alive()).inspect_err(|e| error!(%e)).is_err() {
				thread::sleep(max_time);
				continue;
			}

			let msg = match queue.pull(max_time) {
				Some(m) => m.to_owned().trim().to_string(),
				None => {
					info!(message="queue empty", waited=%max_time.as_secs_f32());
					continue;
				}
			};
			debug!(message="processing message", message=%msg);

			let mut fields = Vec::new();
			for table in &conf.database.tables {
				if table.for_messages != "" {
					for field in &table.fields {
						match field {
							DbField::String { name, origin: _ } => fields.push((String::from(name), DbValue::String(String::from("msg")))),
							DbField::Float { name, origin: _ } => fields.push((String::from(name), DbValue::F64(0.0))),
							DbField::Bool { name, origin: _ } => fields.push((String::from(name), DbValue::Bool(true))),
							DbField::Int { name, origin: _ } => fields.push((String::from(name), DbValue::I64(0))),
							_ => {}
						}
					}
					break;
				}
			}

			block_on(db.insert(&db_name, &fields)).ok();
		}
	});
}
