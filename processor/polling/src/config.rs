use core::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::types::{Authentication, Method, PagingRequest, PagingRequestUntil, Param};

impl fmt::Display for Method {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}

impl fmt::Display for Authentication {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Authentication::None => write!(f, "None"),
			Authentication::Basic { user, pass } => write!(f, "Basic '{}: {}****'", user, {
				if let Some(sub) = pass.get(0..4)
					&& sub == "file"
				{
					pass
				} else {
					pass.get(0..4).unwrap_or_default()
				}
			}),
			Authentication::Bearer(bearer) => write!(f, "Bearer {}****", {
				if let Some(sub) = bearer.get(0..4)
					&& sub == "file"
				{
					bearer
				} else {
					bearer.get(0..4).unwrap_or_default()
				}
			}),
			Authentication::Header { header, value } => write!(f, "Header '{}: {}****'", header, {
				if let Some(sub) = value.get(0..4)
					&& sub == "file"
				{
					value.as_str()
				} else {
					value.get(0..4).unwrap_or_default()
				}
			}),
		}
	}
}

impl fmt::Display for Param {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "[{:?}]='{:?}'", self.name, self.value)
	}
}

impl fmt::Display for PagingRequest {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(
			f,
			"PagingRequest: [param]={:?}; [timeout]={:?}; [max]={:?}; [until]={:?};",
			self.param, self.timeout, self.max_pages, self.until
		)
	}
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
	/// If this function returns true, this means that there is no next page to request
	///
	/// # Examples
	///
	/// ```ignore
	/// PagingRequestUntil::None.check(200, String::from("")); // -> true
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
			Self::EmptyValue(val) => {
				shared::template::template_string(val, json.clone()).is_empty()
			}

			// Parse JSON-Pointer value and compare to the value
			Self::Equals(left, right) => {
				tracing::debug!(
					message = "PAGING-CHECK",
					left = shared::template::template_string(left, json.clone()),
					right = shared::template::template_string(right, json.clone())
				);
				shared::template::template_string(left, json.clone())
					== shared::template::template_string(right, json.clone())
			}

			// Anything else (should be handled above in the first match clause)
			_ => true,
		}
	}
}

#[cfg(test)]
pub mod test {
	use super::*;

	const JSON_CURSOR: &str = "{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\" } }";

	#[test]
	fn test_paging_request_none() {
		let paging = PagingRequestUntil::None;

		assert!(paging.check(200, String::from(JSON_CURSOR)));
	}

	#[test]
	fn test_paging_request_empty() {
		let paging = PagingRequestUntil::Empty;

		assert!(paging.check(200, String::from("")));
		assert!(!paging.check(200, String::from(JSON_CURSOR)));
	}

	#[test]
	fn test_paging_request_satus() {
		let paging = PagingRequestUntil::StatusCode(200);

		assert!(paging.check(200, String::from("")));
		assert!(!paging.check(404, String::from(JSON_CURSOR)));
	}

	#[test]
	fn test_paging_request_empty_value() {
		let paging = PagingRequestUntil::EmptyValue(String::from("{{ $response/paging/cursor }}"));

		assert!(paging.check(
			200,
			String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"\" } }")
		));
		assert!(paging.check(200, String::from("{ \"foo\":\"bar\",\"paging\":{  } }")));
		assert!(paging.check(
			200,
			String::from("{ \"foo\":\"bar\",\"paging\":{ \"cursor\":null } }")
		));
		assert!(!paging.check(200, String::from(JSON_CURSOR)));
	}

	#[test]
	fn test_paging_request_empty_value_no_json() {
		let paging = PagingRequestUntil::EmptyValue(String::from("{{ $response/paging/cursor }}"));

		// A response which is not JSON has no value, therefore it is "empty"
		assert!(paging.check(200, String::from("this is not json")));
	}

	#[test]
	fn test_paging_request_equal_value() {
		let json = "{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"Paging-Cursor\", \"last\":\"Paging-Cursor\" } }";
		let json_diff = "{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"No-Paging-Cursor\", \"last\":\"Paging-Cursor\" } }";
		let json_null =
			"{ \"foo\":\"bar\",\"paging\":{ \"cursor\":null, \"last\":\"Paging-Cursor\" } }";
		let json_not_null =
			"{ \"foo\":\"bar\",\"paging\":{ \"cursor\":\"NotNull\", \"last\":\"Paging-Cursor\" } }";

		let paging_a = PagingRequestUntil::Equals(
			String::from("{{ $response/paging/cursor }}"),
			String::from("Paging-Cursor"),
		);
		let paging_b = PagingRequestUntil::Equals(
			String::from("Paging-Cursor"),
			String::from("{{ $response/paging/cursor }}"),
		);
		let paging_c = PagingRequestUntil::Equals(
			String::from("{{ $response/paging/cursor }}"),
			String::from("{{ $response/paging/last }}"),
		);
		let paging_d = PagingRequestUntil::Equals(
			String::from("{{ $response/paging/cursor }}"),
			String::from("null"),
		);

		assert!(paging_a.check(200, String::from(json)));
		assert!(!paging_a.check(200, String::from(json_diff)));
		assert!(paging_b.check(200, String::from(json)));
		assert!(!paging_b.check(200, String::from(json_diff)));
		assert!(paging_c.check(200, String::from(json)));
		assert!(!paging_c.check(200, String::from(json_diff)));
		assert!(paging_d.check(200, String::from(json_null)));
		assert!(!paging_d.check(200, String::from(json_not_null)));
	}

	#[test]
	fn test_display_masks_secrets() {
		let basic = Authentication::Basic {
			user: String::from("user"),
			pass: String::from("supersecret"),
		};
		let bearer = Authentication::Bearer(String::from("supersecret"));
		let header = Authentication::Header {
			header: String::from("X-Key"),
			value: String::from("supersecret"),
		};

		assert_eq!(basic.to_string(), "Basic 'user: supe****'");
		assert_eq!(bearer.to_string(), "Bearer supe****");
		assert_eq!(header.to_string(), "Header 'X-Key: supe****'");
		assert_eq!(Authentication::None.to_string(), "None");
	}

	#[test]
	fn test_display_short_secrets() {
		// Values shorter than 4 chars must not panic
		assert_eq!(
			Authentication::Bearer(String::from("ab")).to_string(),
			"Bearer ****"
		);
		assert_eq!(
			Authentication::Basic {
				user: String::new(),
				pass: String::new(),
			}
			.to_string(),
			"Basic ': ****'"
		);
	}

	#[test]
	fn test_display_method() {
		assert_eq!(Method::GET.to_string(), "GET");
		assert_eq!(Method::OPTION.to_string(), "OPTION");
	}
}
