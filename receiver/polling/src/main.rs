pub mod config;

use core::time;
use std::{
	str::FromStr, sync::Arc, thread
};

use chrono::{Duration, Utc};
use cron::Schedule;
use reqwest::StatusCode;
use shared::{self, init_logging, parser::MessageParser, queue::MessageQueue, usage};
use serde_json::{Value, json};
use tracing::{debug, error, info};

use crate::config::{Authentication, Method, Param};

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
		Ok(c) => Arc::new(c.polling),
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	let parser = Arc::new(MessageParser::<String>::new(queue.clone(), conf.queue, conf.parser.clone()));
	parser.run();

	let endpoint = Arc::new(conf.api.clone());
	match run_scheduler(endpoint, conf.timer.as_str(), queue.clone()) {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

fn run_scheduler(conf: Arc<config::Endpoint>, cron_expr: &str, queue: Arc<shared::queue::MessageQueue<String>>) -> anyhow::Result<()> {
	let schedule = Schedule::from_str(cron_expr)?;
	info!(message="scheduler started", cron=%cron_expr);

	loop {
		// Run if the next schedule would be in the past already on the next run
		let now = Utc::now();
		if let Some(upcoming) = schedule.upcoming(Utc).next() {
			if upcoming < (now + Duration::seconds(1)) {
				debug!(message="polling api", method=%conf.method, api=%conf.uri, auth=%conf.auth.as_ref().unwrap_or(&Authentication::None));

				let conf_t = Arc::clone(&conf);
				let queue_t = Arc::clone(&queue);
				thread::spawn(move || {
					let _ = call_api(conf_t, queue_t);
				});
			}
		}

		// Check Cron-Scheduler every second
		thread::sleep(time::Duration::from_secs(1));
	}
}

fn call_api(conf: Arc<config::Endpoint>, queue: Arc<shared::queue::MessageQueue<String>>) -> anyhow::Result<()> {
	let response = Arc::new(
		json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"bar"}, {"foo":"bar"} ] })
	);


	// Parse URI and Body
	let mut uri = String::from(&conf.uri);
	let mut send_body = String::from(conf.body.as_deref().unwrap_or_default());
	replace_params(&conf.params, &mut uri, Arc::clone(&response));
	replace_params(&conf.params, &mut send_body, Arc::clone(&response));

	// Create the Request
	let client = reqwest::blocking::Client::new();
	let mut req = match conf.method {
		Method::GET => client.get(uri),
		Method::POST => client.post(uri).body(send_body),
		Method::HEAD => client.head(uri),
		_ => client.get(uri),
	};

	// Authentication
	req = match &conf.auth {
		Some(Authentication::Header(param)) => req.header(&param.name, parse_param_value(&param.value, Arc::clone(&response))),
		Some(Authentication::Basic { user, pass }) => req.basic_auth(user, Some(pass)),
		Some(Authentication::Bearer(token)) => {
			if token.starts_with("Bearer") {
				req.bearer_auth(token.get(7..).unwrap_or_default())
			} else {
				req.bearer_auth(token)
			}
		},
		_ => req,
	};

	// Additional Headers
	req = req.header("User-Agent", "ingesto-polling/1.0");
	for header in &conf.header {
		req = req.header(header.name.to_string(), parse_param_value(&header.value, Arc::clone(&response)));
	}

	let resp = req.send()?;
	let status = resp.status();
	let body = resp.text().unwrap_or_default();
	if status != StatusCode::OK {
		error!(message="calling api", status=%status.as_u16(), error=body.to_owned());
	}

	debug!(message="data received", data=body.to_owned());
	queue.push(body.to_owned());

	Ok(())
}

fn replace_params(params: &[Param], value: &mut String, response: Arc<Value>) {
	for param in params {
		let repl = parse_param_value(&param.value, response.clone());
		*value = value.replace(&param.name, &repl);
	};
}

fn parse_param_value(param: &String, response: Arc<Value>) -> String {
	// TODO
	String::from(param)
}
