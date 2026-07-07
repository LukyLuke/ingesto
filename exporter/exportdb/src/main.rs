pub mod config;

use std::{sync::Arc, thread, time::Duration};

use shared::{self, init_logging, queue::MessageQueue, usage, receiver::start_otel_listener};
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
	start_exporter(&conf, queue.clone());

	let c = Arc::new(conf.listener.clone());
	match start_otel_listener(c, queue.clone()) {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

fn start_exporter(conf: &config::DbConf, queue: Arc<MessageQueue<String>>) {
	let max_time = Duration::from_secs_f32(conf.queue.max_seconds as f32);
	let _max_messages = conf.queue.max_messages;

	thread::spawn(move || {
		loop {
			let msg = match queue.pull(max_time) {
				Some(m) => m.to_owned().trim().to_string(),
				None => {
					info!(message="queue empty", waited=%max_time.as_secs_f32());
					continue;
				}
			};
			debug!(message="processing message", message=%msg);

		}
	});
}
