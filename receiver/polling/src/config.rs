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

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
	pub config: Polling,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Polling {
	pub name: String,
	pub api: Endpoint,

	#[serde(default = "default_cron_timer")]
	pub timer: String,

	#[serde(default)]
	pub queue: Queue,

	#[serde(default)]
	pub parser: Vec<Parser>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Endpoint {
	pub uri: String,

	#[serde(default)]
	pub body: Option<String>,

	#[serde(default = "default_method")]
	pub method: Method,

	#[serde(default)]
	pub auth: Option<Authentication>,

	#[serde(default)]
	pub header: Vec<Param>,

	#[serde(default)]
	pub paging: PagingReguest,
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Authentication {
	None,
	Basic { user: String, pass: String },
	Bearer(String),
	Header(Param),
}
impl fmt::Display for Authentication {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Authentication::None => write!(f, "None"),
			Authentication::Basic { user: u, pass: _ } => write!(f, "Basic '{}: ****'", u),
			Authentication::Bearer(_) => write!(f, "Bearer ****"),
			Authentication::Header(param) => write!(f, "Header '{}: ****'", param.name),
		}
	}
}

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PagingReguest {
	pub param: Param,

	pub until: Option<PagingRequestUntil>,

	#[serde(default)]
	pub timeout: u32,

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
		// Some simple checks without parsing the response first
		match self {
			Self::None => return true,
			Self::Empty => return value.is_empty(),
			Self::StatusCode(code) => return *code == status,
			_ => {}
		}

		// Try to parse the response as JSON
		let json = Arc::<Value>::new(serde_json::from_str(value.as_str()).unwrap_or_default());
		return match self {
			// Parse JSON-Pointer value and compare to empty
			Self::EmptyValue(val) => return template_string(&val, json.clone()).is_empty(),

			// Parse JSON-Pointer value and compare to the value
			Self::Equals(left, right) => return template_string(&left, json.clone()) == template_string(&right, json.clone()),

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
		return template.render(values.clone());
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
		let result_ok1 = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"\" } }"));
		let result_ok2 = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{  } }"));
		let result_nok = paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\" } }"));

		assert_eq!(result_ok1, true);
		assert_eq!(result_ok2, true);
		assert_eq!(result_nok, false);
	}

	#[test]
	fn test_paging_request_equal_value() {
		let paging_a = PagingRequestUntil::Equals(String::from("{{ $response/paging/cursor }}"), String::from("Paging-Cursor"));
		let paging_b = PagingRequestUntil::Equals(String::from("Paging-Cursor"), String::from("{{ $response/paging/cursor }}"));
		let paging_c = PagingRequestUntil::Equals(String::from("{{ $response/paging/cursor }}"), String::from("{{ $response/paging/last }}"));

		let result_a_ok  = paging_a.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_a_nok = paging_a.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_b_ok  = paging_b.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_b_nok = paging_b.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_c_ok  = paging_c.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));
		let result_c_nok = paging_c.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }"));

		assert_eq!(result_a_ok,  true);
		assert_eq!(result_a_nok, false);
		assert_eq!(result_b_ok,  true);
		assert_eq!(result_b_nok, false);
		assert_eq!(result_c_ok,  true);
		assert_eq!(result_c_nok, false);
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
