use core::fmt;
use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use jiff::{Timestamp, Zoned, tz::TimeZone};
use serde_json::Value;
use tracing::warn;
use uuid::Uuid;

/// Static Lazy-Loaded template cache
static TEMPLATE_CACHE: Lazy<DashMap<String, Template>> = Lazy::new(|| DashMap::new());

// ISO-8601 Date-Format in UTC
// "%+" is equal to "%FT%T%.6f%:z" is equal to "%Y-%m-%dT%H:%M:%S%.6f%:z"
// However, "%+" throws errors...
// See https://docs.rs/chrono/latest/chrono/format/strftime/index.html
static FORMAT_ISO8601: &str = "%FT%T%.6f%:z";

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
/// shared::template::template_string_parse(&tpl, &tpl);
/// let key = String::from("fixed_key");
/// shared::template::template_string_parse(&key, &tpl);
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
/// let response = std::sync::Arc::new(
///     serde_json::json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"bar"}, {"foo":"bar"} ] })
/// );
/// let tpl = String::from("this is {{ $uuid }} a templated {{ $response/foo/bar }} string");
/// let result = shared::template::template_string(&tpl, std::sync::Arc::clone(&response));
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

/// Represents the Template-Parsing engine.
#[derive(Clone, Debug)]
pub struct Template {
	/// All parsed tokens in order of occurence.
	tokens: Vec<TemplateToken>,

	// The calculated capacity for the final string.
	capacity: usize,
}
impl Template {
	/// Parse a string into static string and parameter pre-evaluated parameter tokens.
	///
	/// ## Valid tokens are:
	///
	/// * `$uuid` - A simple UUID-v4.
	/// * `$now(FORMAT)` - The current Date/Time. If `FORMAT` is not given, the value `%Y-%m-%d` is used.
	/// * `$date(DATE#FORMAT)` - The Date/Time value given in `DATE` or the current DateTime value (now). If `FORMAT` is not given, the value `%Y-%m-%d` is used.
	/// * `$response/JSON/POINTER` - A JSON-Pointer value which is evaluated against the value given in `render(value)`. The format is `/key/key` for objects and in case of array indexes like `/key/0/key/33`.
	///
	/// Every Token is encapsulated in `{{ ... }}`.
	///
	///
	/// # Arguments
	///
	/// * `s` - The string to parse
	///
	/// # Returns
	///
	/// A `Template` instance.
	///
	/// # Example
	///
	/// ```
	/// let uuid = shared::template::Template::parse("UUID: {{ $uuid }}");
	/// let resp = shared::template::Template::parse("Response: {{ $response/one/two/three }};");
	/// let now  = shared::template::Template::parse("Now: {{ $now }}; Formatted: {{ $now(%d-%m-%Y) }}");
	/// let date = shared::template::Template::parse("Date: {{ $date(2010-11-12) }}; Formatted: {{ $date(2010-11-12#%d-%m-%Y) }};");
	/// ```
	pub fn parse(s: &str) -> Self {
		let mut tokens = Vec::new();
		let mut iter = s.chars();
		let mut last_end: usize = 0;
		let mut num_param: usize = 0;

		// Start Position of '{{'
		while let Some(mut pos) = iter.position(|x| x == '{') {
			if let Some(pos_end) = iter.next() && pos_end == '{' {
				// End Position of '}}'
				while let Some(end) = iter.position(|x| x == '}') {
					if let Some(end_end) = iter.next() && end_end == '}' {
						// Update start position
						pos = last_end + pos;

						// save static string
						let static_token = String::from(&s[last_end..pos]);
						if !static_token.is_empty() {
							tokens.push(TemplateToken::Static( static_token ));
						}

						// Update Param-End Position, which is before the '}}'
						pos = pos + 2;
						last_end = pos + end;

						// save param
						let param_token = String::from(s[pos..last_end].trim());
						if !param_token.is_empty() {
							tokens.push(TemplateToken::Param( ParamParser::from(param_token) ));
						}

						// Update next start position, which is after '}}'
						last_end = last_end + 2;

						// break the inner loop to find the next '{{' position within the outer loop
						num_param += 1;
						break;
					}
				}
			}
		}

		// Add the last static string
		if last_end < s.len() {
			tokens.push(TemplateToken::Static( String::from(&s[last_end..]) ));
		}

		return Self {
			tokens,
			capacity: (s.len() + (num_param * 24)) // Predict each param is max 24 chars long
		};
	}

	/// Renders the Template based on the JSON-Value.
	///
	/// # Arguments
	///
	/// * `value` - A serde_json::Value which is used for the `$response` tokens
	///
	/// # Returns
	///
	/// The Template-String given in `parse(s)` where all `{{ ... }}` token parameters are replaced.
	///
	/// # Example
	///
	/// ```
	/// let params = std::sync::Arc::new(serde_json::Value::Null);
	///
	/// let uuid = shared::template::Template::parse("UUID: {{ $uuid }}");
	/// println!("Parsed: {}", uuid.render(params.clone()));
	///
	/// let json = std::sync::Arc::new(serde_json::json!({"one":{"two":{"three":"Foo Bar"}}}));
	/// let resp = shared::template::Template::parse("Response: {{ $response/one/two/three }};");
	/// println!("Parsed: {}", resp.render(json.clone()));
	///
	/// let now  = shared::template::Template::parse("Now: {{ $now }}; Formatted: {{ $now(%d-%m-%Y) }}");
	/// println!("Parsed: {}", now.render(params.clone()));
	///
	/// let date = shared::template::Template::parse("Date: {{ $date(2010-11-12) }}; Formatted: {{ $date(2010-11-12#%d-%m-%Y) }};");
	/// println!("Parsed: {}", date.render(params.clone()));
	///
	/// ```
	pub fn render(&self, value: Arc<Value>) -> String {
		let mut out = String::with_capacity(self.capacity);
		for token in &self.tokens {
			match token {
				// Append the static string value
				TemplateToken::Static(val) => {
					out.push_str(&val)
				},
				// Append the 'PARAM-Value' or '{{PARAM}}' if no value is set
				TemplateToken::Param(val) => {
					let v = match &val {
						// UUIDs
						&ParamParser::Uuid => {
							let uuid = Uuid::new_v4().to_string();
							&format!("{}", uuid)
						},

						// Current Date-Time Values
						&ParamParser::Now(format) => {
							let formatted = if format.is_empty() {
								Zoned::now().with_time_zone(TimeZone::UTC).strftime("%Y-%m-%d").to_string()
							} else {
								Zoned::now().with_time_zone(TimeZone::UTC).strftime(if format == "iso8601" {FORMAT_ISO8601} else {format}).to_string()
							};
							&format!("{}", formatted)
						},

						// Any DateTime formats, including relative dates like "-5 days"
						&ParamParser::Date(d, format) => {
							let date = if d == "$now" {
								Zoned::now().with_time_zone(TimeZone::UTC)

							} else {
								// Check the result for a DateTime value, parse it and use the UTC value
								let date_val = if d.get(0..4).unwrap_or_default() == "$res" {
									// TODO: implement $date($response/foo/bar#%Y-%m-%d)
									// Enable `test_render_date_result()` afterwards
									d.to_owned()
								} else {
									d.to_owned()
								};

								// Try to parse a normal date in one of the most common formats,
								// if this fails, try to parse it as a relative date format like "-5 days"
								match dateparser::parse(&date_val) {
									Ok(d) => {
										let ts: Timestamp = d.to_rfc3339().parse().unwrap_or_default();
										ts.to_zoned(TimeZone::UTC)
									},
									Err(_) => {
										match parse_datetime::parse_datetime(&date_val) {
											Ok(p) => p.into_zoned().unwrap_or_default().with_time_zone(TimeZone::UTC),
											Err(_) => Zoned::now().with_time_zone(TimeZone::UTC)
										}
									}
								}
							};

							let formatted = if format.is_empty() {
								date.strftime("%Y-%m-%d").to_string()
							} else {
								date.strftime(if format == "iso8601" {FORMAT_ISO8601} else {format}).to_string()
							};
							&format!("{}", formatted)
						},

						// Value from the last response
						&ParamParser::Response(json_path) => {
							let v = match &value.pointer(json_path.as_str()).unwrap_or(&Value::Null) {
								&Value::Bool(v) => format!("{}", v),
								&Value::Number(v) => format!("{}", v.as_f64().unwrap_or(0.0)),
								&Value::String(v) => if v == "null" { "".to_string() } else { v.clone() },
								_ => String::new()
							};
							&format!("{}", v)
						},

						// Just a static value
						&ParamParser::Static(token) => {
							&format!("{{{{{}}}}}", token)
						},
					};
					out.push_str(v);
				}
			}
		}
		return out;
	}
}


/// Represents a static string or a token which has to be evaluated
#[derive(Clone, Debug, PartialEq, Eq)]
enum TemplateToken {
	/// A static string which is show 'as is'
	Static(String),

	// A Parameter which is parsed and evaluated on runtime
	Param(ParamParser),
}

/// Represents a special token which is evaluated/parsed when shown/called
#[derive(Clone, Debug, PartialEq, Eq)]
enum ParamParser {
	/// A simple UUID-v4
	Uuid,

	/// A JSON-Pointer value
	Response(String),

	/// Current Date/Time in UTC, formatted by the given format-string
	Now(String),

	/// A predefined (or dynamic) date value, formatted by the given format-string
	Date(String, String),

	/// Just a static string value
	Static(String),
}
impl ParamParser {
	/// Checks the given String and returns an appropriate ParamValue
	///
	/// # Arguments
	///
	/// * `val` - The String to parse
	///
	/// # Returns
	///
	/// * A ParamValue which represents the given token
	///
	/// # Examples
	///
	/// * `$uuid` - Returns a `ParamValue::Uuid`
	/// * `$now` - Returns a `ParamValue::Now` with the default Date-Representaiton
	/// * `$now(%m-%d-%Y)` - Returns a `ParamValue::Now` with the given Date-Representaiton
	/// * `$date(2020-01-02)` - Returns a `ParamValue::Date` with the default Date-Representaiton
	/// * `$date(2020-01-02#%m-%d-%Y)` - Returns a `ParamValue::Date` with the given Date-Representaiton
	/// * `$response/one/two/three` - Returns a `ParamValue::Response` and the JSON-Pointer to `/one/two/three`
	///
	/// # Helpful References
	///
	/// * [JSON-Pointer RFC6901](https://datatracker.ietf.org/doc/html/rfc6901)
	/// * [Date Time Formatting](https://docs.rs/chrono/latest/chrono/format/strftime/index.html)
	pub fn from(val: String) -> Self {
		return match &val[..4] {
			// A simple UUID
			"$uui" => Self::Uuid,

			// $now
			// $now(%Y-%m-%dT%H:%M:%sZ)
			// See https://docs.rs/chrono/latest/chrono/format/strftime/index.html
			"$now" => {
				let s = val.find('(').unwrap_or(0);
				let e = val.rfind(')').unwrap_or(0);
				if s > 0 && e > s {
					Self::Now(val[s+1..e].to_string())
				} else {
					Self::Now(String::new())
				}
			},

			// $date(2006-05-04T15:16:17.0001)
			// $date(2006-05-04T15:16:17.0001#%Y-%m-%dT%H:%M:%sZ)
			// See https://docs.rs/chrono/latest/chrono/format/strftime/index.html
			"$dat" if val.len() > 8 => {
				let s = val.find('(').unwrap_or(0);
				let p = val.rfind('#').unwrap_or(0);
				let e = val.rfind(')').unwrap_or(0);

				// Date-String with a Format string
				let (date, format) = if s > 0 && p > s && e > p {
					(val[s+1..p].to_string(), val[p+1..e].to_string())
				// Only a Date-String
				} else if s > 0 && e > s {
					(val[s+1..e].to_string(), String::new())
				// No Date-String
				} else {
					("$now".to_string(), String::new())
				};
				Self::Date(date, format)
			},

			// $response/FIELD/INDEX/FIELD
			// $response/RFC6901
			// See https://datatracker.ietf.org/doc/html/rfc6901
			"$res" if val.len() > 9 => Self::Response(val[9..].to_string()),

			// Undefined is just a static value
			_ => Self::Static(val),
		}
	}
}
impl fmt::Display for ParamParser {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{:?}", self)
	}
}


#[cfg(test)]
mod tests {
	use std::sync::Arc;
	use jiff::ToSpan;
	use serde_json::json;

	use super::*;

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

	#[test]
	fn test_render_none() {
		let tpl = Template::parse("Param:{{ $PARAM_A }}; Param:{{ PARAM_B }}; NoValue:{{PARAM_C}}");
		let params = Arc::new(Value::Null);

		let res = tpl.render(params);
		assert_eq!(res, String::from("Param:{{$PARAM_A}}; Param:{{PARAM_B}}; NoValue:{{PARAM_C}}"));
	}

	#[test]
	fn test_render_dates() {
		let tpl = Template::parse("Now:{{ $now }}; dmY:{{ $now(%d-%m-%Y) }}; Date:{{ $date(2008-10-12) }}; Date-dmY:{{ $date(2008-10-12#%d-%m-%Y) }};");
		let params = Arc::new(Value::Null);
		let now = Zoned::now().with_time_zone(TimeZone::UTC);

		let res = tpl.render(params);
		assert_eq!(res, String::from(format!("Now:{}; dmY:{}; Date:2008-10-12; Date-dmY:12-10-2008;", now.strftime("%Y-%m-%d"), now.strftime("%d-%m-%Y"))));
	}

	#[test]
	fn test_render_relative_dates() {
		let tpl = Template::parse("Relative:{{ $date(-5days) }}; Date-dmY:{{ $date(-5days#%d-%m-%Y) }};");
		let params = Arc::new(Value::Null);
		let date = Zoned::now().saturating_sub(5.days()).with_time_zone(TimeZone::UTC);

		let res = tpl.render(params);
		assert_eq!(res, String::from(format!("Relative:{}; Date-dmY:{};", date.strftime("%Y-%m-%d"), date.strftime("%d-%m-%Y"))));
	}

	#[test]
	fn test_render_iso8601_dates() {
		let tpl = Template::parse("ISO-8601:{{ $date(#iso8601) }};");
		let params = Arc::new(Value::Null);
		let date = Zoned::now().with_time_zone(TimeZone::UTC);

		let res = tpl.render(params);
		// Do not check seconds-fraction '%.6f' which will never be the same anyways - even seconds is critical to test
		assert_eq!(res.get(0..29), String::from(format!("ISO-8601:{}", date.strftime("%FT%T%.6f%:z"))).get(0..29));
	}

	#[test]
	#[ignore]
	fn test_render_date_result() {
		let tpl = Template::parse("Response:{{ $date($response/data/foo) }}; Date-dmY:{{ $date($response/data/foo#%d-%m-%Y) }};");
		let params = Arc::new(
			json!({ "paging":{"cursor":"xxx","pages":77}, "data":{ "foo": "2008-10-12 10:12:14" } })
		);

		let res = tpl.render(params);
		assert_eq!(res, String::from("Response:2008-10-12; Date-dmY: 12-10-2008"));
	}

	#[test]
	fn test_render_uuid() {
		let tpl = Template::parse("Now:{{ $uuid }};");
		let params = Arc::new(Value::Null);

		let res = tpl.render(params);
		assert_eq!(&res[..4], "Now:");
		assert_eq!(res.chars().last().unwrap_or_default(), ';');
		assert_eq!(res.len(), 41); // "Now:UUID4;" --> 4 (Now:) + 32 (Chars) + 4 (hyphens) + 1 (;) = 41
	}

	#[test]
	fn test_render_result() {
		let tpl = Template::parse("foo: {{ $response/data/0/foo }}; cursor: {{ $response/paging/cursor }}");
		let params = Arc::new(
			json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"bar"}, {"foo":"bar"} ] })
		);

		let res = tpl.render(params);
		assert_eq!(res, String::from("foo: bar; cursor: xxx"));
	}

	#[test]
	fn test_render_no_result() {
		let tpl = Template::parse("foo: {{ $response/data/foo }}; cursor: {{ $response/paging/cursor }}");
		let params = Arc::new(
			json!({ "paging":{"cursor":"xxx","pages":77}, "data":[ {"foo":"bar"}, {"foo":"bar"}, {"foo":"bar"} ] })
		);

		let res = tpl.render(params);
		assert_eq!(res, String::from("foo: ; cursor: xxx"));
	}


	#[test]
	fn test_parse_inner() {
		let s = String::from("foo {{PARAM1}} bar {{PARAM2}} end");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 5);
		assert_eq!(result.tokens[0], TemplateToken::Static(String::from("foo ")));
		assert_eq!(result.tokens[1], TemplateToken::Param(ParamParser::Static("PARAM1".to_string())));
		assert_eq!(result.tokens[2], TemplateToken::Static(String::from(" bar ")));
		assert_eq!(result.tokens[3], TemplateToken::Param(ParamParser::Static("PARAM2".to_string())));
		assert_eq!(result.tokens[4], TemplateToken::Static(String::from(" end")));
	}

	#[test]
	fn test_parse_start_and_end() {
		let s = String::from("{{PARAM1}} bar {{PARAM2}}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 3);
		assert_eq!(result.tokens[0], TemplateToken::Param(ParamParser::Static("PARAM1".to_string())));
		assert_eq!(result.tokens[1], TemplateToken::Static(String::from(" bar ")));
		assert_eq!(result.tokens[2], TemplateToken::Param(ParamParser::Static("PARAM2".to_string())));
	}

	#[test]
	fn test_parse_none() {
		let s = String::from("foo bar");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 1);
		assert_eq!(result.tokens[0], TemplateToken::Static(String::from("foo bar")));
	}

	#[test]
	fn test_parse_only_param() {
		let s = String::from("{{PARAM}}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 1);
		assert_eq!(result.tokens[0], TemplateToken::Param(ParamParser::Static("PARAM".to_string())));
	}

	#[test]
	fn test_parse_only_params() {
		let s = String::from("{{PARAM1}}{{PARAM2}}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 2);
		assert_eq!(result.tokens[0], TemplateToken::Param(ParamParser::Static("PARAM1".to_string())));
		assert_eq!(result.tokens[1], TemplateToken::Param(ParamParser::Static("PARAM2".to_string())));
	}

	#[test]
	fn test_parse_messy_end() {
		let s = String::from("{{PARAM1}} foo }}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 2);
		assert_eq!(result.tokens[0], TemplateToken::Param(ParamParser::Static("PARAM1".to_string())));
		assert_eq!(result.tokens[1], TemplateToken::Static(String::from(" foo }}")));
	}

	#[test]
	fn test_parse_messy_start() {
		let s = String::from("{{PARAM1 {{TEST}} foo");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 2);
		assert_eq!(result.tokens[0], TemplateToken::Param(ParamParser::Static("PARAM1 {{TEST".to_string())));
		assert_eq!(result.tokens[1], TemplateToken::Static(String::from(" foo")));
	}
}
