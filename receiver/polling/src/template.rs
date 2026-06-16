use std::{fmt, sync::Arc};

use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateToken {
	Static(String),
	Param(ParamParser),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParamParser {
	Uuid,
	Response(String),
	Now(String),
	Date(String, String),
	Static(String),
}
impl ParamParser {
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

#[derive(Clone, Debug)]
pub struct Template {
	tokens: Vec<TemplateToken>,
	capacity: usize,
}

impl Template {
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
