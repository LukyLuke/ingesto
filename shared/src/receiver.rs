
use std::{sync::Arc, thread};

use opentelemetry_proto::tonic::collector::logs::v1::{ExportLogsServiceRequest, ExportLogsServiceResponse};
use opentelemetry_proto::tonic::common::v1::any_value::Value::{
	self, ArrayValue, BoolValue, BytesValue, DoubleValue, IntValue, KvlistValue, StringValue, StringValueStrindex
};
use prost::Message;
use serde_json::{Map, json};
use tiny_http::{Header, Request, Response, Server, StatusCode};
use tracing::{debug, error, info};

use crate::{queue::MessageQueue, types::OtelReceiver};

pub fn start_otel_listener(conf: Arc<OtelReceiver>, queue: Arc<MessageQueue::<String>>) ->  anyhow::Result<()> {
	let server = match Server::http(&conf.get_address()) {
		Ok(server) => server,
		Err(e) => return Err(anyhow::anyhow!("{:#?}", e))
	};
	info!(message="http server started", address=%conf.address, port=%conf.port);

	for request in server.incoming_requests() {
		let q = queue.clone();
		let c = conf.clone();
		thread::spawn(move || handle_request(request, c, q));
	}

	Ok(())
}

fn handle_request(mut req: Request, conf: Arc<OtelReceiver>, queue: Arc<MessageQueue<String>>) {
	let url = req.url().to_string();
	let method = req.method().to_string();

	if method == "POST" && url == conf.path {
		// Read and decode the body as OTEL LogRecord
		let mut body = Vec::new();
		if let Err(e) = req.as_reader().read_to_end(&mut body) {
			let resp = Response::from_string("invalid data")
				.with_status_code(StatusCode(400))
				.with_header(Header::from_bytes("content-type", "text/plain").unwrap());
			req.respond(resp).ok();

			error!("Error while reading request data: {:#?}", e);
			return;
		}

		// Use prost::Message decode trait to deserialize the received body into a ExportLogsServiceRequest
		let logs = match ExportLogsServiceRequest::decode(body.as_slice()) {
			Ok(l) => l,
			Err(e) => {
				let resp = Response::from_string("invalid data")
					.with_status_code(StatusCode(400))
					.with_header(Header::from_bytes("content-type", "text/plain").unwrap());
				req.respond(resp).ok();

				error!("Error while reading request data: {:#?}", e);
				return;
			}
		};

		// Process logs and insert into queue
		let num = logs.resource_logs.iter()
			.flat_map(|log| log.scope_logs.iter()
				.flat_map(|log| log.log_records.iter())
			)
			.filter_map(|line| line.body.as_ref())
			.filter_map(|line| line.value.as_ref())
			.map(convert_tonic)
			.filter_map(|val| {
				match serde_json::to_string(&val) {
					Ok(json) => {
						queue.push(json.clone());
						debug!(message="log received", log=%json);
						Ok(())
					},
					Err(e) => {
						error!(message="log not in json format", value=%val, error=%e);
						Err(e)
					}
				}.ok()
			})
			.count();
		info!(message="received logs", num=num);

		// Send back a default OK-Response
		let resp_msg = ExportLogsServiceResponse::default();
		let mut out = Vec::new();
		let _ = resp_msg.encode(&mut out);

		let resp = Response::from_data(out)
			.with_status_code(StatusCode(200))
			.with_header(Header::from_bytes("content-type", "application/x-protobuf").unwrap());
			req.respond(resp).ok();

	} else {
		debug!(message="invalid request", url=url, method=method);

		let resp = Response::from_string("invalid request")
		.with_status_code(StatusCode(500))
		.with_header(Header::from_bytes("content-type", "text/plain").unwrap());
		req.respond(resp).ok();
	}
}

fn convert_tonic(val: &Value) -> serde_json::Value {
	match val {
		BoolValue(v) => json!(*v),
		StringValue(v) => json!(v.to_string()),
		StringValueStrindex(v) => { error!("type 'StringValueStrindex' received which shold not be used: {}", v); json!(v.to_string()) },
		IntValue(v) => json!(v),
		DoubleValue(v) => json!(v),
		BytesValue(v) => json!(String::from_utf8(v.clone()).unwrap_or_default()),
		ArrayValue(v) => {
			v.values.iter()
				.filter_map(|v| v.value.as_ref())
				.map(convert_tonic)
				.collect()
		},
		KvlistValue(v) => {
			let mut map = Map::new();
			v.values.iter().for_each(|kv| {
				let val = if let Some(v) = kv.value.as_ref() {
					let def = StringValue("".to_owned());
					let inner = v.value.as_ref().unwrap_or(&def);
					convert_tonic(&inner)
				} else {
					serde_json::Value::Null
				};
				map.insert(kv.key.to_owned(), val);
			});
			serde_json::Value::Object(map)
		},
	}
}
