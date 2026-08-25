mod utils;
mod types;

use wasm_bindgen::prelude::*;
use shared::types::{ConfStruct, ConfType};

use exp_azuredcr;
use exp_exportdb;
use recv_database;
use recv_file;
use recv_network;
use recv_polling;
use recv_webhook;
use web_sys::Element;

use crate::types::MenuEntry;

/// Macro to simplify calls to console.log("", ...) in the browser
/// Use it like `console_log!("Format String", args...)`
#[allow(unused_macros)]
macro_rules! console_log {
	($($args:tt)*) => {
		web_sys::console::log_1(&format_args!($($args)*).to_string().into());
	};
}
#[allow(unused_macros)]
macro_rules! console_error {
	($($args:tt)*) => {
		web_sys::console::error_1(&format_args!($($args)*).into());
	};
}
#[allow(unused_macros)]
macro_rules! console_debug {
	($value:expr) => {{
		let js_val = serde_wasm_bindgen::to_value(&$value).unwrap_or_default();
		web_sys::console::debug_1(&js_val);
	}};
	($($args:tt)*) => {
		let js_val = serde_wasm_bindgen::to_value(&format!($($args)*)).unwrap_or_default();
		web_sys::console::debug_1(&js_val);
	};
}

#[allow(unused)]
const MAIN_NAV_ID: &str = "main-navigation";
#[allow(unused)]
const SUB_NAV_ID: &str = "sub-navigation";
#[allow(unused)]
const CONF_CONTAINER_ID: &str = "dynamic-form";
#[allow(unused)]
const CONF_TITLE_ID: &str = "config-title";
#[allow(unused)]
const CONF_DESCRIPTION_ID: &str = "config-subtitle";
#[allow(unused)]
const CONF_RESET_ID: &str = "btn-reset";
#[allow(unused)]
const CONF_SAVE_ID: &str = "btn-save-json";

const DEFAULT_EMPTY_CONTAINER: &str = r##"
<div class="text-center text-slate-500 py-12">
	<svg class="w-16 h-16 mx-auto mb-4 text-slate-300" fill="none" stroke="currentColor" viewBox="0 0 24 24">
		<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
	</svg>
	<p class="text-lg font-medium">Select a configuration type</p>
	<p class="text-sm mt-1">Choose from the "Create New" menu above to start editing.</p>
</div>
"##;

/// Define all Funciton here which are accessible from WASM in the browser over JavaScript
#[wasm_bindgen]
extern "C" {
	fn alert(s: &str);
}

/// Return a list with all Configuration-Types
#[wasm_bindgen]
pub fn get_config_list() -> Box<[MenuEntry]> {
	vec![
		MenuEntry::new( "Webhook", "recv_webhook" ).into(),
		MenuEntry::new( "API Polling", "recv_polling" ).into(),
		MenuEntry::new( "Network Listener", "recv_network" ).into(),
		MenuEntry::new( "File Reader", "recv_file" ).into(),
		MenuEntry::new( "Database Reader", "recv_database" ).into(),
		MenuEntry::new( "Database Export", "exp_exportdb" ).into(),
		MenuEntry::new( "Azure DCR Export", "exp_azuredcr" ).into(),
	].into_boxed_slice()
}

/// Creates the Configuration-Formular absed on the selected type and returns the Configuration-Schema.
#[wasm_bindgen]
pub fn get_config_schema(config_type: &str) -> Result<JsValue, JsValue> {
	let window = web_sys::window().expect("Unable to get the 'window' object from your browser: window");
	let document = window.document().expect("Unable to get the 'document' Object from your browser: window.document");

	let (config, title, description) = match config_type {
		"exp_azuredcr"  => (exp_azuredcr::types::Config::default().into(), "Azure DCR", "Writes out structured data to an Azure Data Collection Rule (DCR)"),
		"exp_exportdb"  => (exp_exportdb::types::Config::default().into(), "Database Exporter", "Writes all Data into different tables in a database."),
		"recv_database" => (recv_database::types::Config::default().into(), "Database Reader", "Reads data from one Database, based on a SQL-Query, a forwards a structured message to an exporter."),
		"recv_file"     => (recv_file::types::Config::default().into(), "File Listener", "Listens on a file for changes and processes each new line, or reads a file completely every defined interval."),
		"recv_network"  => (recv_network::types::Config::default().into(), "Network Listener", "Listens on the network on UDP or TCP and processes the received content."),
		"recv_polling"  => (recv_polling::types::Config::default().into(), "API Polling", "Polls an API in a given interval and processes the received content."),
		"recv_webhook"  => (recv_webhook::types::Config::default().into(), "Webhook Listener", "Listens as a Webserver on a port and path and processes the received data."),

		// This should only be triggered on the "init" call when first loaded
		_ => (ConfStruct::new(), "Configuration", "Select a config type from the menu to begin."),
	};

	// Hide the sub-navigation
	document.get_element_by_id(SUB_NAV_ID).map(|nav|
		nav.closest("details").map(|det| det.map(|elem| elem.remove_attribute("open") ) )
	);

	// Change the title and description
	document.get_element_by_id(CONF_TITLE_ID).map(|elem| elem.set_inner_html(title));
	document.get_element_by_id(CONF_DESCRIPTION_ID).map(|elem| elem.set_inner_html(description));

	// If the configuraiton is just a string, the structure is not yet implemented and therefore not valid
	// If the configuration is valid, enable the buttons and create the containers based on it
	// If it is not valid, clear the container and disable the buttons
	if config.is_empty() || config.get("config").is_some_and(|val| val == &ConfType::String) {
		document.get_element_by_id(CONF_RESET_ID).map(|elem| elem.set_attribute("disabled", "true"));
		document.get_element_by_id(CONF_SAVE_ID).map(|elem| elem.set_attribute("disabled", "true"));
		document.get_element_by_id(CONF_CONTAINER_ID).map(|elem| elem.set_inner_html(DEFAULT_EMPTY_CONTAINER));

	} else {
		document.get_element_by_id(CONF_RESET_ID).map(|elem| elem.remove_attribute("disabled"));
		document.get_element_by_id(CONF_SAVE_ID).map(|elem| elem.remove_attribute("disabled"));

		if let Some(container) = document.get_element_by_id(CONF_CONTAINER_ID) {
			build_conf_form(&container)?;
		}
	}

	serde_wasm_bindgen::to_value(&config).map_err(|e| JsValue::from(&e.to_string()))
}

fn build_conf_form(container: &Element) -> Result<(), &'static str> {
	container.set_inner_html("ok");
	Err("not yet")
}
