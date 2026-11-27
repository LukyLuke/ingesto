pub mod config;
pub mod types;

use core::time;
use std::{str::FromStr, sync::Arc, thread};

use chrono::{Duration, Utc};
use cron::Schedule;
use reqwest::blocking::{RequestBuilder, Response};
use shared::{
	self, init_logging, parser::MessageParser, queue::MessageQueue, reqwest::call_api, types::Endpoint, types::Authentication, usage
};
use tracing::{debug, error, info};

fn main() {
	init_logging();

	let conf_file = match usage() {
		Ok(file) => file,
		Err(e) => {
			error!(%e);
			return;
		}
	};

	let r_conf: anyhow::Result<types::Config> = shared::load_config(conf_file);
	let conf = match r_conf {
		Ok(c) => Arc::new(c.config),
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	let parser = Arc::new(MessageParser::<String>::new(
		queue.clone(),
		conf.queue.clone(),
		conf.parser.clone(),
	));
	parser.run();

	let endpoint = Arc::new(conf.api.clone());
	let res = run_scheduler(endpoint, conf.timer.as_str(), queue.clone());
	if let Err(e) = res {
		error!("{:#?}", e);
	}
}

fn run_scheduler(
	conf: Arc<Vec<Endpoint>>,
	cron_expr: &str,
	queue: Arc<shared::queue::MessageQueue<String>>,
) -> anyhow::Result<()> {
	let schedule = Schedule::from_str(cron_expr)?;
	info!(message="scheduler started", cron=%cron_expr);

	// Prepare Body and Url Template-Cache
	for c in conf.iter() {
		let u = c.uri.to_owned();
		let b = c.body.as_deref().unwrap_or_default().to_owned();
		shared::template::template_string_parse(&u, &u);
		shared::template::template_string_parse(&b, &b);
	}

	loop {
		// Run if the next schedule would be in the past already on the next run
		let now = Utc::now();
		if let Some(upcoming) = schedule.upcoming(Utc).next()
			&& upcoming < (now + Duration::seconds(1))
		{
			let queue_t = Arc::clone(&queue);
			let conf_t = Arc::clone(&conf);

			thread::spawn(move || {
				for c in conf_t.iter() {
					debug!(message="polling api", method=%c.method, api=%c.name, auth=%c.auth.as_ref().unwrap_or(&Authentication::None));

					// call_api_internal(req) function sends out the real request (dep-injection for test)
					// queue_message is for adding the received data to the queue (dep-injection for test)
					call_api(
						&Arc::new(c),
						None,
						queue_t.clone(),
						call_api_internal,
						queue_message_internal,
					)
					.ok();
				}
			});
		}

		// Check Cron-Scheduler every second
		thread::sleep(time::Duration::from_secs(1));
	}
}

fn call_api_internal(req: RequestBuilder) -> Result<Response, reqwest::Error> {
	req.send()
}

fn queue_message_internal(data: String, queue: Arc<shared::queue::MessageQueue<String>>) {
	queue.push(data);
}

