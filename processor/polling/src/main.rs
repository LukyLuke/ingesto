pub mod types;

use anyhow::{Context, Result};
use reqwest::blocking::{RequestBuilder, Response};
use serde_json::Value;
use shared::{
	self, init_logging,
	parser::MessageParser,
	queue::MessageQueue,
	receiver::start_otel_listener,
	reqwest::call_api,
	types::{Authentication, Endpoint},
	usage,
};
use std::{sync::Arc, thread, time::Duration};
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

	// The input-queue holds the received messages, the output-queue the answers of the APIs
	let input = Arc::new(MessageQueue::<String>::new());
	let output = Arc::new(MessageQueue::<String>::new());

	// Forwards everything from the output-queue to the OpenTelemetry Log-Receiver
	let parser = Arc::new(MessageParser::<String>::new(
		output.clone(),
		conf.queue.clone(),
		conf.parser.clone(),
	));
	parser.run();

	let max_time = Duration::from_secs(u64::from(conf.queue.max_seconds.max(1)));
	let endpoint = Arc::new(conf.api.clone());
	start_processor(endpoint, max_time, input.clone(), output.clone()).ok();

	let c = Arc::new(conf.listener.clone());
	let res = start_otel_listener(c, input.clone());
	if let Err(e) = res {
		error!("{:#?}", e);
	}
}

/// Starting the Processor
///
/// Prepares the caches for all Endpoints and starts a thread which reads messages from the
/// input queue. Each message is processed in its own thread, which sends out the web requests
/// and enqueues the answers into the output queue.
///
/// # Arguments
///
/// * `conf` - All Endpoint-Configurations
/// * `max_time` - Time to wait for a message in the input queue
/// * `input` - Queue to fetch received messages from
/// * `output` - Queue where the answers of the web requests are enqueued
///
/// # Returns
///
/// An empty Ok Result or an Error identifying what went wrong
fn start_processor(
	conf: Arc<Vec<Endpoint>>,
	max_time: Duration,
	input: Arc<MessageQueue<String>>,
	output: Arc<MessageQueue<String>>,
) -> anyhow::Result<()> {
	// Prepare Body and Url Template-Cache
	for c in conf.iter() {
		let u = c.uri.to_owned();
		let b = c.body.as_deref().unwrap_or_default().to_owned();
		shared::template::template_string_parse(&u, &u);
		shared::template::template_string_parse(&b, &b);
	}

	thread::spawn(move || -> anyhow::Result<()> {
		loop {
			let msg = match input.pull(max_time) {
				Some(m) => m.trim().to_string(),
				None => {
					info!(message="queue empty", waited=%max_time.as_secs_f32());
					continue;
				}
			};

			debug!(message="processing message", msg=%msg);
			let json: Arc<Value> =
				Arc::new(serde_json::from_str(&msg).context("message is not a valid json")?);
			let conf_t = Arc::clone(&conf);
			let output_t = Arc::clone(&output);
			thread::spawn(move || {
				for endpoint in conf_t.iter() {
					debug!(message="polling api", method=%endpoint.method, api=%endpoint.name, auth=%endpoint.auth.as_ref().unwrap_or(&Authentication::None));
					call_api(
						endpoint,
						Some(json.clone()),
						output_t.clone(),
						call_api_internal,
						queue_message_internal,
					)
					.ok();
				}
			});
		}
	});

	Ok(())
}

fn call_api_internal(req: RequestBuilder) -> Result<Response, reqwest::Error> {
	req.send()
}

fn queue_message_internal(data: String, queue: Arc<MessageQueue<String>>) {
	queue.push(data);
}
