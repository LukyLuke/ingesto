mod utils;

use wasm_bindgen::prelude::*;

/// Macro to simplify calls to console.log("", ...) in the browser
/// Use it like `console_log!("Format String", args...)`
macro_rules! console_log {
	($($args:tt)*) => {
		web_sys::console::log_1(&format_args!($($args)*).to_string().into());
	};
}

/// Define all Funciton here which are accessible from WASM in the browser over JavaScript
#[wasm_bindgen]
extern "C" {
	fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
	console_log!("Hello {} - how is your {}", name, "day");
}
