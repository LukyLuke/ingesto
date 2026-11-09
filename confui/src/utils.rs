pub fn set_panic_hook() {
	// When the `console_error_panic_hook` feature is enabled, we can call the
	// `set_panic_hook` function at least once during initialization, and then
	// we will get better error messages if our code ever panics.
	//
	// For more details see
	// https://github.com/rustwasm/console_error_panic_hook#readme
	#[cfg(feature = "console_error_panic_hook")]
	console_error_panic_hook::set_once();
}

/// Escapes a string to be safely used in HTML text and in quoted attributes
pub fn esc(input: &str) -> String {
	let mut out = String::with_capacity(input.len());
	for c in input.chars() {
		match c {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\'' => out.push_str("&#39;"),
			c => out.push(c),
		}
	}
	out
}

/// Convert snake_case to Title Case
pub fn title_case(input: &str) -> String {
	input.split('_')
		.filter(|s| !s.is_empty())
		.map(|s| {
			let mut c = s.chars();
			match c.next() {
				None => String::new(),
				Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
			}
		})
		.collect::<Vec<String>>()
		.join(" ")
}

/// Appends a segment to a path: `("config.listen", "port")` -> `config.listen.port`
pub fn join_path(parent: &str, segment: &str) -> String {
	if parent.is_empty() {
		segment.to_owned()
	} else {
		format!("{parent}.{segment}")
	}
}

/// Splits a path into it's segments
pub fn split_path(path: &str) -> Vec<&str> {
	path.split('.').filter(|s| !s.is_empty()).collect()
}

/// A label for the last segment of a path: `config.parser.0` -> `Parser #1`, `config.max_size` -> `Max Size`
pub fn label_for(path: &str) -> String {
	let segments = split_path(path);
	match segments.as_slice() {
		[.., parent, last] if last.parse::<usize>().is_ok() => {
			let index: usize = last.parse().unwrap_or_default();
			format!("{} #{}", title_case(parent), index.saturating_add(1))
		},
		[.., last] if last.parse::<usize>().is_ok() => format!("Item #{}", last.parse::<usize>().unwrap_or_default().saturating_add(1)),
		[.., last] => title_case(last),
		[] => String::new(),
	}
}

/// A readable version of a whole path: `config.parser.0.name` -> `Parser #1 › Name`
pub fn readable_path(path: &str) -> String {
	let mut segments = split_path(path);
	if segments.len() > 1 && segments[0] == "config" {
		segments.remove(0);
	}

	let mut parts: Vec<String> = Vec::new();
	for segment in segments {
		match (segment.parse::<usize>(), parts.last_mut()) {
			(Ok(index), Some(last)) => last.push_str(&format!(" #{}", index.saturating_add(1))),
			_ => parts.push(title_case(segment)),
		}
	}
	parts.join(" › ")
}

/// Calls `f` with the numeric index and the rest of the path if `path` is inside the list `list_path`
/// Used to shift paths after an item of a list was removed.
pub fn list_index_in(path: &str, list_path: &str) -> Option<(usize, String)> {
	let rest = path.strip_prefix(list_path)?.strip_prefix('.')?;
	let (index, tail) = match rest.split_once('.') {
		Some((index, tail)) => (index, format!(".{tail}")),
		None => (rest, String::new()),
	};
	Some((index.parse().ok()?, tail))
}

/// Quotes a value for the use in a CSS attribute selector: `[data-path="VALUE"]`
pub fn attr_selector(attribute: &str, value: &str) -> String {
	let mut quoted = String::with_capacity(value.len());
	for c in value.chars() {
		match c {
			'\\' | '"' => {
				quoted.push('\\');
				quoted.push(c);
			},
			'\n' => quoted.push_str("\\a "),
			c => quoted.push(c),
		}
	}
	format!("[{attribute}=\"{quoted}\"]")
}

/// Moves the paths of open elements after the entry `removed` was removed from the list `list_path`:
/// Paths inside the removed entry are dropped, those behind it are shifted by one.
pub fn shift_after_remove(paths: Vec<String>, list_path: &str, removed: usize) -> Vec<String> {
	paths
		.into_iter()
		.filter_map(|path| match list_index_in(&path, list_path) {
			Some((index, _)) if index == removed => None,
			Some((index, tail)) if index > removed => Some(format!("{list_path}.{}{tail}", index - 1)),
			_ => Some(path),
		})
		.collect()
}

/// The name of the file to download: The name of the loaded file with the extension of the chosen format
pub fn download_name(file_name: &str, extension: &str) -> String {
	let stem = match file_name.rsplit_once('.') {
		Some((stem, _)) => stem,
		None => file_name,
	};
	let stem = if stem.trim().is_empty() { "config" } else { stem };
	format!("{stem}.{extension}")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn esc_handles_all_special_characters() {
		assert_eq!(esc(r#"<a href="x" onclick='y'>&</a>"#), "&lt;a href=&quot;x&quot; onclick=&#39;y&#39;&gt;&amp;&lt;/a&gt;");
		assert_eq!(esc(""), "");
		assert_eq!(esc("plain ü"), "plain ü");
	}

	#[test]
	fn title_case_works() {
		assert_eq!(title_case("max_pages"), "Max Pages");
		assert_eq!(title_case("name"), "Name");
		assert_eq!(title_case("__x__y"), "X Y");
		assert_eq!(title_case(""), "");
		assert_eq!(title_case("VerifyCa"), "VerifyCa");
	}

	#[test]
	fn paths() {
		assert_eq!(join_path("", "config"), "config");
		assert_eq!(join_path("config", "name"), "config.name");
		assert_eq!(split_path("a..b."), vec!["a", "b"]);
		assert!(split_path("").is_empty());
	}

	#[test]
	fn labels() {
		assert_eq!(label_for("config.max_size"), "Max Size");
		assert_eq!(label_for("config.parser.0"), "Parser #1");
		assert_eq!(label_for("config.parser.11"), "Parser #12");
		assert_eq!(label_for("3"), "Item #4");
		assert_eq!(label_for(""), "");
	}

	#[test]
	fn readable_paths() {
		assert_eq!(readable_path("config.listen.port"), "Listen › Port");
		assert_eq!(readable_path("config.parser.2.mapping.0.name"), "Parser #3 › Mapping #1 › Name");
		assert_eq!(readable_path("config"), "Config");
		assert_eq!(readable_path(""), "");
	}

	#[test]
	fn list_index_detection() {
		assert_eq!(list_index_in("config.parser.2", "config.parser"), Some((2, String::new())));
		assert_eq!(list_index_in("config.parser.2.name", "config.parser"), Some((2, ".name".to_string())));
		assert_eq!(list_index_in("config.parser", "config.parser"), None);
		assert_eq!(list_index_in("config.parsers.2", "config.parser"), None);
		assert_eq!(list_index_in("config.parser.x", "config.parser"), None);
	}

	#[test]
	fn selectors_are_quoted() {
		assert_eq!(attr_selector("data-path", "config.name"), r#"[data-path="config.name"]"#);
		assert_eq!(attr_selector("data-path", r#"a"]b\c"#), r#"[data-path="a\"]b\\c"]"#);
	}

	#[test]
	fn open_paths_are_shifted_after_a_removal() {
		let open = vec!["config.p".to_string(), "config.p.0".to_string(), "config.p.1.m.0".to_string(), "config.p.2".to_string(), "config.q.2".to_string()];
		assert_eq!(shift_after_remove(open, "config.p", 1), vec!["config.p", "config.p.0", "config.p.1", "config.q.2"]);
		assert!(shift_after_remove(vec![], "config.p", 0).is_empty());
	}

	#[test]
	fn download_names() {
		assert_eq!(download_name("webhook.config.toml", "yaml"), "webhook.config.yaml");
		assert_eq!(download_name("config", "toml"), "config.toml");
		assert_eq!(download_name(".toml", "yaml"), "config.yaml");
		assert_eq!(download_name("", "yaml"), "config.yaml");
	}
}
