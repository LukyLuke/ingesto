/// The file formats a configuration can be loaded from and saved to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
	Toml,
	Yaml,
}

impl Format {
	/// File extension without a dot
	pub fn extension(self) -> &'static str {
		match self {
			Format::Toml => "toml",
			Format::Yaml => "yaml",
		}
	}

	/// MIME-Type used for the download
	pub fn mime(self) -> &'static str {
		match self {
			Format::Toml => "application/toml",
			Format::Yaml => "application/yaml",
		}
	}

	/// Parses the name used in the HTML (`data-format`)
	pub fn from_name(name: &str) -> Option<Format> {
		match name.to_ascii_lowercase().as_str() {
			"toml" => Some(Format::Toml),
			"yaml" | "yml" => Some(Format::Yaml),
			_ => None,
		}
	}

	/// Detects the format by the extension of a file name
	pub fn from_file_name(name: &str) -> Option<Format> {
		name.rsplit_once('.').and_then(|(_, ext)| Self::from_name(ext))
	}
}

/// A problem with a value in the form. The `path` is the same as the `data-path` of the form field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
	pub path: String,
	pub message: String,
}

impl Issue {
	pub fn new(path: &str, message: impl Into<String>) -> Self {
		Self { path: path.to_owned(), message: message.into() }
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn format_from_file_name() {
		assert_eq!(Format::from_file_name("webhook.config.toml"), Some(Format::Toml));
		assert_eq!(Format::from_file_name("a.YAML"), Some(Format::Yaml));
		assert_eq!(Format::from_file_name("a.yml"), Some(Format::Yaml));
		assert_eq!(Format::from_file_name("config"), None);
		assert_eq!(Format::from_file_name("config.json"), None);
		assert_eq!(Format::from_file_name(""), None);
		assert_eq!(Format::from_file_name(".toml"), Some(Format::Toml));
	}

	#[test]
	fn format_names_and_extensions() {
		assert_eq!(Format::from_name("TOML"), Some(Format::Toml));
		assert_eq!(Format::from_name("json"), None);
		assert_eq!(Format::Toml.extension(), "toml");
		assert_eq!(Format::Yaml.extension(), "yaml");
	}
}
