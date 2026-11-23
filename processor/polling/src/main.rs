pub mod config;
pub mod types;

use std::{
	collections::HashMap,
	sync::{Arc, RwLock},
	thread,
	time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use reqwest::{
	StatusCode,
	blocking::{Client, RequestBuilder, Response},
};
use serde_json::Value;
use shared::{
	self, init_logging, parser::MessageParser, queue::MessageQueue, receiver::start_otel_listener,
	secrets_string, usage,
};
use tracing::{debug, error, info};

use crate::types::{Authentication, Endpoint, Method, PagingRequestUntil};

static MAX_PAGING_REQUESTS: u16 = 1024;

/// A static HashMap to cache the Regular Expressions to match a message with an Endpoint.
/// The Regular-Expression String is the key and the Regex-Impl the value.
static ENDPOINT_REGEXES: RwLock<Option<HashMap<String, Regex>>> = RwLock::new(None);

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
	start_processor(
		Arc::new(conf.api.clone()),
		max_time,
		input.clone(),
		output.clone(),
	);

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
fn start_processor(
	conf: Arc<Vec<Endpoint>>,
	max_time: Duration,
	input: Arc<MessageQueue<String>>,
	output: Arc<MessageQueue<String>>,
) {
	precompile(&conf);

	thread::spawn(move || {
		loop {
			let msg = match input.pull(max_time) {
				Some(m) => m.trim().to_string(),
				None => {
					info!(message="queue empty", waited=%max_time.as_secs_f32());
					continue;
				}
			};

			let conf_t = Arc::clone(&conf);
			let output_t = Arc::clone(&output);
			thread::spawn(move || {
				// call_api_internal(req) function sends out the real request (dep-injection for test)
				// queue_message_internal is for adding the received data to the queue (dep-injection for test)
				if let Err(e) = process_message(
					&msg,
					&conf_t,
					output_t,
					call_api_internal,
					queue_message_internal,
				) {
					error!("{:#}", e);
				}
			});
		}
	});
}

/// Prepares the Url and Body Template-Cache and the Regex-Cache of all Endpoints
///
/// # Arguments
///
/// * `endpoints` - All Endpoint-Configurations
fn precompile(endpoints: &[Endpoint]) {
	for c in endpoints {
		let u = c.uri.to_owned();
		let b = c.body.as_deref().unwrap_or_default().to_owned();
		shared::template::template_string_parse(&u, &u);
		shared::template::template_string_parse(&b, &b);

		if cached_regex(&c.for_messages).is_some() {
			info!(message="regex compile", regex=%c.for_messages, api=%c.name);
		} else {
			error!(message="regex compile", regex=%c.for_messages, api=%c.name);
		}
	}
}

/// Returns the compiled Regular Expression of the pattern
///
/// A pattern which is not cached yet is compiled and cached.
///
/// # Arguments
///
/// * `pattern` - The Regular Expression
///
/// # Returns
///
/// The Regex or None if the pattern is not a valid Regular Expression
fn cached_regex(pattern: &str) -> Option<Regex> {
	if let Ok(cache) = ENDPOINT_REGEXES.read()
		&& let Some(re) = cache.as_ref().and_then(|c| c.get(pattern))
	{
		return Some(re.clone());
	}

	let re = Regex::new(pattern).ok()?;
	if let Ok(mut cache) = ENDPOINT_REGEXES.write() {
		cache
			.get_or_insert_with(HashMap::new)
			.insert(pattern.to_owned(), re.clone());
	}
	Some(re)
}

/// Finds the first matching Endpoint-Configuration for a given message
///
/// # Arguments
///
/// * `endpoints` - List of all Endpoint-Configurations
/// * `msg` - Message to find a configuration for
///
/// # Returns
///
/// The matching Endpoint or an Error if there is no Endpoint which matches the message.
/// An Endpoint with an invalid regular expression is logged and never matches.
fn find_matching_endpoint<'a>(endpoints: &'a [Endpoint], msg: &str) -> Result<&'a Endpoint> {
	endpoints
		.iter()
		.find(|e| match cached_regex(&e.for_messages) {
			Some(matcher) => matcher.is_match(msg),
			None => {
				error!(message="invalid regex", regex=%e.for_messages, api=%e.name);
				false
			}
		})
		.ok_or_else(|| anyhow!("no suitable endpoint configuration found"))
}

/// Processes one message: Sends the web request(s) and enqueues the answer(s)
///
/// # Arguments
///
/// * `msg` - The received message which has to be a JSON-String
/// * `endpoints` - List of all Endpoint-Configurations
/// * `queue` - The output queue where the answers are enqueued
/// * `send_reqwest` - Function which sends out the real request
/// * `queue_message` - Function to enqueue an answer
///
/// # Returns
///
/// An empty Ok Result or an Error identifying what went wrong
fn process_message(
	msg: &str,
	endpoints: &[Endpoint],
	queue: Arc<MessageQueue<String>>,
	send_reqwest: impl Fn(RequestBuilder) -> Result<Response, reqwest::Error>,
	queue_message: impl Fn(String, Arc<MessageQueue<String>>),
) -> Result<()> {
	debug!(message="processing message", msg=%msg);

	let endpoint = find_matching_endpoint(endpoints, msg)?;
	let json: Value = serde_json::from_str(msg).context("message is not a valid json")?;

	debug!(message="polling api", method=%endpoint.method, api=%endpoint.name, auth=%endpoint.auth.as_ref().unwrap_or(&Authentication::None));
	call_api(endpoint, Arc::new(json), queue, send_reqwest, queue_message)
		.with_context(|| format!("calling api '{}'", endpoint.name))
}

/// Sends the request(s) of an Endpoint and enqueues each answer
///
/// The `$response` of all templates is the received message in the first request and the
/// JSON-Answer of the previous request in every following paging request.
///
/// # Arguments
///
/// * `conf` - The Endpoint to send the request to
/// * `message` - The received message as JSON
/// * `queue` - The output queue where the answers are enqueued
/// * `send_reqwest` - Function which sends out the real request (dep-injection for test)
/// * `queue_message` - Function to enqueue an answer (dep-injection for test)
///
/// # Returns
///
/// An empty Ok Result or an Error if the request failed or the status code was not 200
fn call_api(
	conf: &Endpoint,
	message: Arc<Value>,
	queue: Arc<MessageQueue<String>>,
	send_reqwest: impl Fn(RequestBuilder) -> Result<Response, reqwest::Error>,
	queue_message: impl Fn(String, Arc<MessageQueue<String>>),
) -> Result<()> {
	let client = Client::builder().build()?;
	let mut response = message;
	let mut paging = true;
	let mut pages = 0;

	while paging {
		// Parse URI and Body
		let mut uri = shared::template::template_string(&conf.uri, Arc::clone(&response));
		let send_body = shared::template::template_string(
			&conf.body.as_deref().unwrap_or_default().to_owned(),
			Arc::clone(&response),
		);

		// Append Paging if available
		if let Some(pg) = conf.paging.as_ref()
			&& let Some(param) = pg.param.as_ref()
			&& !param.name.is_empty()
		{
			let page_val = shared::template::template_string(&param.value, Arc::clone(&response));
			let sep = if uri.contains('?') { "&" } else { "?" };
			uri = format!(
				"{}{}{}={}",
				uri,
				sep,
				param.name.as_str(),
				page_val.as_str()
			);
		}
		debug!(message="calling api", api=conf.uri, uri=%uri);

		// Create the Request
		let mut req = match conf.method {
			Method::GET => client.get(uri),
			Method::POST => client.post(uri).body(send_body),
			Method::HEAD => client.head(uri),
			_ => client.get(uri),
		};

		// Authentication
		req = match &conf.auth {
			Some(Authentication::Header { header, value }) => req.header(
				header,
				shared::template::template_string(
					&secrets_string(value).unwrap_or_default(),
					Arc::clone(&response),
				),
			),

			Some(Authentication::Basic { user, pass }) => req.basic_auth(
				secrets_string(user).unwrap_or_default(),
				secrets_string(pass).ok(),
			),

			Some(Authentication::Bearer(token)) => {
				if token.starts_with("Bearer") {
					req.bearer_auth(
						secrets_string(token.get(7..).unwrap_or_default()).unwrap_or_default(),
					)
				} else {
					req.bearer_auth(secrets_string(token).unwrap_or_default())
				}
			}

			_ => req,
		};

		// Additional Headers
		req = req.header("User-Agent", "ingesto-processor-polling/1.0");
		for header in &conf.header {
			req = req.header(
				header.name.to_string(),
				shared::template::template_string(&header.value, Arc::clone(&response)),
			);
		}

		let resp = send_reqwest(req)?;
		let status = resp.status();
		let body = resp.text().unwrap_or_default();
		if status != StatusCode::OK {
			bail!("unexpected status {}: {}", status.as_u16(), body);
		}

		debug!(message = "data received", data = body.to_owned());
		queue_message(body.to_owned(), queue.clone());

		// Check for paging
		if let Some(pg) = conf.paging.as_ref() {
			pages += 1;
			debug!(message="paging", conf=%pg, pages=pages, max_pages=MAX_PAGING_REQUESTS);
			if pg.until == PagingRequestUntil::None
				|| pages >= pg.max_pages
				|| pages >= MAX_PAGING_REQUESTS
			{
				break;
			}

			paging = !pg.until.check(status.as_u16(), body.to_owned());
			debug!(message = "apply paging", apply = paging);
			if paging {
				match serde_json::from_str(body.to_owned().as_str()).unwrap_or_default() {
					Value::Null => break,
					j => response = Arc::<Value>::new(j),
				}

				// Wait between the paging requests to avoid rate limiting
				if pg.timeout > 0 {
					thread::sleep(Duration::from_millis(u64::from(pg.timeout)));
				}
			}
		} else {
			break;
		}
	}

	Ok(())
}

fn call_api_internal(req: RequestBuilder) -> Result<Response, reqwest::Error> {
	req.send()
}

fn queue_message_internal(data: String, queue: Arc<MessageQueue<String>>) {
	queue.push(data);
}

#[cfg(test)]
pub mod test {
	use super::*;

	use serde_json::json;

	use crate::types::{PagingRequest, Param};

	fn endpoint(for_messages: &str, uri: &str) -> Endpoint {
		Endpoint {
			name: String::from("Test API"),
			for_messages: String::from(for_messages),
			uri: String::from(uri),
			..Endpoint::default()
		}
	}

	fn paging(until: PagingRequestUntil, max_pages: u16) -> PagingRequest {
		PagingRequest {
			param: Some(Param {
				name: String::from("page"),
				value: String::from("{{ $response/paging/page }}"),
			}),
			until,
			timeout: 0,
			max_pages,
		}
	}

	fn pull_json(queue: &MessageQueue<String>) -> Value {
		let msg = queue.pull(Duration::from_millis(100)).unwrap_or_default();
		serde_json::from_str(&msg).unwrap_or_default()
	}

	/// Answers with a JSON which contains the sent request and a paging-section
	fn call_api_echo(reqb: RequestBuilder) -> Result<Response, reqwest::Error> {
		let req = reqb.build()?;
		let body = req
			.body()
			.and_then(|b| b.as_bytes())
			.map(|b| String::from_utf8_lossy(b).to_string());
		let header = |name: &str| {
			req.headers()
				.get(name)
				.and_then(|v| v.to_str().ok())
				.map(String::from)
		};

		let json = json!({
			"uri": req.url().as_str(),
			"method": req.method().as_str(),
			"body": body,
			"x-id": header("x-id"),
			"authorization": header("authorization"),
			"paging": { "page": 1, "cursor": "testing" },
		});
		Ok(Response::from(http::Response::new(json.to_string())))
	}

	/// Answers with the requested URI as a plain string, which is not JSON
	fn call_api_nojson(reqb: RequestBuilder) -> Result<Response, reqwest::Error> {
		let url = reqb.build()?.url().to_string();
		Ok(Response::from(http::Response::new(url)))
	}

	fn call_api_status(status: u16) -> impl Fn(RequestBuilder) -> Result<Response, reqwest::Error> {
		move |_reqb| {
			let resp = http::Response::builder()
				.status(status)
				.body(String::from("some error"))
				.unwrap_or_default();
			Ok(Response::from(resp))
		}
	}

	fn call_api_failing(reqb: RequestBuilder) -> Result<Response, reqwest::Error> {
		// Building a request with an invalid URL results in a reqwest::Error
		reqb.build()
			.map(|_| Response::from(http::Response::new("")))
	}

	#[test]
	fn test_process_message_uri_from_message() {
		let endpoints = vec![endpoint(
			".*",
			"http://127.0.0.1/users/{{ $response/user/id }}/{{ $response/user/name }}",
		)];
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			r#"{"user":{"id":42,"name":"foo"}}"#,
			&endpoints,
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
		assert_eq!(pull_json(&queue)["uri"], "http://127.0.0.1/users/42/foo");
	}

	#[test]
	fn test_process_message_body_and_header_from_message() {
		let mut conf = endpoint(".*", "http://127.0.0.1/post");
		conf.method = Method::POST;
		conf.body = Some(String::from("agent={{ $response/agent }}"));
		conf.header = vec![Param {
			name: String::from("X-Id"),
			value: String::from("id-{{ $response/agent }}"),
		}];
		conf.auth = Some(Authentication::Bearer(String::from("Bearer secret")));
		let endpoints = vec![conf];
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			r#"{"agent":"abc"}"#,
			&endpoints,
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_ok());
		let json = pull_json(&queue);
		assert_eq!(json["method"], "POST");
		assert_eq!(json["body"], "agent=abc");
		assert_eq!(json["x-id"], "id-abc");
		assert_eq!(json["authorization"], "Bearer secret");
	}

	#[test]
	fn test_process_message_first_matching_endpoint() {
		let endpoints = vec![
			endpoint("\"type\":\"first\"", "http://127.0.0.1/first"),
			endpoint("\"type\":\"second\"", "http://127.0.0.1/second"),
			endpoint(".*", "http://127.0.0.1/any"),
		];
		let queue = Arc::new(MessageQueue::<String>::new());

		for (msg, expected) in [
			(r#"{"type":"second"}"#, "http://127.0.0.1/second"),
			(r#"{"type":"first"}"#, "http://127.0.0.1/first"),
			(r#"{"type":"other"}"#, "http://127.0.0.1/any"),
		] {
			let res = process_message(
				msg,
				&endpoints,
				queue.clone(),
				call_api_echo,
				queue_message_internal,
			);
			assert!(res.is_ok());
			assert_eq!(pull_json(&queue)["uri"], expected);
		}
	}

	#[test]
	fn test_process_message_no_match() {
		let endpoints = vec![endpoint("\"type\":\"first\"", "http://127.0.0.1/")];
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			r#"{"type":"other"}"#,
			&endpoints,
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert_eq!(
			res.unwrap_err().to_string(),
			"no suitable endpoint configuration found"
		);
		assert_eq!(queue.size(), 0);
	}

	#[test]
	fn test_process_message_no_endpoints() {
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			r#"{"type":"other"}"#,
			&[],
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_err());
		assert_eq!(queue.size(), 0);
	}

	#[test]
	fn test_process_message_invalid_json() {
		let endpoints = vec![endpoint(".*", "http://127.0.0.1/")];
		let queue = Arc::new(MessageQueue::<String>::new());

		for msg in ["not a json", "{\"open\":", ""] {
			let res = process_message(
				msg,
				&endpoints,
				queue.clone(),
				call_api_echo,
				queue_message_internal,
			);
			assert_eq!(res.unwrap_err().to_string(), "message is not a valid json");
		}
		assert_eq!(queue.size(), 0);
	}

	#[test]
	fn test_process_message_missing_values_are_empty() {
		let endpoints = vec![endpoint(
			".*",
			"http://127.0.0.1/users/?id={{ $response/user/id }}",
		)];
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			r#"{"other":1}"#,
			&endpoints,
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_ok());
		assert_eq!(pull_json(&queue)["uri"], "http://127.0.0.1/users/?id=");
	}

	#[test]
	fn test_process_message_error_status() {
		let endpoints = vec![endpoint(".*", "http://127.0.0.1/")];
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			"{}",
			&endpoints,
			queue.clone(),
			call_api_status(500),
			queue_message_internal,
		);

		let err = format!("{:#}", res.unwrap_err());
		assert!(err.contains("calling api 'Test API'"));
		assert!(err.contains("unexpected status 500"));
		assert_eq!(queue.size(), 0);
	}

	#[test]
	fn test_process_message_request_error() {
		let endpoints = vec![endpoint(".*", "not an url")];
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = process_message(
			"{}",
			&endpoints,
			queue.clone(),
			call_api_failing,
			queue_message_internal,
		);

		assert!(res.is_err());
		assert_eq!(queue.size(), 0);
	}

	#[test]
	fn test_find_matching_endpoint_skips_invalid_regex() {
		let endpoints = vec![
			endpoint("(", "http://127.0.0.1/invalid"),
			endpoint("^valid", "http://127.0.0.1/valid"),
		];

		let found = find_matching_endpoint(&endpoints, "valid message");
		assert_eq!(
			found.map(|e| e.uri.as_str()).unwrap_or_default(),
			"http://127.0.0.1/valid"
		);
		assert!(find_matching_endpoint(&endpoints[..1], "valid message").is_err());
	}

	#[test]
	fn test_call_api_paging() {
		let mut conf = endpoint(
			".*",
			"http://127.0.0.1/polling/?cursor={{ $response/paging/cursor }}",
		);
		conf.paging = Some(paging(PagingRequestUntil::Empty, 2));
		let queue = Arc::new(MessageQueue::<String>::new());
		let message = Arc::new(json!({ "paging": { "cursor": "start", "page": 0 } }));

		let res = call_api(
			&conf,
			message,
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		// Two requests: the first uses the message, the second the answer of the first
		assert!(res.is_ok());
		assert_eq!(queue.size(), 2);
		assert_eq!(
			pull_json(&queue)["uri"],
			"http://127.0.0.1/polling/?cursor=start&page=0"
		);
		assert_eq!(
			pull_json(&queue)["uri"],
			"http://127.0.0.1/polling/?cursor=testing&page=1"
		);
	}

	#[test]
	fn test_call_api_paging_until_equals() {
		let mut conf = endpoint(".*", "http://127.0.0.1/polling/");
		conf.paging = Some(paging(
			PagingRequestUntil::Equals(
				String::from("{{ $response/paging/page }}"),
				String::from("1"),
			),
			10,
		));
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = call_api(
			&conf,
			Arc::new(json!({})),
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		// The answer of the first request has already page 1
		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
	}

	#[test]
	fn test_call_api_paging_max_pages() {
		let mut conf = endpoint(".*", "http://127.0.0.1/polling/");
		conf.paging = Some(paging(PagingRequestUntil::Empty, 3));
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = call_api(
			&conf,
			Arc::new(json!({})),
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_ok());
		assert_eq!(queue.size(), 3);
	}

	#[test]
	fn test_call_api_paging_zero_max_pages() {
		let mut conf = endpoint(".*", "http://127.0.0.1/polling/");
		conf.paging = Some(paging(PagingRequestUntil::Empty, 0));
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = call_api(
			&conf,
			Arc::new(json!({})),
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
	}

	#[test]
	fn test_call_api_paging_without_json_answer() {
		let mut conf = endpoint(".*", "http://127.0.0.1/polling/?id={{ $response/id }}");
		conf.paging = Some(paging(PagingRequestUntil::Empty, 5));
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = call_api(
			&conf,
			Arc::new(json!({ "id": "abc" })),
			queue.clone(),
			call_api_nojson,
			queue_message_internal,
		);

		// No more requests possible because the answer is not JSON
		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
		assert_eq!(
			queue.pull(Duration::from_millis(100)).unwrap_or_default(),
			"http://127.0.0.1/polling/?id=abc&page="
		);
	}

	#[test]
	fn test_call_api_paging_error_keeps_previous_pages() {
		let mut conf = endpoint(".*", "http://127.0.0.1/polling/");
		conf.paging = Some(paging(PagingRequestUntil::Empty, 5));
		let queue = Arc::new(MessageQueue::<String>::new());
		let calls = std::sync::atomic::AtomicU16::new(0);

		let res = call_api(
			&conf,
			Arc::new(json!({})),
			queue.clone(),
			|reqb| {
				if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
					call_api_echo(reqb)
				} else {
					call_api_status(503)(reqb)
				}
			},
			queue_message_internal,
		);

		assert!(res.is_err());
		assert_eq!(queue.size(), 1);
	}

	#[test]
	fn test_call_api_without_paging_sends_once() {
		let conf = endpoint(".*", "http://127.0.0.1/polling/");
		let queue = Arc::new(MessageQueue::<String>::new());

		let res = call_api(
			&conf,
			Arc::new(json!({})),
			queue.clone(),
			call_api_echo,
			queue_message_internal,
		);

		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
	}

	#[test]
	fn test_cached_regex() {
		assert!(cached_regex("^cache-test-[0-9]+$").is_some());
		// The second call is served from the cache
		let re = cached_regex("^cache-test-[0-9]+$");
		assert!(re.is_some_and(|r| r.is_match("cache-test-42") && !r.is_match("cache-test-x")));
		assert!(cached_regex("(").is_none());
		assert!(cached_regex("(").is_none());
	}

	#[test]
	fn test_precompile_caches_templates_and_regexes() {
		let mut conf = endpoint("^precompile-test$", "http://127.0.0.1/{{ $response/id }}");
		conf.body = Some(String::from("id={{ $response/id }}"));

		// Valid and invalid regexes must not panic
		precompile(&[conf, endpoint("(", "http://127.0.0.1/")]);

		assert!(ENDPOINT_REGEXES.read().is_ok_and(|c| {
			c.as_ref()
				.is_some_and(|c| c.contains_key("^precompile-test$"))
		}));
		assert!(
			ENDPOINT_REGEXES
				.read()
				.is_ok_and(|c| c.as_ref().is_some_and(|c| !c.contains_key("(")))
		);
	}

	#[test]
	fn test_message_threads_are_independent() {
		let endpoints = Arc::new(vec![endpoint(
			".*",
			"http://127.0.0.1/users/{{ $response/id }}",
		)]);
		let queue = Arc::new(MessageQueue::<String>::new());

		let handles: Vec<_> = (0..8)
			.map(|id| {
				let endpoints = Arc::clone(&endpoints);
				let queue = Arc::clone(&queue);
				thread::spawn(move || {
					process_message(
						&format!("{{\"id\":{}}}", id),
						&endpoints,
						queue,
						call_api_echo,
						queue_message_internal,
					)
				})
			})
			.collect();
		for handle in handles {
			assert!(handle.join().is_ok_and(|res| res.is_ok()));
		}

		let mut uris: Vec<String> = (0..8)
			.map(|_| {
				pull_json(&queue)["uri"]
					.as_str()
					.unwrap_or_default()
					.to_string()
			})
			.collect();
		uris.sort();
		let expected: Vec<String> = (0..8)
			.map(|id| format!("http://127.0.0.1/users/{}", id))
			.collect();
		assert_eq!(uris, expected);
	}
}
