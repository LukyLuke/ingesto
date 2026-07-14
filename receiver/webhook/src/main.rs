pub mod config;

use std::{sync::Arc, thread};
use anyhow::anyhow;
use tiny_http::{self, Request, Response, Server, StatusCode};
use shared::{self, init_logging, usage, parser::MessageParser, queue::MessageQueue};
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
	let parser = Arc::new(MessageParser::<String>::new(queue.clone(), conf.queue.clone(), conf.parser.clone()));
	parser.run();

	let res = webhook_listener(conf.clone(), queue.clone());
	match res {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

fn webhook_listener(conf: Arc<config::Webhook>, queue: Arc<shared::queue::MessageQueue<String>>) -> anyhow::Result<()> {
	let server = match Server::http(conf.listen.get_address()) {
		Ok(server) => server,
		Err(e) => return Err(anyhow!("{:#?}", e))
	};
	info!(message="http server started", address=%conf.listen.address, port=%conf.listen.port);

	for request in server.incoming_requests() {
		let c = conf.clone();
		let q = queue.clone();
		thread::spawn(move || handle_request(request, c, q).is_err_and(|err| {
			error!("{:?}", err);
			true
		}) );
	}

	Ok(())
}

fn handle_request(mut req: Request, conf: Arc<config::Webhook>, queue: Arc<shared::queue::MessageQueue<String>>) -> anyhow::Result<()> {
	// As first get the configured route
	let url = req.url().to_string();
	let method = req.method().to_string();
	let mut route_opt: Option<&config::Route> = None;
	for r in &conf.routes {
		if url == r.path && method == r.kind {
			route_opt = Some(r);
		}
	}
	if route_opt.is_none() {
		let mut resp = Response::from_string("invalid request");
		resp = resp.with_status_code(StatusCode(404));
		req.respond(resp).ok();
		return Err(anyhow!("Invalid Request: {:?} {:?}", method, url));
	}
	let route = route_opt.unwrap();

	// Check Authentication
	if let Some(expected) = &route.auth {
		let header_name = match expected {
			config::Authentication::Basic { user: _, pass: _ } | config::Authentication::Bearer(_) => "Authorization".to_string(),
			config::Authentication::Header { name, value: _ } => name.to_owned(),
			_ => String::new()
		};
		let auth_header = req.headers()
			.iter()
			.find(|h| h.field.as_str().as_str() == header_name )
			.map(|h| h.value.to_string())
			.map(|h| format!("{}: {}", header_name, h));

		if auth_header.as_ref().is_none() || !expected.check(auth_header.as_ref()) {
			let mut resp = Response::from_string("invalid authorization");
			resp = resp.with_status_code(StatusCode(401));
			req.respond(resp).ok();
			return Err(anyhow!("Authorization Required: {:?} {:?}", method, url));
		}
	}

	// Get the body and enqueue it based on the route information
	let mut body = String::new();
	if let Err(e) = req.as_reader().read_to_string(&mut body) {
		let mut resp = Response::from_string("invalid data");
		resp = resp.with_status_code(StatusCode(400));
		req.respond(resp).ok();
		return Err(anyhow!("Error while reading Requets-Body: {:#?}", e));
	}

	// TODO: Possible Preprocessing based on the route?

	// Append to the queue
	let src_addr = if req.remote_addr().is_none() { String::from("0.0.0.0:0") } else { req.remote_addr().unwrap().to_string() };
	debug!(message="data received", src_addr=%src_addr, size=body.len(), data=%body);
	queue.push(body);

	// Just a default response
	let mut ok = Response::from_string("processed");
	ok = ok.with_status_code(200);
	req.respond(ok).map_err(|err| anyhow!("{:?}", err))
}

#[cfg(test)]
mod test {
	use std::str::FromStr;

	use super::*;
	use shared::types;
	use tiny_http::TestRequest;


	#[test]
	fn test_handle_request_no_routes() {
		let conf = Arc::new(config::Webhook{
			name: "".to_string(),
			listen: config::Server { address: "0.0.0.0".to_string(), port: 1514 },
			routes: vec![],
			queue: types::Queue::default(),
			parser: vec![],
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		let req = TestRequest::new().with_body("foo bar");

		let res = handle_request(req.into(), conf, queue.clone());
		assert!(res.is_err());
		assert_eq!(res.err().unwrap().to_string().get(0..16).unwrap(), "Invalid Request:");
	}

	#[test]
	fn test_handle_request_no_route_match() {
		let conf = Arc::new(config::Webhook{
			name: "".to_string(),
			listen: config::Server { address: "0.0.0.0".to_string(), port: 1514 },
			routes: vec![config::Route{
				path: "/test".to_string(),
				kind: "GET".to_string(),
				auth: None,
			}],
			queue: types::Queue::default(),
			parser: vec![],
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		let req = TestRequest::new().with_body("foo bar");

		let res = handle_request(req.into(), conf, queue.clone());
		assert!(res.is_err());
		assert_eq!(res.err().unwrap().to_string().get(0..16).unwrap(), "Invalid Request:");
	}

	#[test]
	fn test_handle_request_route_match() {
		let conf = Arc::new(config::Webhook{
			name: "".to_string(),
			listen: config::Server { address: "0.0.0.0".to_string(), port: 1514 },
			routes: vec![config::Route{
				path: "/test".to_string(),
				kind: "GET".to_string(),
				auth: None,
			}],
			queue: types::Queue::default(),
			parser: vec![],
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		let req = TestRequest::new().with_body("foo bar").with_path("/test").with_method(tiny_http::Method::Get);

		let res = handle_request(req.into(), conf, queue.clone());
		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
	}

	#[test]
	fn test_handle_request_auth_bearer_none() {
		let conf = Arc::new(config::Webhook{
			name: "".to_string(),
			listen: config::Server { address: "0.0.0.0".to_string(), port: 1514 },
			routes: vec![config::Route{
				path: "/test".to_string(),
				kind: "GET".to_string(),
				auth: Some(config::Authentication::Bearer("Bearer TEST".to_string())),
			}],
			queue: types::Queue::default(),
			parser: vec![],
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		let req = TestRequest::new()
			.with_body("foo bar")
			.with_path("/test")
			.with_method(tiny_http::Method::Get);

		let res = handle_request(req.into(), conf, queue.clone());
		assert!(res.is_err());
		assert_eq!(res.err().unwrap().to_string().get(0..23).unwrap(), "Authorization Required:");
	}

	#[test]
	fn test_handle_request_auth_bearer() {
		let conf = Arc::new(config::Webhook{
			name: "".to_string(),
			listen: config::Server { address: "0.0.0.0".to_string(), port: 1514 },
			routes: vec![config::Route{
				path: "/test".to_string(),
				kind: "GET".to_string(),
				auth: Some(config::Authentication::Bearer("Bearer TEST".to_string())),
			}],
			queue: types::Queue::default(),
			parser: vec![],
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		let req = TestRequest::new()
			.with_body("foo bar")
			.with_path("/test")
			.with_method(tiny_http::Method::Get)
			.with_header(tiny_http::Header::from_str("Authorization: Bearer TEST").unwrap());

		let res = handle_request(req.into(), conf, queue.clone());
		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
	}

	#[test]
	fn test_handle_request_auth_basic() {
		let conf = Arc::new(config::Webhook{
			name: "".to_string(),
			listen: config::Server { address: "0.0.0.0".to_string(), port: 1514 },
			routes: vec![config::Route{
				path: "/test".to_string(),
				kind: "GET".to_string(),
				auth: Some(config::Authentication::Basic{ user: "john@example.com".to_string(), pass: "SecretPassword".to_string() }),
			}],
			queue: types::Queue::default(),
			parser: vec![],
		});
		let queue = Arc::new(shared::queue::MessageQueue::<String>::new());
		let req = TestRequest::new()
			.with_body("foo bar")
			.with_path("/test")
			.with_method(tiny_http::Method::Get)
			.with_header(tiny_http::Header::from_str("Authorization: Basic am9obkBleGFtcGxlLmNvbTpTZWNyZXRQYXNzd29yZA==").unwrap());

		let res = handle_request(req.into(), conf, queue.clone());
		assert!(res.is_ok());
		assert_eq!(queue.size(), 1);
	}

}

