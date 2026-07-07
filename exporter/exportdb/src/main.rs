pub mod config;

use std::sync::Arc;

use shared::{self, init_logging, queue::MessageQueue, usage, receiver::start_otel_listener};
use tracing::{error, info};

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
		Ok(c) => Arc::new(c.database),
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	let c = Arc::new(conf.listener.clone());
	match start_otel_listener(c, queue.clone()) {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}
