use std::collections::HashMap;


#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateToken {
	Static(String),
	Param(String),
}

#[derive(Clone, Debug)]
pub struct Template {
	pub params: Vec<String>,
	tokens: Vec<TemplateToken>,
	capacity: usize,
}

impl Template {
	pub fn parse(s: &str) -> Self {
		let mut tokens = Vec::new();
		let mut params = Vec::new();
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
						let param_token = String::from(&s[pos..last_end]);
						if !param_token.is_empty() {
							params.push(param_token.clone());
							tokens.push(TemplateToken::Param( param_token ));
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
			params,
			tokens,
			capacity: (s.len() + (num_param * 24)) // Predict each param is max 24 chars long
		};
	}

	pub fn render(&self, values: HashMap<String, String>) -> String {
		let mut out = String::with_capacity(self.capacity);
		for token in &self.tokens {
			match token {
				// Append the static string value
				TemplateToken::Static(val) => {
					out.push_str(&val)
				},
				// Append the 'PARAM-Value' or '{{PARAM}}' if no value is set
				TemplateToken::Param(val) => {
					if let Some(v) = values.get(val.as_str()) {
						out.push_str(v);
					} else {
						// Escaping of { is {{ -> {{ is {{{{
						out.push_str(&format!("{{{{{}}}}}", val));
					}
				}
			}
		}
		return out;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_render() {
		let tpl = Template::parse("Param:{{PARAM_A}}; Param:{{PARAM_B}}; NoValue:{{PARAM_C}}");
		let mut params = HashMap::new();
		params.insert(String::from("PARAM_A"), String::from("Foo"));
		params.insert(String::from("PARAM_B"), String::from("Bar"));
		params.insert(String::from("PARAM_X"), String::from("FooBar"));

		let res = tpl.render(params);
		assert_eq!(res, String::from("Param:Foo; Param:Bar; NoValue:{{PARAM_C}}"));
	}

	#[test]
	fn test_parse_inner() {
		let s = String::from("foo {{PARAM1}} bar {{PARAM2}} end");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 5);
		assert_eq!(result.tokens[0], TemplateToken::Static(String::from("foo ")));
		assert_eq!(result.tokens[1], TemplateToken::Param(String::from("PARAM1")));
		assert_eq!(result.tokens[2], TemplateToken::Static(String::from(" bar ")));
		assert_eq!(result.tokens[3], TemplateToken::Param(String::from("PARAM2")));
		assert_eq!(result.tokens[4], TemplateToken::Static(String::from(" end")));
	}

	#[test]
	fn test_parse_start_and_end() {
		let s = String::from("{{PARAM1}} bar {{PARAM2}}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 3);
		assert_eq!(result.tokens[0], TemplateToken::Param(String::from("PARAM1")));
		assert_eq!(result.tokens[1], TemplateToken::Static(String::from(" bar ")));
		assert_eq!(result.tokens[2], TemplateToken::Param(String::from("PARAM2")));
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
		assert_eq!(result.tokens[0], TemplateToken::Param(String::from("PARAM")));
	}

	#[test]
	fn test_parse_only_params() {
		let s = String::from("{{PARAM1}}{{PARAM2}}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 2);
		assert_eq!(result.tokens[0], TemplateToken::Param(String::from("PARAM1")));
		assert_eq!(result.tokens[1], TemplateToken::Param(String::from("PARAM2")));
	}

	#[test]
	fn test_parse_messy_end() {
		let s = String::from("{{PARAM1}} foo }}");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 2);
		assert_eq!(result.tokens[0], TemplateToken::Param(String::from("PARAM1")));
		assert_eq!(result.tokens[1], TemplateToken::Static(String::from(" foo }}")));
	}

	#[test]
	fn test_parse_messy_start() {
		let s = String::from("{{PARAM1 {{TEST}} foo");
		let result = Template::parse(&s);

		assert_eq!(result.tokens.len(), 2);
		assert_eq!(result.tokens[0], TemplateToken::Param(String::from("PARAM1 {{TEST")));
		assert_eq!(result.tokens[1], TemplateToken::Static(String::from(" foo")));
	}
}
