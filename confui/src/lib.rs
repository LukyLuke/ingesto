mod utils;
mod types;

use std::collections::HashMap;

use shared::types::{ConfType, Queue};
use wasm_bindgen::prelude::*;

use recv_polling;


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

#[wasm_bindgen]
pub fn greet(name: &str) {
	let xx = recv_polling::types::Config{
		config: recv_polling::types::Polling{
			name: "Name blubber".to_string(),
			api: vec![],
			timer: "".to_string(),
			queue: Queue::default(),
			parser: vec![],
		},
	};
	console_log!("Hello {} - how is your {}", name, "day");

	let c: HashMap<String, ConfType> = xx.into();
	console_debug!(c);
}
