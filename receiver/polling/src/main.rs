pub mod config;

use core::time;
use std::{
	str::FromStr, sync::Arc, thread
};

use chrono::{Duration, Utc};
use cron::Schedule;
use once_cell::sync::Lazy;
use reqwest::{StatusCode, blocking::{Response, RequestBuilder}};
use shared::{self, init_logging, parser::MessageParser, queue::MessageQueue, usage, secrets_string};
use serde_json::{Value, json};
use tracing::{debug, error, info};

use crate::{config::{Authentication, Method}};

static TEMPLATE_URI_KEY: Lazy<String> = Lazy::new(|| String::from("uri"));
static TEMPLATE_BODY_KEY: Lazy<String> = Lazy::new(|| String::from("body"));

static MAX_PAGING_REQUESTS: u16 = 1024;

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
	let parser = Arc::new(MessageParser::<String>::new(queue.clone(), conf.queue.clone(), conf.parser.clone()));
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

	// Prepare Body and Url Template-Cache
	shared::template::template_string_parse(&TEMPLATE_URI_KEY,  &conf.uri);
	shared::template::template_string_parse(&TEMPLATE_BODY_KEY, &String::from(conf.body.as_deref().unwrap_or_default()));

	loop {
		// Run if the next schedule would be in the past already on the next run
		let now = Utc::now();
		if let Some(upcoming) = schedule.upcoming(Utc).next() {
			if upcoming < (now + Duration::seconds(1)) {
				debug!(message="polling api", method=%conf.method, api=%conf.uri, auth=%conf.auth.as_ref().unwrap_or(&Authentication::None));

				let conf_t = Arc::clone(&conf);
				let queue_t = Arc::clone(&queue);
				thread::spawn(move || {
					// call_api_internal(req) function sends out the real request (dep-injection for test)
					// queue_message is for adding the received data to the queue (dep-injection for test)
					let _ = call_api(conf_t, queue_t, call_api_internal, queue_message_internal);
				});
			}
		}

		// Check Cron-Scheduler every second
		thread::sleep(time::Duration::from_secs(1));
	}
}

fn call_api(conf: Arc<config::Endpoint>, queue: Arc<shared::queue::MessageQueue<String>>, send_reqwest: impl Fn(RequestBuilder) -> Result<Response, reqwest::Error>, queue_message: impl Fn(String, Arc<shared::queue::MessageQueue<String>>)) -> anyhow::Result<()> {
	let mut response = Arc::new( json!({}) );
	let mut paging = true;
	let mut pages = 0;

	while paging {
		// Parse URI and Body
		let mut uri = shared::template::template_string(&TEMPLATE_URI_KEY, Arc::clone(&response));
		let send_body = shared::template::template_string(&TEMPLATE_BODY_KEY, Arc::clone(&response));

		// Append Paging if available
		if !conf.paging.param.name.is_empty() {
			let page_val = shared::template::template_string(&conf.paging.param.value, Arc::clone(&response));
			let sep = if uri.find('?').is_some() { "&" } else { "?" };
			uri = format!("{}{}{}={}", uri, sep, conf.paging.param.name.as_str(), page_val.as_str());
		}
		debug!(message="calling api", uri=%uri);

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
			Some(Authentication::Header(param)) => req.header(&param.name, shared::template::template_string(&secrets_string(&param.value).unwrap_or_default(), Arc::clone(&response))),
			Some(Authentication::Basic { user, pass }) => req.basic_auth(secrets_string(user).unwrap_or_default(), secrets_string(pass).ok()),
			Some(Authentication::Bearer(token)) => {
				if token.starts_with("Bearer") {
					req.bearer_auth(secrets_string(token.get(7..).unwrap_or_default()).unwrap_or_default())
				} else {
					req.bearer_auth(secrets_string(token).unwrap_or_default())
				}
			},
			_ => req,
		};

		// Additional Headers
		req = req.header("User-Agent", "ingesto-polling/1.0");
		for header in &conf.header {
			req = req.header(header.name.to_string(), shared::template::template_string(&header.value, Arc::clone(&response)));
		}

		let resp = send_reqwest(req)?;
		let status = resp.status();
		let body = resp.text().unwrap_or_default();
		if status != StatusCode::OK {
			error!(message="calling api", status=%status.as_u16(), error=body.to_owned());
		}

		debug!(message="data received", data=body.to_owned());
		queue_message(body.to_owned(), queue.clone());

		// Check for paging
		pages += 1;
		debug!(message="paging", conf=%conf.paging, pages=pages, max_pages=MAX_PAGING_REQUESTS);
		if conf.paging.until.is_none() || pages >= conf.paging.max_pages || pages >= MAX_PAGING_REQUESTS {
			break;
		}

		paging = conf.paging.until.as_ref().is_some_and(|p| !p.check(status.as_u16(), body.to_owned()));
		debug!(message="apply paging", apply=paging);
		if paging {
			match serde_json::from_str(body.to_owned().as_str()).unwrap_or_default() {
				Value::Null => break,
				j => response = Arc::<Value>::new(j)
			}
		}
	}

	Ok(())
}

fn call_api_internal(req: RequestBuilder) -> Result<Response, reqwest::Error> {
	req.send()
}

fn queue_message_internal(data: String, queue: Arc<shared::queue::MessageQueue<String>>) {
	queue.push(data);
}

#[cfg(test)]
pub mod test {
	use super::*;

	use std::time::Duration;

	#[test]
	fn test_call_api() {
		let conf = Arc::new(config::Endpoint{
			uri: String::from("http://127.0.0.1/polling/?cursor={{ $response/paging/cursor }}"),
			body: None,
			method: Method::GET,
			auth: None,
			header: Vec::new(),
			paging: config::PagingReguest {
				param: config::Param { name: String::from("page"), value: String::from("{{ $response/paging/page }}")},
				until: Some(config::PagingRequestUntil::Empty),
				timeout: 100,
				max_pages: 2,
			}
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		shared::template::template_string_parse(&TEMPLATE_URI_KEY,  &conf.uri);
		shared::template::template_string_parse(&TEMPLATE_BODY_KEY, &String::from(conf.body.as_deref().unwrap_or_default()));

		let res = call_api(conf.clone(), queue.clone(), call_api_internal, queue_message_internal);

		// Check for two responses (paging requests)
		assert_eq!(res.is_ok(), true);
		assert_eq!(queue.size(), 2);

		let r1 = queue.pull(Duration::from_secs_f32(1.0)).unwrap_or_default();
		let j1: Value = serde_json::from_str(r1.to_owned().as_str()).unwrap_or_default();
		let r2 = queue.pull(Duration::from_secs_f32(1.0)).unwrap_or_default();
		let j2: Value = serde_json::from_str(r2.to_owned().as_str()).unwrap_or_default();

		// Check the URLs from the two requests (paging and cursor replaced correctly)
		assert_eq!(j1["uri"], "http://127.0.0.1/polling/?cursor=&page=");
		assert_eq!(j2["uri"], "http://127.0.0.1/polling/?cursor=testing&page=1");
	}

	#[test]
	fn test_call_api_nojson() {
		let conf = Arc::new(config::Endpoint{
			uri: String::from("http://127.0.0.1/polling/?cursor={{ $response/paging/cursor }}"),
			body: None,
			method: Method::GET,
			auth: None,
			header: Vec::new(),
			paging: config::PagingReguest {
				param: config::Param { name: String::from("page"), value: String::from("{{ $response/paging/page }}")},
				until: Some(config::PagingRequestUntil::Empty),
				timeout: 100,
				max_pages: 2,
			}
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		shared::template::template_string_parse(&TEMPLATE_URI_KEY,  &conf.uri);
		shared::template::template_string_parse(&TEMPLATE_BODY_KEY, &String::from(conf.body.as_deref().unwrap_or_default()));

		let res = call_api(conf.clone(), queue.clone(), call_api_internal_nojson, queue_message_internal);

		// Check for one response (no paging requests possible due to no json)
		assert_eq!(res.is_ok(), true);
		assert_eq!(queue.size(), 1);

		// Check for no error in case of no json string
		let r1 = queue.pull(Duration::from_secs_f32(1.0)).unwrap_or_default();
		assert_eq!(r1, "http://127.0.0.1/polling/?cursor=&page=");
	}


	fn call_api_internal(reqb: RequestBuilder) -> Result<Response, reqwest::Error> {
		let resp = http::response::Response::new("Test Response");
		let (mut parts, _body) = resp.into_parts();

		let req = reqb.build().unwrap();
		let url = String::from(req.url().as_str());

		// Copy over all headers from the request to the response
		let headers = req.headers();
		headers.iter().for_each(|h| { parts.headers.insert(h.0, h.1.to_owned()); } );

		let body = json!({ "uri":url, "paging": { "page":1, "cursor":"testing" } });
		let resp = http::response::Response::from_parts(parts, body.to_string());
		Ok(Response::from(resp))
	}

	fn call_api_internal_nojson(reqb: RequestBuilder) -> Result<Response, reqwest::Error> {
		let resp = http::response::Response::new("Test Response");
		let (mut parts, _body) = resp.into_parts();

		let req = reqb.build().unwrap();
		let url = String::from(req.url().as_str());

		// Copy over all headers from the request to the response
		let headers = req.headers();
		headers.iter().for_each(|h| { parts.headers.insert(h.0, h.1.to_owned()); } );

		let resp = http::response::Response::from_parts(parts, url);
		Ok(Response::from(resp))
	}

	fn queue_message_internal(data: String, queue: Arc<shared::queue::MessageQueue<String>>) {
		queue.push(data);
	}

}

