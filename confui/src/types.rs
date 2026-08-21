use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct MenuEntry {
	title: String,
	call: String
}

#[wasm_bindgen]
impl MenuEntry {
	#[wasm_bindgen(constructor)]
	pub fn new(title: &str, call: &str) -> MenuEntry {
		MenuEntry { title: title.into(), call: call.into() }
	}

	#[wasm_bindgen(getter)]
	pub fn title(&self) -> String {
		self.title.clone()
	}

	#[wasm_bindgen(getter)]
	pub fn call(&self) -> String {
		self.call.clone()
	}
}

