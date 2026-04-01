pub mod config;

use std::{sync::{Arc}, thread};
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
		Ok(c) => c.webhook,
		Err(e) => {
			error!("{:#?}", e);
			return;
		}
	};

	info!(message="starting", name=%conf.name);
	let queue = Arc::new(MessageQueue::<String>::new());
	MessageParser::<String>::new(Arc::clone(&queue)).run();

	let c = Arc::new(conf);
	let q = queue.clone();
	let res = webhook_listener(c, q);
	match res {
		Err(e) => {
			error!("{:#?}", e);
		},
		Ok(_) => {}
	}
}

fn webhook_listener(conf: Arc<config::Webhook>, queue: Arc<shared::queue::MessageQueue<String>>) ->anyhow::Result<()> {
	let server = match Server::http(conf.listen.get_address()) {
		Ok(server) => server,
		Err(e) => return Err(anyhow!("{:#?}", e))
	};
	info!(message="http server started", address=%conf.listen.address, port=%conf.listen.port);

	for request in server.incoming_requests() {
		let c = conf.clone();
		let q = queue.clone();
		thread::spawn(move || handle_request(request, c, q));
	}

	Ok(())
}

fn handle_request(mut req: Request, conf: Arc<config::Webhook>, queue: Arc<shared::queue::MessageQueue<String>>) {
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
		let _ = req.respond(resp);
		error!("Invalid Request: {:?} {:?}", method, url);
		return;
	}
	let _route = route_opt.unwrap();

	// TODO: Check Authorization req.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.clone()); ...
	//       Check Token-Headers req.headers().iter().find(|h| h.field.equiv("???")).map(|h| h.value.clone()); ...

	// Get the body and enqueue it based on the route information
	let mut body = String::new();
	if let Err(e) = req.as_reader().read_to_string(&mut body) {
		let mut resp = Response::from_string("invalid data");
		resp = resp.with_status_code(StatusCode(400));
		let _ = req.respond(resp);
		error!("Error while reading Requets-Body: {:#?}", e);
		return;
	}

	// TODO: Possible Preprocessing based on the route?

	// Append to the queue
	let src_addr = if req.remote_addr().is_none() { String::from("0.0.0.0:0") } else { req.remote_addr().unwrap().to_string() };
	debug!(message="data received", src_addr=%src_addr, size=body.len(), data=%body);
	queue.push(body);

	// Just a default response
	let mut ok = Response::from_string("processed");
	ok = ok.with_status_code(200);
	let _ = req.respond(ok);
}



