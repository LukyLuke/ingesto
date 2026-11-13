//! ConfUI: A WebUI to create, load and change the configuration files (TOML and YAML) of all
//! receivers and exporters. Everything runs in the browser, the forms are built from the schemas
//! (`ConfStruct`) of the receivers and exporters.

use wasm_bindgen::prelude::*;

#[cfg(test)]
mod all_kinds;
mod app;
mod codec;
mod kinds;
mod model;
mod render;
mod types;
mod utils;

/// Builds the menu and registers all event handlers. Call it once after the page has been loaded.
#[wasm_bindgen]
pub fn init() -> Result<(), JsValue> {
	app::init()
}
