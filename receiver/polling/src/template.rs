use std::{fmt, sync::Arc};

use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

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
	/// let uuid = Template::parse("UUID: {{ $uuid }}");
	/// let resp = Template::parse("Response: {{ $response/one/two/three }};");
	/// let now  = Template::parse("Now: {{ $now }}; Formatted: {{ $now(%d-%m-%Y) }}");
	/// let date = Template::parse("Date: {{ $date(2010-11-12) }}; Formatted: {{ $date(2010-11-12#%d-%m-%Y) }};");
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
	/// let params = Arc::new(Value::Null);
	///
	/// let uuid = Template::parse("UUID: {{ $uuid }}");
	/// println!("Parsed: {}", uuid.parse(params.clone()));
	///
	/// let json = Arc::new(json!({"one":{"two":{"three":"Foo Bar"}}}));
	/// let resp = Template::parse("Response: {{ $response/one/two/three }};");
	/// println!("Parsed: {}", resp.parse(json.clone()));
	///
	/// let now  = Template::parse("Now: {{ $now }}; Formatted: {{ $now(%d-%m-%Y) }}");
	/// println!("Parsed: {}", now.parse(params.clone()));
	///
	/// let date = Template::parse("Date: {{ $date(2010-11-12) }}; Formatted: {{ $date(2010-11-12#%d-%m-%Y) }};");
	/// println!("Parsed: {}", date.parse(params.clone()));
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
						&ParamParser::Uuid => {
							let uuid = Uuid::new_v4().to_string();
							&format!("{}", uuid)
						},

						&ParamParser::Now(format) => {
							let formatted = if format.is_empty() {
								Utc::now().format("%Y-%m-%d").to_string()
							} else {
								Utc::now().format(format).to_string()
							};
							&format!("{}", formatted)
						},

						&ParamParser::Date(d, format) => {
							let date = if d == "$now" {
								Utc::now()
							} else {
								dateparser::parse(d).unwrap_or_default().with_timezone(&Utc)
							};
							let formatted = if format.is_empty() {
								date.format("%Y-%m-%d").to_string()
							} else {
								date.format(format).to_string()
							};
							&format!("{}", formatted)
						},

						&ParamParser::Response(json_path) => {
							&format!("{}", value.pointer(json_path.as_str()).unwrap_or(&Value::Null).as_str().unwrap_or_default())
						},

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
	use serde_json::json;

use super::*;

	#[test]
	fn test_render_none() {
		let tpl = Template::parse("Param:{{ $PARAM_A }}; Param:{{ PARAM_B }}; NoValue:{{PARAM_C}}");
		let params = Arc::new(Value::Null);

		let res = tpl.render(params);
		assert_eq!(res, String::from("Param:{{$PARAM_A}}; Param:{{PARAM_B}}; NoValue:{{PARAM_C}}"));
	}

	#[test]
	fn test_render_dates() {
		let tpl = Template::parse("Now:{{ $now }}; dmY:{{ $now(%d-%m-%Y) }}; Date:{{ $date(2010-11-12) }}; Date-dmY:{{ $date(2010-11-12#%d-%m-%Y) }};");
		let params = Arc::new(Value::Null);
		let now = Utc::now();

		let res = tpl.render(params);
		assert_eq!(res, String::from(format!("Now:{}; dmY:{}; Date:2010-11-12; Date-dmY:12-11-2010;", now.format("%Y-%m-%d"), now.format("%d-%m-%Y"))));
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
