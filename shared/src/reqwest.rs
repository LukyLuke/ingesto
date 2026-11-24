use std::sync::Arc;
use serde_json::{Value, json};
use reqwest::{
	self,
	StatusCode,
	blocking::{RequestBuilder, Response},
};
use tracing::{debug, error};

use crate::{
	secrets_string,
	queue,
	template,
	types,
};

static MAX_PAGING_REQUESTS: u16 = 1024;

pub fn call_api(
	conf: &types::Endpoint,
	queue: Arc<queue::MessageQueue<String>>,
	send_reqwest: impl Fn(RequestBuilder) -> Result<Response, reqwest::Error>,
	queue_message: impl Fn(String, Arc<queue::MessageQueue<String>>),
) -> anyhow::Result<()> {
	let mut response = Arc::new(json!({}));
	let mut paging = true;
	let mut pages = 0;

	while paging {
		// Parse URI and Body
		let mut uri = template::template_string(&conf.uri, Arc::clone(&response));
		let send_body = template::template_string(
			&conf.body.as_deref().unwrap_or_default().to_owned(),
			Arc::clone(&response),
		);

		// Append Paging if available
		if let Some(pg) = conf.paging.as_ref()
			&& let Some(param) = pg.param.as_ref()
			&& !param.name.is_empty()
			{
				let page_val = template::template_string(&param.value, Arc::clone(&response));
				let sep = if uri.find('?').is_some() { "&" } else { "?" };
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
			let client = reqwest::blocking::Client::new();
			let mut req = match conf.method {
				types::Method::GET => client.get(uri),
				types::Method::POST => client.post(uri).body(send_body),
				types::Method::HEAD => client.head(uri),
				_ => client.get(uri),
			};

			// Authentication
			req = match &conf.auth {
				Some(types::Authentication::Header { header, value }) => req.header(
					header,
					template::template_string(
						&secrets_string(value).unwrap_or_default(),
						Arc::clone(&response),
					),
				),

				Some(types::Authentication::Basic { user, pass }) => req.basic_auth(
					secrets_string(user).unwrap_or_default(),
					secrets_string(pass).ok(),
				),

				Some(types::Authentication::Bearer(token)) => {
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
			req = req.header("User-Agent", "ingesto-polling/1.0");
			for header in &conf.header {
				req = req.header(
					header.name.to_string(),
					template::template_string(&header.value, Arc::clone(&response)),
				);
			}

			let resp = send_reqwest(req)?;
			let status = resp.status();
			let body = resp.text().unwrap_or_default();
			if status != StatusCode::OK {
				error!(message="calling api", api=conf.uri, status=%status.as_u16(), error=body.to_owned());
				break;
			}

			debug!(message = "data received", data = body.to_owned());
			queue_message(body.to_owned(), queue.clone());

			// Check for paging
			if let Some(pg) = conf.paging.as_ref() {
				pages += 1;
				debug!(message="paging", conf=%pg, pages=pages, max_pages=MAX_PAGING_REQUESTS);
				if pg.until == types::PagingRequestUntil::None
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
				}
			} else {
				break;
			}
	}

	Ok(())
}


#[cfg(test)]
pub mod test {
	use super::*;
	use reqwest::blocking::Response;
	use serde_json::json;
	use std::time::Duration;

	#[test]
	fn test_call_api() {
		let conf = types::Endpoint {
			name: String::from("Test API"),
			uri: String::from("http://127.0.0.1/polling/?cursor={{ $response/paging/cursor }}"),
			body: None,
			method: types::Method::GET,
			auth: None,
			header: Vec::new(),
			paging: Some(types::PagingRequest {
				param: Some(types::Param {
					name: String::from("page"),
					value: String::from("{{ $response/paging/page }}"),
				}),
				until: types::PagingRequestUntil::Empty,
				timeout: 100,
				max_pages: 2,
			}),
		};
		let queue = Arc::new(queue::MessageQueue::<String>::new());
		template::template_string_parse(&conf.uri, &conf.uri);
		template::template_string_parse(
			&conf.body.as_deref().unwrap_or_default().to_owned(),
			&conf.body.as_deref().unwrap_or_default().to_owned(),
		);

		let res = call_api(
			&conf,
			queue.clone(),
			call_api_internal,
			queue_message_internal,
		);

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
		let conf = types::Endpoint {
			name: String::from("Test API"),
			uri: String::from("http://127.0.0.1/polling/?cursor={{ $response/paging/cursor }}"),
			body: None,
			method: types::Method::GET,
			auth: None,
			header: Vec::new(),
			paging: Some(types::PagingRequest {
				param: Some(types::Param {
					name: String::from("page"),
					value: String::from("{{ $response/paging/page }}"),
				}),
				until: types::PagingRequestUntil::Empty,
				timeout: 100,
				max_pages: 2,
			}),
		};
		let queue = Arc::new(queue::MessageQueue::<String>::new());
		template::template_string_parse(&conf.uri, &conf.uri);
		template::template_string_parse(
			&conf.body.as_deref().unwrap_or_default().to_owned(),
			&conf.body.as_deref().unwrap_or_default().to_owned(),
		);

		let res = call_api(
			&conf,
			queue.clone(),
			call_api_internal_nojson,
			queue_message_internal,
		);

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
		headers.iter().for_each(|h| {
			parts.headers.insert(h.0, h.1.to_owned());
		});

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
		headers.iter().for_each(|h| {
			parts.headers.insert(h.0, h.1.to_owned());
		});

		let resp = http::response::Response::from_parts(parts, url);
		Ok(Response::from(resp))
	}

	fn queue_message_internal(data: String, queue: Arc<queue::MessageQueue<String>>) {
		queue.push(data);
	}
}
