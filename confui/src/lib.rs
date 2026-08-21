mod utils;
mod types;

use wasm_bindgen::prelude::*;

use shared::types::ConfStruct;
use recv_polling;

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

/// Define all Funciton here which are accessible from WASM in the browser over JavaScript
#[wasm_bindgen]
extern "C" {
	fn alert(s: &str);
}

/// Return a list with all Configuration-Types
#[wasm_bindgen]
pub fn get_config_list() -> Box<[MenuEntry]> {
	return vec![
		MenuEntry::new( "Webhook", "recv_webhook" ),
		MenuEntry::new( "API Polling", "rec_polling" ),
		MenuEntry::new( "Network Listener", "recv_network" ),
		MenuEntry::new( "File Reader", "recv_file" ),
		MenuEntry::new( "Database Reader", "recv_database" ),
		MenuEntry::new( "Database Export", "exp_database" ),
		MenuEntry::new( "Azure DCR Export", "exp_azuredcr" ),
	].into_boxed_slice();
}


#[wasm_bindgen]
pub fn greet(name: &str) {
	let xx = recv_polling::types::Config::default();
	console_log!("Hello {} - how is your {}", name, "day");

	let c: ConfStruct = xx.into();
	console_debug!(c);
}



