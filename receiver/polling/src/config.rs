use core::fmt;
use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared::types::{Parser, Queue};
use tracing::warn;

use crate::template::Template;

/// Static Lazy-Loaded template cache
static TEMPLATE_CACHE: Lazy<DashMap<String, Template>> = Lazy::new(|| DashMap::new());

/// The main Polling-Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Polling,
}

/// A Polling Configuration
#[derive(Debug, Deserialize, Serialize)]
pub struct Polling {
	/// Name of the instance
	pub name: String,

	/// Where to send the requests to
	pub api: Endpoint,

	/// Timeout between the requests
	#[serde(default = "default_cron_timer")]
	pub timer: String,

	/// Message-Queue Configuration
	#[serde(default)]
	pub queue: Queue,

	/// Message-Parser Configuration
	#[serde(default)]
	pub parser: Vec<Parser>,
}

/// An Endpoint where and how to send a request to
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
	/// FQDN where to send a request to.
	/// Can contain Template-Parameters: {{ $uuid }}, {{ $date([$response/json/pointer/value]#FORMAT) }}, {{ $response/json/pointer/value }}
	pub uri: String,

	/// In case of a POST, the Body to send.
	/// Can contain Template-Parameters
	#[serde(default)]
	pub body: Option<String>,

	/// Method to use to send a request
	/// Can be GET, POST, HEAD, OPTION
	#[serde(default = "default_method")]
	pub method: Method,

	/// Authentication Configuration
	#[serde(default)]
	pub auth: Option<Authentication>,

	/// Custom Header Pairs
	#[serde(default)]
	pub header: Vec<Param>,

	/// Paging-Request Configuration
	#[serde(default)]
	pub paging: PagingReguest,
}

/// Request-Methods
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Method {
	GET,
	POST,
	HEAD,
	OPTION,
}
impl fmt::Display for Method {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

/// Authentication on an Endpoint
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	/// No Authenticaton
	None,

	/// Basic-Auth with a User and Password
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Basic { user: String, pass: String },

	/// A Bearer Token
	/// With/out 'Bearer' prefix
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Bearer(String),

	/// Header Authentication with a key and a value.
	/// use `file:/FILE` or `env:ENV_VAR` for a secure configuration of user and password values
	Header(Param),
}
impl fmt::Display for Authentication {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Authentication::None => write!(f, "None"),
			Authentication::Basic { user, pass } => write!(f, "Basic '{}: {}****'", user, pass.get(0..3).unwrap_or_default()),
			Authentication::Bearer(bearer) => write!(f, "Bearer {}****", bearer.get(0..3).unwrap_or_default()),
			Authentication::Header(param) => write!(f, "Header '{}: {}****'", param.name, param.value.get(0..3).unwrap_or_default()),
		}
	}
}

/// A simple Key-Value pair used for different representations
/// The Value can be a Template-Param in most constructs: {{ ... }}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Param {
	pub name: String,
	pub value: String,
}
impl fmt::Display for Param {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "[{:?}]='{:?}'", self.name, self.value)
	}
}

/// Paging Requests can be used if an Endpoint sends a lot of data which are split over multiple requests and responses
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PagingReguest {
	/// Name and Value for the parameter which is added on the Endpoints URI
	/// The Value can/should be a Template-Value which normally contains a value from the response, like: `{{ $response/paging/cursor }}`
	pub param: Param,

	/// Defines how to check if there is no more pages
	pub until: Option<PagingRequestUntil>,

	/// Timeout between paging requests in milliseconds
	#[serde(default)]
	pub timeout: u32,

	/// Maximum number of paging requests
	/// Exit-Strategy to avoid too many requests
	#[serde(default)]
	pub max_pages: u16,
}
impl Default for PagingReguest {
	fn default() -> Self {
		Self { param: Param { name: String::new(), value: String::new() }, until: None, timeout: 3600, max_pages: 1 }
	}
}
impl fmt::Display for PagingReguest {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "PagingRequest: [param]={:?}; [timeout]={:?}; [max]={:?}; [until]={:?};", self.param, self.timeout, self.max_pages, self.until.as_ref().unwrap_or(&PagingRequestUntil::None))
	}
}

/// Defines the paging
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum PagingRequestUntil {
	/// No Paging
	None,

	/// An empty response
	Empty,

	/// A defined status code
	StatusCode(u16),

	/// An empty value inside the json-response
	EmptyValue(String),

	/// Two values from inside the json-response or static strings have to match
	Equals(String, String),
}
impl PagingRequestUntil {
	/// Checks if the PagingRequestUntil matches the received response
	///
	/// # Arguments
	///
	/// * `status` - Status Code from the web request/response
	/// * `value` - Response value; a JSON String
	///
	/// # Returns
	///
	/// A boolean if the given PagingRequestUntil matches the value
	/// If this function returns true, this mostly means that a next page has to be requested
	///
	/// # Examples
	///
	/// ```
	/// PagingRequestUntil::None.check(200, String::from("")); // -> false
	/// PagingRequestUntil::Empty.check(200, String::from("")); // -> true
	/// PagingRequestUntil::Empty.check(200, String::from("{}")); // -> false
	/// PagingRequestUntil::StatusCode(200).check(200, String::from("")); // -> true
	/// PagingRequestUntil::StatusCode(202).check(200, String::from("")); // -> false
	/// PagingRequestUntil::EmptyValue(String::from("{{ $response/foo }}")).check(200, String::from("{ \"foo\":\"\" }")); // -> true
	/// PagingRequestUntil::EmptyValue(String::from("{{ $response/foo }}")).check(200, String::from("{ \"foo\":\"bar\" }")); // -> false
	/// PagingRequestUntil::Equals(String::from("{{ $response/foo }}"), String::from("{{ $response/bar }}")).check(200, String::from("{ \"foo\":\"bar\", \"bar\":\"bar\" }")); // -> true
	/// PagingRequestUntil::Equals(String::from("{{ $response/foo }}"), String::from("bar")).check(200, String::from("{ \"foo\":\"bar\", \"bar\":\"bar\" }")); // -> true
	/// PagingRequestUntil::Equals(String::from("bar"), String::from("{{ $response/foo }}")).check(200, String::from("{ \"foo\":\"bar\", \"bar\":\"bar\" }")); // -> true
	/// PagingRequestUntil::Equals(String::from("bar"), String::from("foo")).check(200, String::from("")); // -> false
	/// ```
	pub fn check(&self, status: u16, value: String) -> bool {
		// Some simple checks with an **early return** and without parsing the vaulue
		match self {
			Self::None => return true,
			Self::Empty => return value.is_empty(),
			Self::StatusCode(code) => return *code == status,
			_ => {}
		}

		// Try to parse the response as JSON
		let json = Arc::<Value>::new(serde_json::from_str(value.as_str()).unwrap_or_default());
		match self {
			// Parse JSON-Pointer value and compare to empty
			Self::EmptyValue(val) => template_string(&val, json.clone()).is_empty(),

			// Parse JSON-Pointer value and compare to the value
			Self::Equals(left, right) => {
				template_string(&left, json.clone()) == template_string(&right, json.clone())
			},

			// Anything else (should be handled above in the first match clause)
			_ => true
		}
	}
}

// Default-Wrapper Functions for Serde::Deserialize
fn default_method() -> Method { Method::GET }
fn default_cron_timer() -> String { String::from("* */5 * * * *") }


/// Parse a string into a Template and inserts it into the template cache
///
/// # Arguments
///
/// * `key` - The key under which the parsed template is stored
/// * `value` - value to use for the template
///
/// # Examples
///
/// ```
/// let tpl = String::from("this is {{ $uuid }} a templated {{ $response/foo/bar }} string");
/// config::template_string_parse(&tpl, &tpl);
/// let key = String::from("fixed_key");
/// config::template_string_parse(&key, &tpl);
/// ```
pub fn template_string_parse(key: &String, value: &String) {
	TEMPLATE_CACHE.insert(key.to_owned(), Template::parse(value));
}

/// Applies the JSON-Values to the given Template and returns the resulting string.
/// If there is no parsed template yet for the given string, parse the template.
///
/// # Arguments
///
/// * `tpl` - The Template-String (or key) of the Template
/// * `values` - The values to apply to the tempated string
///
/// # Returns
///
/// A String where all template params are applied
///
/// # Examples
///
/// ```
/// let response = Arc::new(
///     json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"bar"}, {"foo":"bar"} ] })
/// );
/// let tpl = String::from("this is {{ $uuid }} a templated {{ $response/foo/bar }} string");
/// let result = config::template_string(&tpl, Arc::clone(&response));
/// ```
pub fn template_string(tpl: &String, values: Arc<Value>) -> String {
	if let Some(template) = TEMPLATE_CACHE.get(tpl).or_else(|| {
		template_string_parse(tpl, tpl);
		TEMPLATE_CACHE.get(tpl)
	}) {
		let res = template.render(values.clone());
		return if res == "null" { "".to_string() } else { res }
	}

	warn!("Template-Cache for '{}' not found and not able to build. Using EMPTY String.", tpl);
	return String::from("");
}



#[cfg(test)]
pub mod test {
	use serde_json::json;

	use super::*;

	#[test]
	fn test_paging_request_none() {
		let paging = PagingRequestUntil::None;
		let result = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\" } }"));

		assert_eq!(result, true);
	}

	#[test]
	fn test_paging_request_empty() {
		let paging = PagingRequestUntil::Empty;
		let result_ok = paging.check(200, String::from(""));
		let result_nok = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\" } }"));

		assert_eq!(result_ok, true);
		assert_eq!(result_nok, false);
	}

	#[test]
	fn test_paging_request_satus() {
		let paging = PagingRequestUntil::StatusCode(200);
		let result_ok = paging.check(200, String::from(""));
		let result_nok = paging.check(404, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\" } }"));

		assert_eq!(result_ok, true);
		assert_eq!(result_nok, false);
	}

	#[test]
	fn test_paging_request_empty_value() {
		let paging = PagingRequestUntil::EmptyValue(String::from("{{ $response/paging/cursor }}"));
		let result_ok1  = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"\" } }"));
		let result_ok2  = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{  } }"));
		let result_null = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":null } }"));
		let result_nok  = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\" } }"));

		assert_eq!(result_ok1,  true);
		assert_eq!(result_ok2,  true);
		assert_eq!(result_null, true);
		assert_eq!(result_nok,  false);
	}

	#[test]
	fn test_paging_request_equal_value() {
		let paging_a = PagingRequestUntil::Equals(String::from("{{ $response/paging/cursor }}"), String::from("Paging-Cursor"));
		let paging_b = PagingRequestUntil::Equals(String::from("Paging-Cursor"), String::from("{{ $response/paging/cursor }}"));
		let paging_c = PagingRequestUntil::Equals(String::from("{{ $response/paging/cursor }}"), String::from("{{ $response/paging/last }}"));
		let paging_d = PagingRequestUntil::Equals(String::from("{{ $response/paging/cursor }}"), String::from("null"));

		let result_a_ok  = paging_a.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_a_nok = paging_a.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_b_ok  = paging_b.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_b_nok = paging_b.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_c_ok  = paging_c.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_c_nok = paging_c.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_d_ok  = paging_d.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":null, \"last\":\"Paging-Cursor\" } }"));
		let result_d_nok = paging_d.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"NotNull\", \"last\":\"Paging-Cursor\" } }"));

		assert_eq!(result_a_ok,  true);
		assert_eq!(result_a_nok, false);
		assert_eq!(result_b_ok,  true);
		assert_eq!(result_b_nok, false);
		assert_eq!(result_c_ok,  true);
		assert_eq!(result_c_nok, false);
		assert_eq!(result_d_ok,  true);
		assert_eq!(result_d_nok, false);
	}


	#[test]
	fn test_template_string_existing() {
		let response = Arc::new(
			json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"barrr"}, {"foo":"bar"} ] })
		);

		let key = String::from("test-key");
		let value = String::from("foo: {{ $response/data/1/foo }}");
		template_string_parse(&key,  &value);

		let result = template_string(&key, response.clone());

		assert_eq!(result, "foo: barrr");
	}

	#[test]
	fn test_template_string_new() {
		let response = Arc::new(
			json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"barrr"}, {"foo":"lastbar"} ] })
		);

		let key = String::from("dummy-key");
		TEMPLATE_CACHE.insert(key.to_owned(),  Template::parse("No real value"));

		let value = String::from("foo: {{ $response/data/2/foo }}");
		let result = template_string(&value, response.clone());

		assert_eq!(result, "foo: lastbar");
	}

}
