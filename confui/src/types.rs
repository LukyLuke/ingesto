use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct MenuEntry {
	Title: String,
	Call: String
}
impl MenuEntry {
	pub fn new(title: &str, call: &str) -> MenuEntry {
		MenuEntry { Title: title.into(), Call: call.into() }
	}
}

