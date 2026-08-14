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
				if let Some(sub) = pass.get(0..4) && sub == "file" { pass } else { pass.get(0..4).unwrap_or_default() }
			}),
			Authentication::Bearer(bearer) => write!(f, "Bearer {}****", {
				if let Some(sub) = bearer.get(0..4) && sub == "file" { bearer } else { bearer.get(0..4).unwrap_or_default() }
			}),
			Authentication::Header(param) => write!(f, "Header '{}: {}****'", param.name, {
				if let Some(sub) = param.value.get(0..4) && sub == "file" { param.value.as_str() } else { param.value.get(0..4).unwrap_or_default() }
			}),
		}
	}
}

impl fmt::Display for Param {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "[{:?}]='{:?}'", self.name, self.value)
	}
}

impl Default for PagingRequest {
	fn default() -> Self {
		Self { param: None, until: None, timeout: 3600, max_pages: 1 }
	}
}
impl fmt::Display for PagingRequest {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		write!(f, "PagingRequest: [param]={:?}; [timeout]={:?}; [max]={:?}; [until]={:?};", self.param, self.timeout, self.max_pages, self.until.as_ref().unwrap_or(&PagingRequestUntil::None))
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
			Self::EmptyValue(val) => shared::template::template_string(&val, json.clone()).is_empty(),

			// Parse JSON-Pointer value and compare to the value
			Self::Equals(left, right) => {
				tracing::debug!(
					message = "PAGING-CHECK",
					left = shared::template::template_string(&left, json.clone()),
					right = shared::template::template_string(&right, json.clone())
				);
				shared::template::template_string(&left, json.clone()) == shared::template::template_string(&right, json.clone())
			},

			// Anything else (should be handled above in the first match clause)
			_ => true
		}
	}
}


#[cfg(test)]
pub mod test {
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

}
