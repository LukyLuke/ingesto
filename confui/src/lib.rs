mod utils;
mod types;

use wasm_bindgen::prelude::*;
use shared::types::{ConfStruct, ConfType};

// Tailwind CSS colors - only accent-color needs inline style (for checkbox hover state)
// Text colors are handled by Tailwind classes: text-slate-800, text-slate-700
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
	if config.is_empty() || config.get("config").is_some_and(|val| val == &ConfType::String("")) {
		document.get_element_by_id(CONF_RESET_ID).map(|elem| elem.set_attribute("disabled", "true"));
		document.get_element_by_id(CONF_SAVE_ID).map(|elem| elem.set_attribute("disabled", "true"));
		document.get_element_by_id(CONF_CONTAINER_ID).map(|elem| elem.set_inner_html(DEFAULT_EMPTY_CONTAINER));

	} else {
		document.get_element_by_id(CONF_RESET_ID).map(|elem| elem.remove_attribute("disabled"));
		document.get_element_by_id(CONF_SAVE_ID).map(|elem| elem.remove_attribute("disabled"));

		if let Some(container) = document.get_element_by_id(CONF_CONTAINER_ID) {
			build_conf_form(&container, &config)?;
		}
	}

	serde_wasm_bindgen::to_value(&config).map_err(|e| JsValue::from(&e.to_string()))
}

/// Build a configuration form dynamically from a ConfStruct schema
///
/// This function creates HTML form elements based on the type definitions in the ConfStruct.
/// It handles:
/// - String/RegEx: Text input fields with inline styles for contrast
/// - UInt/Int/Float: Number inputs (with min/max for UInt)
/// - Bool: Checkbox with accent color styling
/// - String/RegEx: Text input fields
/// - UInt/Int/Float: Number inputs (with min/max for UInt)
/// - Enum: Select dropdown
/// - Struct: Nested form section with fieldset
/// - Vec: Array with "Add" button to append new items
/// - Option: Optional field wrapped in a section that can be expanded
fn build_conf_form(container: &Element, config: &ConfStruct) -> Result<(), &'static str> {
	// Start building the form HTML
	let mut html = String::new();
	
	for (field_name, field_type) in config {
		html.push_str(&render_field(field_name, field_type, "", true)?);
	}
	
	container.set_inner_html(&html);
	Ok(())
}

/// Render a single field based on its type
fn render_field(name: &str, field_type: &ConfType, path: &str, required: bool) -> Result<String, &'static str> {
	let field_path = if path.is_empty() { name.to_string() } else { format!("{}.{}", path, name) };
	
	match field_type {
		ConfType::String(placeholder) => Ok(render_string_input(name, &field_path, placeholder, required)),
		ConfType::RegEx(placeholder) => Ok(render_regex_input(name, &field_path, placeholder, required)),
		ConfType::Bool(placeholder) => Ok(render_bool_checkbox(name, &field_path, *placeholder, required)),
		ConfType::UInt(placeholder) => Ok(render_uint_input(name, &field_path, *placeholder, required)),
		ConfType::Int(placeholder) => Ok(render_int_input(name, &field_path, *placeholder, required)),
		ConfType::Float(placeholder) => Ok(render_float_input(name, &field_path, *placeholder, required)),
		ConfType::Enum(enum_values, placeholder) => Ok(render_enum_select(name, enum_values, &field_path, placeholder, required)),
		ConfType::Struct(struct_values) => render_struct_field(name, struct_values, &field_path, false),
		ConfType::Vec(inner_type) => Ok(render_vec_array(name, inner_type, path, false)),
		ConfType::Option(inner_type) => render_option_field(name, inner_type, path, false),
		_conf_type @ _ => {
			console_debug!("Unhandled type: {:?}", _conf_type);
			Ok(format!(r#"<div class="text-red-500">Unsupported type: {:?}</div>"#, _conf_type))
		}
	}
}

/// Create the label HTML with optional "Required" badge
fn make_label_html(name: &str, required: bool) -> String {
	if required {
		format!(r#"<label class="block text-sm font-medium text-slate-700 mb-1">{} {}</label>"#, to_title_case(name), required_badge())
	} else {
		format!(r#"<label class="block text-sm font-medium text-slate-700 mb-1">{}</label>"#, to_title_case(name))
	}
}

/// Add a "Required" badge to a field label
fn required_badge() -> &'static str {
	r#"<span class="inline-flex items-center rounded-md bg-red-50 px-2 py-1 text-xs font-medium text-red-700 ring-inset ring-red-600/10">Required</span>"#
}

/// Render a string input field with improved contrast using Tailwind classes
fn render_string_input(name: &str, path: &str, placeholder: &str, required: bool) -> String {
	let label_html = make_label_html(name, required);
	format!(
		r#"
		<div class="mb-4">
			{}
			<input type="text" data-path="{}" placeholder="{}"
				class="w-full rounded border-gray-300 px-3 py-2 shadow-sm text-slate-800 bg-white"
			/>
		</div>
		"#,
		label_html,
		path,
		placeholder
	)
}

/// Render a regular expression input field with improved contrast
fn render_regex_input(name: &str, path: &str, placeholder: &str, required: bool) -> String {
	let label_html = make_label_html(name, required);
	format!(
		r#"
		<div class="mb-4">
			{}
			<input type="text" data-path="{}" placeholder="{}"
				pattern="^.*$"
				title="Enter a valid regex pattern"
				class="w-full rounded border-gray-300 px-3 py-2 font-mono text-sm shadow-sm text-slate-800 bg-white"
			/>
			<p class="text-xs text-slate-500 mt-1">Enter a regular expression pattern</p>
		</div>
		"#,
		label_html,
		path,
		placeholder
	)
}

/// Render an unsigned integer input field with improved contrast
fn render_uint_input(name: &str, path: &str, placeholder: u64, required: bool) -> String {
	let label_html = make_label_html(name, required);
	format!(
		r#"
		<div class="mb-4">
			{}
			<input type="number" min="0" max="65535" data-path="{}" placeholder="{}"
				class="w-32 rounded border-gray-300 px-3 py-2 shadow-sm text-slate-800 bg-white"
			/>
		</div>
		"#,
		label_html,
		path,
		placeholder
	)
}

/// Render a signed integer input field with improved contrast
fn render_int_input(name: &str, path: &str, placeholder: i64, required: bool) -> String {
	let label_html = make_label_html(name, required);
	format!(
		r#"
		<div class="mb-4">
			{}
			<input type="number" min="-2147483648" max="2147483647" data-path="{}" placeholder="{}"
				class="w-32 rounded border-gray-300 px-3 py-2 shadow-sm text-slate-800 bg-white"
			/>
		</div>
		"#,
		label_html,
		path,
		placeholder
	)
}

/// Render a float input field with improved contrast
fn render_float_input(name: &str, path: &str, placeholder: f64, required: bool) -> String {
	let label_html = make_label_html(name, required);
	format!(
		r#"
		<div class="mb-4">
			{}
			<input type="number" step="any" data-path="{}" placeholder="{}"
				class="w-32 rounded border-gray-300 px-3 py-2 shadow-sm text-slate-800 bg-white"
			/>
		</div>
		"#,
		label_html,
		path,
		placeholder
	)
}

/// Render a boolean checkbox with improved contrast using accent-color
fn render_bool_checkbox(name: &str, path: &str, placeholder: bool, required: bool) -> String {
	let label_html = make_label_html(name, required);
	format!(
		r#"
		<div class="mb-4 flex items-center">
			<input type="checkbox" data-path="{}" {}
				class="h-4 w-4 rounded border-gray-300 text-teal-600 shadow-sm accent-[color-mix(in_lab,teal_500_70%,white)]"
			/>
			{}
		</div>
		"#,
		path,
		if placeholder { "checked" } else { "" },
		label_html
	)
}

/// Render an enum select dropdown with improved contrast
fn render_enum_select(name: &str, enum_values: &ConfStruct, path: &str, placeholder: &str, required: bool) -> String {
	let label_html = make_label_html(name, required);
	let mut options = String::new();
	
	for (variant_name, _variant_type) in enum_values {
		let is_selected = if variant_name == placeholder { "selected" } else { "" };
		options.push_str(&format!(
			r#"<option value="{}" class="text-slate-800" {}>{}</option>"#,
			variant_name,
			is_selected,
			to_title_case(variant_name)
		));
	}
	
	format!(
		r#"
		<div class="mb-4">
			{}
			<select data-path="{}" placeholder="{}"
				class="w-full rounded border-gray-300 px-3 py-2 shadow-sm text-slate-800"
			>
				{}
			</select>
		</div>
		"#,
		label_html,
		path,
		placeholder,
		options
	)
}

/// Render a struct field with nested fields (wrapped in a section)
fn render_struct_field(name: &str, struct_values: &ConfStruct, path: &str, required: bool) -> Result<String, &'static str> {
	let label_html = if required {
		format!(r#"<h3 class="bg-slate-50 px-4 py-3 text-sm font-semibold text-slate-900">{} {}</h3>"#, to_title_case(name), required_badge())
	} else {
		format!(r#"<h3 class="bg-slate-50 px-4 py-3 text-sm font-semibold text-slate-900">{}</h3>"#, to_title_case(name))
	};
	let mut html = format!(
		r#"
		<div class="mb-6 border border-slate-300 rounded-lg shadow-sm">
			{}
			<div class="p-4 space-y-4 bg-white">
		"#,
		label_html
	);
	
	for (field_name, field_type) in struct_values {
		html.push_str(&render_field(field_name, field_type, path, true)?);
	}
	
	html.push_str("</div></div>");
	
	Ok(html)
}

/// Render an optional field - renders the inner type directly (Options are truly optional in TOML/YAML)
fn render_option_field(name: &str, inner_type: &ConfType, path: &str, _required: bool) -> Result<String, &'static str> {
	// When rendering an option as a top-level field, mark it as optional (false)
	render_field(name, inner_type, path, false)
}

/// Render an array/vector field with "Add" button
fn render_vec_array(name: &str, inner_type: &ConfType, path: &str, _required: bool) -> String {
	let field_label = to_title_case(name);
	
	// Determine what kind of items are in the array
	let item_label = match inner_type {
		ConfType::Struct(_) => "Item",
		ConfType::String(_) | ConfType::RegEx(_) => "Entry",
		_ => "Item",
	};
	
	format!(
		r#"
		<div class="mb-4" data-array-path="{}">
			<div class="flex items-center justify-between mb-2">
				<h3 class="text-sm font-semibold text-slate-800">{}</h3>
				<button type="button" class="add-array-item inline-flex items-center gap-1 px-3 py-1.5 text-xs font-medium text-teal-700 bg-teal-50 hover:bg-teal-100 rounded border border-teal-200 transition">
					+ Add {}
				</button>
			</div>
			<div class="space-y-3 array-items-container">
				<div class="text-sm text-slate-500 italic">No items yet. Click "+ Add {}" to add one.</div>
			</div>
		</div>
		"#,
		path,
		field_label,
		item_label,
		field_label
	)
}

/// Convert snake_case or PascalCase to Title Case
fn to_title_case(input: &str) -> String {
	input.split('_')
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
