use serde::de::DeserializeOwned;
use shared::types::{ConfStruct, ConfType};

use crate::types::{Format, Issue};

/// A configuration type which can be created, loaded and saved with the ConfUI
pub struct ConfigKind {
	/// Unique identifier, used in the HTML
	pub id: &'static str,

	/// Name shown in the menu and the title
	pub title: &'static str,

	/// Name of a new configuration file without the extension
	pub file_stem: &'static str,

	/// A short description of what the receiver/exporter does
	pub description: &'static str,

	/// Returns the schema of the configuration
	pub schema: fn() -> ConfStruct,

	/// Parses a generated file with the real configuration type of the receiver/exporter
	pub check: fn(&str, Format) -> Result<(), Issue>,
}

impl ConfigKind {
	/// A configuration is not implemented if it only consists of a plain string as `config`
	pub fn implemented(&self) -> bool {
		let schema = (self.schema)();
		!schema.is_empty() && !matches!(schema.get("config"), Some(ConfType::String(_)))
	}
}

fn schema_of<T: Default + Into<ConfStruct>>() -> ConfStruct {
	T::default().into()
}

/// Converts the path of `serde_path_to_error` (`config.api[0].name`) to the path used in the form (`config.api.0.name`)
pub(crate) fn form_path(path: &str) -> String {
	let mut out = String::with_capacity(path.len());
	for c in path.chars() {
		match c {
			'[' => out.push('.'),
			']' => {},
			c => out.push(c),
		}
	}
	out.trim_start_matches('.').to_owned()
}

fn check_text<T: DeserializeOwned>(text: &str, format: Format) -> Result<(), Issue> {
	fn to_issue<E: std::fmt::Display>(e: serde_path_to_error::Error<E>) -> Issue {
		Issue::new(&form_path(&e.path().to_string()), e.inner().to_string())
	}

	match format {
		Format::Toml => {
			let de = toml::Deserializer::parse(text).map_err(|e| Issue::new("", e.to_string()))?;
			serde_path_to_error::deserialize::<_, T>(de).map(|_| ()).map_err(to_issue)
		},
		Format::Yaml => {
			let de = serde_yaml::Deserializer::from_str(text);
			serde_path_to_error::deserialize::<_, T>(de).map(|_| ()).map_err(to_issue)
		},
	}
}

macro_rules! kind {
	($id:literal, $title:literal, $stem:literal, $description:literal, $config:ty) => {
		ConfigKind {
			id: $id,
			title: $title,
			file_stem: $stem,
			description: $description,
			schema: schema_of::<$config>,
			check: check_text::<$config>,
		}
	};
}

static KINDS: [ConfigKind; 7] = [
	kind!(
		"recv_webhook",
		"Webhook",
		"webhook",
		"Listens as a Webserver on a port and path and processes the received data.",
		recv_webhook::types::Config
	),
	kind!(
		"recv_polling",
		"API Polling",
		"polling",
		"Polls an API in a given interval and processes the received content.",
		recv_polling::types::Config
	),
	kind!(
		"recv_network",
		"Network Listener",
		"network",
		"Listens on the network on UDP or TCP and processes the received content.",
		recv_network::types::Config
	),
	kind!(
		"recv_file",
		"File Reader",
		"file",
		"Listens on a file for changes and processes each new line, or reads a file completely every defined interval.",
		recv_file::types::Config
	),
	kind!(
		"recv_database",
		"Database Reader",
		"database",
		"Reads data from one Database, based on a SQL-Query, a forwards a structured message to an exporter.",
		recv_database::types::Config
	),
	kind!(
		"exp_exportdb",
		"Database Export",
		"exportdb",
		"Writes all Data into different tables in a database.",
		exp_exportdb::types::Config
	),
	kind!(
		"exp_azuredcr",
		"Azure DCR Export",
		"azuredcr",
		"Writes out structured data to an Azure Data Collection Rule (DCR).",
		exp_azuredcr::types::Config
	),
];

/// All configuration types
pub fn all() -> &'static [ConfigKind] {
	&KINDS
}

/// Finds a configuration type by it's id
pub fn find(id: &str) -> Option<&'static ConfigKind> {
	KINDS.iter().find(|kind| kind.id == id)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn all_kinds_have_unique_ids_and_a_schema() {
		let mut ids: Vec<&str> = all().iter().map(|k| k.id).collect();
		ids.sort_unstable();
		ids.dedup();
		assert_eq!(ids.len(), all().len());
		for kind in all() {
			assert!(!(kind.schema)().is_empty(), "{}", kind.id);
			assert_eq!(find(kind.id).map(|k| k.id), Some(kind.id));
		}
		assert!(find("unknown").is_none());
	}

	#[test]
	fn stubs_are_not_implemented() {
		let implemented: Vec<&str> = all().iter().filter(|k| k.implemented()).map(|k| k.id).collect();
		assert_eq!(implemented, vec!["recv_webhook", "recv_polling", "recv_network", "recv_file", "exp_exportdb"]);
	}

	#[test]
	fn form_path_conversion() {
		assert_eq!(form_path("config.api[0].name"), "config.api.0.name");
		assert_eq!(form_path("config.a[1][2]"), "config.a.1.2");
		assert_eq!(form_path("."), "");
		assert_eq!(form_path(""), "");
	}

	#[test]
	fn check_reports_the_path_of_the_wrong_value() {
		let toml = "[config]\nname = \"x\"\n[config.listen]\naddress = \"0.0.0.0\"\nport = 70000\n";
		let err = (find("recv_webhook").map(|k| k.check)).map(|f| f(toml, Format::Toml));
		let Some(Err(issue)) = err else {
			panic!("port 70000 must be rejected");
		};
		assert_eq!(issue.path, "config.listen.port");

		let yaml = "config:\n  name: x\n  listen:\n    address: 0.0.0.0\n    port: 70000\n";
		let err = (find("recv_webhook").map(|k| k.check)).map(|f| f(yaml, Format::Yaml));
		let Some(Err(issue)) = err else {
			panic!("port 70000 must be rejected");
		};
		assert_eq!(issue.path, "config.listen.port");
	}

	#[test]
	fn check_rejects_broken_syntax() {
		let check = (find("recv_webhook").map(|k| k.check)).unwrap_or(|_, _| Ok(()));
		assert!(check("[config", Format::Toml).is_err());
		assert!(check("", Format::Toml).is_err());
		assert!(check("config: [", Format::Yaml).is_err());
	}
}
