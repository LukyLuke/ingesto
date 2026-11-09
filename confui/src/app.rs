//! The connection between the page and the form: state and event handling.
//!
//! There are only four listeners on the document (`click`, `input`, `change`, `focusin`). They find out
//! what happened by the `data-*` attributes which `render.rs` puts on the elements, so the form can be
//! replaced at any time without registering anything again.

use std::cell::RefCell;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{Blob, BlobPropertyBag, Document, Element, Event, HtmlAnchorElement, HtmlElement, HtmlInputElement, HtmlSelectElement, Url};

use crate::codec::{self, LoadReport};
use crate::kinds::{self, ConfigKind};
use crate::model::{Form, Node};
use crate::render::{self, Level};
use crate::types::{Format, Issue};
use crate::utils::{attr_selector, download_name, join_path, shift_after_remove};

const FORM_ID: &str = "dynamic-form";
const TITLE_ID: &str = "config-title";
const SUBTITLE_ID: &str = "config-subtitle";
const MESSAGES_ID: &str = "config-messages";
const SUB_NAV_ID: &str = "sub-navigation";
const FILE_INPUT_ID: &str = "file-input";
const DOC_TITLE_ID: &str = "docTitle";
const DOC_CONTENT_ID: &str = "docContent";
const BUTTON_IDS: [&str; 3] = ["btn-reset", "btn-save-toml", "btn-save-yaml"];

const DEFAULT_TITLE: &str = "Configuration";
const DEFAULT_SUBTITLE: &str = "Select a config type from the menu to begin.";
const DEFAULT_DOC: &str = r#"<div class="rounded-lg bg-white/5 border border-slate-800 p-3 text-sm text-slate-400 space-y-2"><p>Click on any field in the configuration form to view its documentation here.</p></div>"#;

/// The configuration which is currently edited
struct Session {
	kind: &'static ConfigKind,
	form: Form,
	/// The values when it was created or loaded, to reset the form
	initial: Node,
	file_name: String,
}

/// The last loaded file, to open it as another type if the detected one was wrong
struct Pending {
	value: serde_json::Value,
	file_name: String,
	/// All types, the best fit first
	ranked: Vec<&'static ConfigKind>,
}

#[derive(Default)]
struct App {
	session: Option<Session>,
	pending: Option<Pending>,
}

thread_local! {
	static APP: RefCell<App> = RefCell::new(App::default());
}

/// Runs `f` with the state. Never keep it across a call which could trigger an event (like `focus()`)
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
	APP.with(|app| app.try_borrow_mut().ok().map(|mut app| f(&mut app)))
}

fn js(e: JsValue) -> String {
	e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

fn document() -> Result<Document, String> {
	web_sys::window().and_then(|w| w.document()).ok_or_else(|| "The page has no document".to_owned())
}

fn by_id(id: &str) -> Option<Element> {
	document().ok()?.get_element_by_id(id)
}

fn set_html(id: &str, html: &str) {
	if let Some(el) = by_id(id) {
		el.set_inner_html(html);
	}
}

fn set_text(id: &str, text: &str) {
	if let Some(el) = by_id(id) {
		el.set_text_content(Some(text));
	}
}

fn set_enabled(enabled: bool) {
	for id in BUTTON_IDS {
		if let Some(el) = by_id(id) {
			let _ = if enabled { el.remove_attribute("disabled") } else { el.set_attribute("disabled", "") };
		}
	}
}

fn close_menu() {
	if let Some(details) = by_id(SUB_NAV_ID).and_then(|nav| nav.closest("details").ok().flatten()) {
		let _ = details.remove_attribute("open");
	}
}

fn set_messages(blocks: &[String]) {
	set_html(MESSAGES_ID, &blocks.concat());
}

fn clear_messages() {
	set_messages(&[]);
	clear_invalid();
}

fn fail(message: &str) {
	set_messages(&[render::messages_html(Level::Error, message, &[])]);
}

fn clear_doc() {
	set_text(DOC_TITLE_ID, "—");
	set_html(DOC_CONTENT_ID, DEFAULT_DOC);
}

// ---------------------------------------------------------------------------------------------
// Elements of the form
// ---------------------------------------------------------------------------------------------

/// Finds the input or the container of a path, or the closest parent which exists
fn element_for(doc: &Document, path: &str) -> Option<Element> {
	let mut path = path;
	loop {
		let selector = format!("{}, {}", attr_selector("data-path", path), attr_selector("data-details", path));
		if let Some(el) = doc.query_selector(&selector).ok().flatten() {
			return Some(el);
		}
		path = path.rsplit_once('.')?.0;
	}
}

/// Opens all collapsed containers around an element and scrolls to it
fn reveal(el: &Element) {
	if el.tag_name() == "DETAILS" {
		let _ = el.set_attribute("open", "");
	}
	let mut parent = el.parent_element();
	while let Some(p) = parent {
		if p.tag_name() == "DETAILS" {
			let _ = p.set_attribute("open", "");
		}
		parent = p.parent_element();
	}
	el.scroll_into_view();
	if let Some(html) = el.dyn_ref::<HtmlElement>() {
		let _ = html.focus();
	}
}

fn clear_invalid() {
	let Ok(doc) = document() else { return };
	if let Ok(list) = doc.query_selector_all(r#"[aria-invalid="true"]"#) {
		for i in 0..list.length() {
			if let Some(el) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
				let _ = el.remove_attribute("aria-invalid");
			}
		}
	}
}

fn mark_invalid(issues: &[Issue]) {
	let Ok(doc) = document() else { return };
	let mut first: Option<Element> = None;
	for issue in issues.iter().filter(|i| !i.path.is_empty()) {
		if let Some(el) = element_for(&doc, &issue.path) {
			let _ = el.set_attribute("aria-invalid", "true");
			if first.is_none() {
				first = Some(el);
			}
		}
	}
	if let Some(el) = first {
		reveal(&el);
	}
}

/// Shows a list of problems and marks the fields
fn report(heading: &str, issues: &[Issue]) {
	set_messages(&[render::messages_html(Level::Error, heading, issues)]);
	mark_invalid(issues);
}

fn open_details_in(root: &Element) -> Vec<String> {
	let mut paths = Vec::new();
	if let Ok(list) = root.query_selector_all("details[data-details][open]") {
		for i in 0..list.length() {
			if let Some(path) = list.item(i).and_then(|n| n.dyn_into::<Element>().ok()).and_then(|el| el.get_attribute("data-details")) {
				paths.push(path);
			}
		}
	}
	paths
}

fn open_details(root: &Element, path: &str) {
	if let Ok(Some(el)) = root.query_selector(&format!("details{}", attr_selector("data-details", path))) {
		let _ = el.set_attribute("open", "");
	}
}

/// Renders the part of the form at `path` again and replaces it in the page.
/// Containers which were open stay open, `removed` is the entry of a list which does not exist anymore.
fn rerender(path: &str, removed: Option<(&str, usize)>, force_open: &[String]) -> Result<(), String> {
	let html = with_app(|app| app.session.as_ref().map(|s| render::render_path(&s.form, path)))
		.flatten()
		.ok_or("There is no configuration to change")?
		.map_err(|e| e.to_string())?;

	let doc = document()?;
	let selector = attr_selector("data-node", path);
	let old = doc.query_selector(&selector).ok().flatten().ok_or_else(|| format!("'{path}' is not in the page"))?;

	let mut open = open_details_in(&old);
	if let Some((list, index)) = removed {
		open = shift_after_remove(open, list, index);
	}
	old.set_outer_html(&html);

	if let Some(new) = doc.query_selector(&selector).ok().flatten() {
		for path in open.iter().chain(force_open) {
			open_details(&new, path);
		}
	}
	Ok(())
}

fn focus_path(path: &str) {
	if let Some(el) = document().ok().and_then(|doc| doc.query_selector(&attr_selector("data-path", path)).ok().flatten())
		&& let Some(html) = el.dyn_ref::<HtmlElement>()
	{
		let _ = html.focus();
	}
}

fn edit<T>(f: impl FnOnce(&mut Form) -> Result<T, crate::model::ModelError>) -> Result<T, String> {
	with_app(|app| app.session.as_mut().map(|s| f(&mut s.form)))
		.flatten()
		.ok_or("There is no configuration to change")?
		.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------------------------

fn show_empty(heading: &str, text: &str) {
	set_text(TITLE_ID, DEFAULT_TITLE);
	set_text(SUBTITLE_ID, DEFAULT_SUBTITLE);
	set_html(FORM_ID, &render::empty_html(heading, text));
	set_enabled(false);
	clear_messages();
	clear_doc();
}

/// Shows the form of the current session
fn show_form() {
	let view = with_app(|app| app.session.as_ref().map(|s| (s.kind.title, s.kind.description, render::form_html(&s.form)))).flatten();
	let Some((title, description, html)) = view else {
		show_empty("Select a configuration type", r#"Choose from the "Create New" menu above or load a file."#);
		return;
	};
	set_text(TITLE_ID, title);
	set_text(SUBTITLE_ID, description);
	set_html(FORM_ID, &html);
	set_enabled(true);
	close_menu();
	clear_messages();
	clear_doc();
}

fn open_session(kind: &'static ConfigKind, form: Form, file_name: String) {
	with_app(|app| app.session = Some(Session { kind, initial: form.root.clone(), form, file_name }));
	show_form();
}

fn start_new(kind_id: &str) -> Result<(), String> {
	let kind = kinds::find(kind_id).ok_or_else(|| format!("Unknown configuration type '{kind_id}'"))?;
	if !kind.implemented() {
		with_app(|app| {
			app.session = None;
			app.pending = None;
		});
		show_empty(&format!("{} is not available yet", kind.title), "The configuration of this type is not implemented yet.");
		set_text(TITLE_ID, kind.title);
		set_text(SUBTITLE_ID, kind.description);
		close_menu();
		return Ok(());
	}
	open_session(kind, Form::new((kind.schema)()), kind.file_stem.to_owned());
	Ok(())
}

fn reset() {
	with_app(|app| {
		if let Some(session) = app.session.as_mut() {
			session.form.root = session.initial.clone();
		}
	});
	show_form();
}

// ---------------------------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------------------------

fn parse_file(name: &str, text: &str) -> Result<serde_json::Value, String> {
	match Format::from_file_name(name) {
		Some(format) => codec::parse_text(text, format).map_err(|e| format!("{name}: {e}")),
		None => codec::parse_text(text, Format::Toml)
			.or_else(|_| codec::parse_text(text, Format::Yaml))
			.map_err(|_| format!("{name} is neither valid TOML nor valid YAML")),
	}
}

fn load_text(name: &str, text: &str) {
	let value = match parse_file(name, text) {
		Ok(value) => value,
		Err(message) => {
			set_messages(&[render::messages_html(Level::Error, "The file could not be loaded", &[Issue::new("", message)])]);
			return;
		},
	};

	let fits = codec::rank(&value, kinds::all());
	if fits.is_empty() {
		fail("No configuration type is available for this file");
		return;
	}
	let best = codec::best_count(&fits);
	let ranked: Vec<&'static ConfigKind> = fits.iter().map(|fit| fit.kind).collect();
	let chooser = (best > 1).then(|| render::chooser_html(name, &ranked, best));
	let first = ranked[0];
	with_app(|app| app.pending = Some(Pending { value: value.clone(), file_name: name.to_owned(), ranked }));

	match chooser {
		None => open_loaded(first, &value, name),
		Some(html) => {
			with_app(|app| app.session = None);
			show_empty("Which configuration is it?", "");
			set_html(FORM_ID, &html);
		},
	}
}

fn issues_of(paths: &[String], message: &str) -> Vec<Issue> {
	paths.iter().map(|path| Issue::new(path, message)).collect()
}

fn open_loaded(kind: &'static ConfigKind, value: &serde_json::Value, file_name: &str) {
	let (form, loaded): (Form, LoadReport) = codec::form_from_value((kind.schema)(), value);
	open_session(kind, form, file_name.to_owned());

	let mut blocks = vec![render::messages_html(Level::Info, &format!("Loaded {file_name} as {}", kind.title), &[])];
	if let Some(html) = with_app(|app| app.pending.as_ref().map(|p| render::reopen_html(&p.ranked, kind.id))).flatten() {
		blocks.push(html);
	}
	for (heading, paths, message) in [
		("Required values are missing in the file", &loaded.missing, "missing"),
		("Values with the wrong type were ignored", &loaded.mismatched, "wrong type"),
		("Unknown entries were ignored and will not be saved", &loaded.unknown, "unknown"),
	] {
		if !paths.is_empty() {
			blocks.push(render::messages_html(Level::Warning, heading, &issues_of(paths, message)));
		}
	}
	set_messages(&blocks);
	mark_invalid(&issues_of(&loaded.missing, "missing"));
}

fn choose_kind(kind_id: &str) -> Result<(), String> {
	let (value, file_name, kind) = with_app(|app| {
		app.pending.as_ref().map(|p| (p.value.clone(), p.file_name.clone(), p.ranked.iter().copied().find(|k| k.id == kind_id)))
	})
	.flatten()
	.ok_or("There is no file to open")?;
	let kind = kind.ok_or_else(|| format!("'{kind_id}' is not one of the possible types"))?;
	open_loaded(kind, &value, &file_name);
	Ok(())
}

fn read_file(input: &HtmlInputElement) {
	let Some(file) = input.files().and_then(|files| files.item(0)) else {
		return;
	};
	let name = file.name();
	let promise = file.text();
	input.set_value("");
	spawn_local(async move {
		match JsFuture::from(promise).await {
			Ok(text) => match text.as_string() {
				Some(text) => load_text(&name, &text),
				None => fail("The file is not a text file"),
			},
			Err(e) => fail(&format!("{name} could not be read: {}", js(e))),
		}
	});
}

// ---------------------------------------------------------------------------------------------
// Save
// ---------------------------------------------------------------------------------------------

fn download(name: &str, text: &str, format: Format) -> Result<(), String> {
	let parts = js_sys::Array::new();
	parts.push(&JsValue::from_str(text));
	let options = BlobPropertyBag::new();
	options.set_type(format.mime());
	let blob = Blob::new_with_str_sequence_and_options(&parts, &options).map_err(js)?;
	let url = Url::create_object_url_with_blob(&blob).map_err(js)?;

	let anchor = document()?.create_element("a").map_err(js)?.dyn_into::<HtmlAnchorElement>().map_err(|_| "Unable to create a download link".to_owned())?;
	anchor.set_href(&url);
	anchor.set_download(name);
	anchor.click();
	Url::revoke_object_url(&url).map_err(js)
}

fn save(format: Format) -> Result<(), String> {
	clear_messages();
	let (kind, generated, file_name) = with_app(|app| app.session.as_ref().map(|s| (s.kind, codec::form_to_value(&s.form, format), s.file_name.clone())))
		.flatten()
		.ok_or("There is nothing to save")?;

	let value = match generated {
		Ok(value) => value,
		Err(issues) => {
			report("Fill in all required fields and fix the marked values", &issues);
			return Ok(());
		},
	};
	let text = codec::to_text(&value, format)?;
	if let Err(issue) = (kind.check)(&text, format) {
		report(&format!("The configuration is not accepted by the {}", kind.title), &[issue]);
		return Ok(());
	}

	let name = download_name(&file_name, format.extension());
	download(&name, &text, format)?;
	set_messages(&[render::messages_html(Level::Info, &format!("Saved as {name}"), &[])]);
	Ok(())
}

// ---------------------------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------------------------

fn target_element(evt: &Event) -> Option<Element> {
	evt.target()?.dyn_into::<Element>().ok()
}

fn closest(el: &Element, selector: &str) -> Option<Element> {
	el.closest(selector).ok().flatten()
}

fn value_of(el: &Element) -> Option<String> {
	if let Some(input) = el.dyn_ref::<HtmlInputElement>() {
		Some(input.value())
	} else {
		el.dyn_ref::<HtmlSelectElement>().map(|select| select.value())
	}
}

fn dispatch(action: &str, button: &Element) -> Result<(), String> {
	let path = button.get_attribute("data-path").unwrap_or_default();
	match action {
		"new" => start_new(&button.get_attribute("data-kind").unwrap_or_default()),
		"load" => {
			if let Some(input) = by_id(FILE_INPUT_ID).and_then(|el| el.dyn_into::<HtmlElement>().ok()) {
				input.click();
			}
			Ok(())
		},
		"reset" => {
			reset();
			Ok(())
		},
		"save" => {
			let format = button.get_attribute("data-format").and_then(|name| Format::from_name(&name)).ok_or("Unknown file format")?;
			save(format)
		},
		"list-add" => {
			let index = edit(|form| form.list_add(&path))?;
			let item = join_path(&path, &index.to_string());
			rerender(&path, None, &[path.clone(), item.clone()])?;
			let doc = document()?;
			if let Some(input) = doc.query_selector(&format!("{} [data-path]", attr_selector("data-node", &item))).ok().flatten()
				&& let Some(html) = input.dyn_ref::<HtmlElement>()
			{
				let _ = html.focus();
			}
			Ok(())
		},
		"list-remove" => {
			let (list, index) = path.rsplit_once('.').and_then(|(list, index)| Some((list, index.parse::<usize>().ok()?))).ok_or("Unknown entry")?;
			edit(|form| form.list_remove(&path))?;
			rerender(list, Some((list, index)), &[])
		},
		"opt-add" => {
			edit(|form| form.set_present(&path, true))?;
			rerender(&path, None, std::slice::from_ref(&path))
		},
		"opt-remove" => {
			edit(|form| form.set_present(&path, false))?;
			rerender(&path, None, &[])
		},
		"goto" => {
			let doc = document()?;
			if let Some(el) = element_for(&doc, &path) {
				reveal(&el);
			}
			Ok(())
		},
		"choose-kind" => choose_kind(&button.get_attribute("data-kind").unwrap_or_default()),
		"clear-doc" => {
			clear_doc();
			Ok(())
		},
		other => Err(format!("Unknown action '{other}'")),
	}
}

fn on_click(evt: &Event) {
	let Some(button) = target_element(evt).and_then(|el| closest(&el, "[data-action]")) else {
		return;
	};
	// Buttons in a <summary> must not open or close the container
	evt.prevent_default();
	let action = button.get_attribute("data-action").unwrap_or_default();
	if let Err(message) = dispatch(&action, &button) {
		fail(&message);
	}
}

fn on_input(evt: &Event) {
	let Some(el) = target_element(evt).and_then(|el| closest(&el, "[data-path]")) else {
		return;
	};
	if el.get_attribute("data-kind").as_deref() != Some("text") {
		return;
	}
	let (Some(path), Some(value)) = (el.get_attribute("data-path"), value_of(&el)) else {
		return;
	};
	let _ = el.remove_attribute("aria-invalid");
	if let Err(message) = edit(|form| form.set_text(&path, &value)) {
		fail(&message);
	}
}

fn on_change(evt: &Event) {
	let Some(target) = target_element(evt) else {
		return;
	};
	if target.id() == FILE_INPUT_ID {
		if let Some(input) = target.dyn_ref::<HtmlInputElement>() {
			read_file(input);
		}
		return;
	}
	let Some(el) = closest(&target, "[data-path]") else {
		return;
	};
	let Some(path) = el.get_attribute("data-path") else {
		return;
	};
	let _ = el.remove_attribute("aria-invalid");

	let result = match el.get_attribute("data-kind").as_deref() {
		Some("check") => {
			let checked = el.dyn_ref::<HtmlInputElement>().is_some_and(|input| input.checked());
			edit(|form| form.set_text(&path, if checked { "true" } else { "false" }))
		},
		Some("enum") => {
			let variant = value_of(&el).unwrap_or_default();
			edit(|form| form.set_variant(&path, &variant)).and_then(|_| rerender(&path, None, &[])).map(|_| focus_path(&path))
		},
		_ => Ok(()),
	};
	if let Err(message) = result {
		fail(&message);
	}
}

fn on_focusin(evt: &Event) {
	let Some(path) = target_element(evt).and_then(|el| closest(&el, "[data-path]")).and_then(|el| el.get_attribute("data-path")) else {
		return;
	};
	if let Some((title, html)) = with_app(|app| app.session.as_ref().and_then(|s| render::doc_html(&s.form, &path))).flatten() {
		set_text(DOC_TITLE_ID, &title);
		set_html(DOC_CONTENT_ID, &html);
	}
}

fn listen(doc: &Document, name: &str, handler: fn(&Event)) -> Result<(), JsValue> {
	// `Fn` and not `FnMut`: handlers trigger events themselves (`input.click()`, `focus()`), which re-enter the listener
	let callback = Closure::<dyn Fn(Event)>::new(move |evt: Event| handler(&evt));
	doc.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
	callback.forget();
	Ok(())
}

/// Builds the menu and registers the event handlers. Called once when the page is loaded.
pub fn init() -> Result<(), JsValue> {
	crate::utils::set_panic_hook();
	let doc = document().map_err(|e| JsValue::from_str(&e))?;

	set_html(SUB_NAV_ID, &render::nav_buttons_html(kinds::all()));
	show_empty("Select a configuration type", r#"Choose from the "Create New" menu above or load a file."#);

	listen(&doc, "click", on_click)?;
	listen(&doc, "input", on_input)?;
	listen(&doc, "change", on_change)?;
	listen(&doc, "focusin", on_focusin)?;
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn issues_of_keeps_paths() {
		let issues = issues_of(&["config.a".to_owned(), "config.b".to_owned()], "missing");
		assert_eq!(issues, vec![Issue::new("config.a", "missing"), Issue::new("config.b", "missing")]);
		assert!(issues_of(&[], "x").is_empty());
	}

	#[test]
	fn files_are_parsed_by_extension_or_by_trial() {
		assert!(parse_file("a.toml", "[config]\nname = \"x\"").is_ok());
		assert!(parse_file("a.toml", "config:\n  name: x").is_err());
		assert!(parse_file("a.yaml", "config:\n  name: x").is_ok());
		assert!(parse_file("a.yml", "config:\n  name: !Bearer x").is_ok());
		assert!(parse_file("config", "[config]\nname = \"x\"").is_ok());
		assert!(parse_file("config", "config:\n  name: x").is_ok());
		assert!(parse_file("config.txt", "config: [").is_err());
		assert!(parse_file("empty.toml", "").is_ok());
	}

	#[test]
	fn state_is_reachable_and_resettable() {
		let kind = kinds::find("recv_webhook").map(|k| k.id);
		assert_eq!(kind, Some("recv_webhook"));
		assert_eq!(with_app(|app| app.session.is_none()), Some(true));
		let nested = with_app(|_| with_app(|_| ()));
		// Re-entering while the state is borrowed must not panic
		assert_eq!(nested, Some(None));
	}
}
