//! Generates the HTML of the form. Everything in here is a pure function without access to the DOM.
//!
//! Contract with the event handling in `app.rs`:
//! - `data-node="PATH"` is on the wrapper of a field or list entry and is what gets replaced when it changes
//! - `data-details="PATH"` is on a `<details>` element to keep it's open/closed state
//! - `data-path="PATH"` is on inputs, with `data-kind` = `text` | `check` | `enum`
//! - `data-action` is on buttons: `list-add`, `list-remove`, `opt-add`, `opt-remove`, `goto`, `new`, `choose-kind`

use shared::types::{ConfStruct, ConfType};

use crate::kinds::ConfigKind;
use crate::model::{peel, variants_of, Form, ModelError, Node};
use crate::types::Issue;
use crate::utils::{esc, join_path, label_for, readable_path, split_path};

const INPUT_CLASS: &str = "w-full rounded border border-slate-300 px-3 py-2 shadow-sm text-slate-800 bg-white";
const LABEL_CLASS: &str = "block text-sm font-semibold text-slate-700 mb-1";
const BADGE_REQUIRED: &str = r#"<span class="inline-flex items-center rounded-md bg-red-50 px-2 py-1 text-xs font-medium text-red-700 ring-inset ring-red-600/10">Required</span>"#;
const BADGE_OPTIONAL: &str = r#"<span class="inline-flex items-center rounded-md bg-slate-100 px-2 py-1 text-xs font-medium text-slate-500">Optional</span>"#;
const BOX_CLASS: &str = "mb-4 border border-slate-300 rounded-lg shadow-sm";
const SUMMARY_CLASS: &str = "bg-slate-50 px-4 py-2 list-none cursor-pointer hover:bg-slate-100 transition-colors rounded-lg";
const BTN_ADD: &str = "inline-flex items-center gap-1 px-3 py-1 text-xs font-medium text-teal-700 bg-teal-50 hover:bg-teal-100 rounded border border-teal-200 transition";
const BTN_REMOVE: &str = "inline-flex items-center gap-1 px-3 py-1 text-xs font-medium text-red-700 bg-red-50 hover:bg-red-100 rounded border border-red-200 transition";

/// How deep fields are nested before the container is closed by default
const OPEN_DEPTH: usize = 0;

fn badge(optional: bool) -> &'static str {
	if optional {
		BADGE_OPTIONAL
	} else {
		BADGE_REQUIRED
	}
}

fn button(class: &str, action: &str, path: &str, label: &str) -> String {
	format!(r#"<button type="button" class="{class}" data-action="{action}" data-path="{}">{label}</button>"#, esc(path))
}

fn is_enum(ty: &ConfType) -> bool {
	matches!(ty, ConfType::Enum(..) | ConfType::Tagged(..))
}

/// A collapsible box with a title and optional buttons
fn container(label: &str, path: &str, depth: usize, optional: bool, actions: &str, body: &str) -> String {
	let open = if depth == OPEN_DEPTH { " open" } else { "" };
	let path = esc(path);
	format!(
		r#"<div class="{BOX_CLASS}" data-node="{path}"><details class="border-none p-0" data-details="{path}"{open}><summary class="{SUMMARY_CLASS}"><div class="flex items-center justify-between gap-2"><span class="flex items-center gap-2 text-sm font-semibold text-slate-700">{} {}</span><span class="flex gap-2">{actions}</span></div></summary><div class="p-4 space-y-4 bg-white">{body}</div></details></div>"#,
		esc(label),
		badge(optional)
	)
}

fn input_attrs(path: &str, kind: &str, type_name: &str, optional: bool) -> String {
	let required = if optional { "false" } else { "true" };
	let aria_required = if optional { "" } else { r#" aria-required="true""# };
	format!(r#"id="f-{path}" data-path="{path}" data-kind="{kind}" data-type="{type_name}" data-required="{required}"{aria_required}"#, path = esc(path))
}

/// The input element for a string, regular expression, number or bool
fn scalar_input(ty: &ConfType, optional: bool, text: &str, path: &str) -> String {
	let value = esc(text);
	match ty {
		ConfType::Bool(_) if optional => {
			let option = |v: &str, label: &str| format!(r#"<option value="{v}"{}>{label}</option>"#, if text == v { " selected" } else { "" });
			format!(
				r#"<select {} class="{INPUT_CLASS}">{}{}{}</select>"#,
				input_attrs(path, "text", "bool", true),
				option("", "— not set —"),
				option("true", "true"),
				option("false", "false")
			)
		},
		ConfType::Bool(_) => {
			let checked = if text == "true" { " checked" } else { "" };
			format!(r#"<input type="checkbox" {} class="h-4 w-4 rounded border-gray-300 text-teal-600 shadow-sm"{checked} />"#, input_attrs(path, "check", "bool", optional))
		},
		ConfType::UInt(default) => format!(
			r#"<input type="number" min="0" step="1" {} value="{value}" placeholder="{default}" class="w-48 rounded border border-slate-300 px-3 py-2 shadow-sm text-slate-800 bg-white" />"#,
			input_attrs(path, "text", "uint", optional)
		),
		ConfType::Int(default) => format!(
			r#"<input type="number" step="1" {} value="{value}" placeholder="{default}" class="w-48 rounded border border-slate-300 px-3 py-2 shadow-sm text-slate-800 bg-white" />"#,
			input_attrs(path, "text", "int", optional)
		),
		ConfType::Float(default) => format!(
			r#"<input type="number" step="any" {} value="{value}" placeholder="{default}" class="w-48 rounded border border-slate-300 px-3 py-2 shadow-sm text-slate-800 bg-white" />"#,
			input_attrs(path, "text", "float", optional)
		),
		ConfType::RegEx(default) => format!(
			r#"<input type="text" spellcheck="false" {} value="{value}" placeholder="{}" class="{INPUT_CLASS} font-mono text-sm" />"#,
			input_attrs(path, "text", "regex", optional),
			esc(default)
		),
		ConfType::String(default) => format!(
			r#"<input type="text" {} value="{value}" placeholder="{}" class="{INPUT_CLASS}" />"#,
			input_attrs(path, "text", "string", optional),
			esc(default)
		),
		_ => String::new(),
	}
}

/// The select for the variant of an enum together with the fields of the selected variant
fn enum_control(ty: &ConfType, optional: bool, node: &Node, path: &str, depth: usize) -> String {
	let (inner, _) = peel(ty);
	let Some(variants) = variants_of(inner) else {
		return String::new();
	};
	let selected = match node {
		Node::Enum { variant, .. } => Some(variant.as_str()),
		Node::Opt(Some(child)) => match child.as_ref() {
			Node::Enum { variant, .. } => Some(variant.as_str()),
			_ => None,
		},
		_ => None,
	};

	let mut options = String::new();
	if optional {
		options.push_str(&format!(r#"<option value=""{}>— not set —</option>"#, if selected.is_none() { " selected" } else { "" }));
	}
	for (name, _) in variants {
		let sel = if selected == Some(name.as_str()) { " selected" } else { "" };
		options.push_str(&format!(r#"<option value="{}"{sel}>{}</option>"#, esc(name), esc(name)));
	}
	let mut html = format!(r#"<select {} class="{INPUT_CLASS}">{options}</select>"#, input_attrs(path, "enum", "enum", optional));

	let payload = match node {
		Node::Enum { payload, .. } => Some(payload.as_ref()),
		Node::Opt(Some(child)) => match child.as_ref() {
			Node::Enum { payload, .. } => Some(payload.as_ref()),
			_ => None,
		},
		_ => None,
	};
	if let (Some(variant), Some(payload)) = (selected, payload)
		&& let Some(variant_ty) = variants.get(variant)
	{
		let payload_path = join_path(path, variant);
		let inner_html = match peel(variant_ty).0 {
			ConfType::EnumValue => String::new(),
			ConfType::Struct(_) | ConfType::Tuple(_) => value_body(variant_ty, payload, &payload_path, depth + 1),
			_ => field(&label_for(&payload_path), variant_ty, payload, &payload_path, depth + 1),
		};
		if !inner_html.is_empty() {
			html.push_str(&format!(r#"<div class="mt-3 pl-4 border-l-2 border-slate-200 space-y-4">{inner_html}</div>"#));
		}
	}
	html
}

/// The content of a value without a label around it
fn value_body(ty: &ConfType, node: &Node, path: &str, depth: usize) -> String {
	let (inner, optional) = peel(ty);
	match (node, inner) {
		(Node::Text(text), _) => scalar_input(inner, optional, text, path),
		(Node::Struct(fields), ConfType::Struct(schema)) => fields
			.iter()
			.filter_map(|(name, child)| schema.get(name).map(|child_ty| (name, child, child_ty)))
			.map(|(name, child, child_ty)| field(&label_for(&join_path(path, name)), child_ty, child, &join_path(path, name), depth + 1))
			.collect(),
		(Node::Tuple(items), ConfType::Tuple(types)) => items
			.iter()
			.zip(types)
			.enumerate()
			.map(|(index, (child, child_ty))| {
				let child_path = join_path(path, &index.to_string());
				field(&label_for(&child_path), child_ty, child, &child_path, depth + 1)
			})
			.collect(),
		(Node::Enum { .. }, _) => enum_control(ty, optional, node, path, depth),
		(Node::Opt(Some(child)), _) => value_body(inner, child, path, depth),
		_ => String::new(),
	}
}

/// An entry of a list, which can be removed
fn list_item(item_ty: &ConfType, node: &Node, path: &str, depth: usize) -> String {
	let label = label_for(path);
	let remove = button(BTN_REMOVE, "list-remove", path, "Remove");
	let body = value_body(item_ty, node, path, depth + 1);
	let open = if depth == OPEN_DEPTH { " open" } else { "" };
	let path = esc(path);
	format!(
		r#"<div class="border border-slate-200 rounded-md" data-node="{path}"><details class="border-none p-0" data-details="{path}"{open}><summary class="{SUMMARY_CLASS}"><div class="flex items-center justify-between gap-2"><span class="text-sm font-semibold text-slate-700">{}</span><span class="flex gap-2">{remove}</span></div></summary><div class="p-4 space-y-4 bg-white">{body}</div></details></div>"#,
		esc(&label)
	)
}

/// A labeled field of any type
fn field(label: &str, ty: &ConfType, node: &Node, path: &str, depth: usize) -> String {
	let (inner, optional) = peel(ty);
	let path_attr = esc(path);
	match (node, inner) {
		(Node::Opt(None), _) if !is_enum(inner) => format!(
			r#"<div class="mb-4 border border-dashed border-slate-300 rounded-lg" data-node="{path_attr}"><div class="px-4 py-2 flex items-center justify-between gap-2"><span class="flex items-center gap-2 text-sm font-semibold text-slate-500">{} {BADGE_OPTIONAL}</span>{}</div></div>"#,
			esc(label),
			button(BTN_ADD, "opt-add", path, "+ Add")
		),
		(Node::Opt(_), _) if is_enum(inner) => {
			format!(r#"<div class="mb-4" data-node="{path_attr}"><label class="{LABEL_CLASS}" for="f-{path_attr}">{} {BADGE_OPTIONAL}</label>{}</div>"#, esc(label), enum_control(ty, true, node, path, depth))
		},
		(Node::Opt(Some(child)), _) => container(label, path, depth, true, &button(BTN_REMOVE, "opt-remove", path, "Remove"), &value_body(inner, child, path, depth)),
		(Node::Opt(None), _) => String::new(),
		(Node::Text(text), ConfType::Bool(_)) if !optional => format!(
			r#"<div class="mb-4 flex items-center gap-3" data-node="{path_attr}">{}<label class="text-sm font-semibold text-slate-700" for="f-{path_attr}">{}</label></div>"#,
			scalar_input(inner, false, text, path),
			esc(label)
		),
		(Node::Text(text), _) => {
			let hint = if matches!(inner, ConfType::RegEx(_)) { r#"<p class="text-xs text-slate-500 mt-1">A regular expression</p>"# } else { "" };
			format!(r#"<div class="mb-4" data-node="{path_attr}"><label class="{LABEL_CLASS}" for="f-{path_attr}">{} {}</label>{}{hint}</div>"#, esc(label), badge(optional), scalar_input(inner, optional, text, path))
		},
		(Node::List(items), ConfType::Vec(item_ty)) => {
			let body = if items.is_empty() {
				r#"<div class="text-sm text-slate-500 italic">No entries yet. Click "+ Add" to add one.</div>"#.to_owned()
			} else {
				let entries: String = items.iter().enumerate().map(|(index, item)| list_item(item_ty, item, &join_path(path, &index.to_string()), depth + 1)).collect();
				format!(r#"<div class="space-y-3">{entries}</div>"#)
			};
			container(label, path, depth, optional, &button(BTN_ADD, "list-add", path, "+ Add"), &body)
		},
		(Node::Enum { .. }, _) => {
			format!(r#"<div class="mb-4" data-node="{path_attr}"><label class="{LABEL_CLASS}" for="f-{path_attr}">{} {}</label>{}</div>"#, esc(label), badge(optional), enum_control(ty, optional, node, path, depth))
		},
		(Node::Struct(_) | Node::Tuple(_), _) => container(label, path, depth, optional, "", &value_body(ty, node, path, depth)),
		(Node::Unit | Node::List(_), _) => String::new(),
	}
}

/// The complete form. A root with just the `config` struct shows it's fields directly.
pub fn form_html(form: &Form) -> String {
	let (Node::Struct(fields), ConfType::Struct(schema)) = (&form.root, &form.schema) else {
		return String::new();
	};
	let (path, fields, schema) = match fields.as_slice() {
		[(name, Node::Struct(children))] => match schema.get(name) {
			Some(ConfType::Struct(child_schema)) => (name.as_str(), children, child_schema),
			_ => ("", fields, schema),
		},
		_ => ("", fields, schema),
	};
	fields
		.iter()
		.filter_map(|(name, child)| schema.get(name).map(|ty| (name, child, ty)))
		.map(|(name, child, ty)| {
			let child_path = join_path(path, name);
			field(&label_for(&child_path), ty, child, &child_path, 0)
		})
		.collect()
}

/// Generates the HTML of the part of the form at `path` which can be replaced in the page: a field or a list entry
pub fn render_path(form: &Form, path: &str) -> Result<String, ModelError> {
	let (node, ty) = form.locate(path)?;
	let depth = split_path(path).len().saturating_sub(2);

	if let Some((parent, last)) = path.rsplit_once('.')
		&& last.parse::<usize>().is_ok()
		&& matches!(form.locate(parent).map(|(_, ty)| peel(ty).0), Ok(ConfType::Vec(_)))
	{
		return Ok(list_item(ty, node, path, depth));
	}
	Ok(field(&label_for(path), ty, node, path, depth))
}

/// A short readable description of a type
pub fn describe(ty: &ConfType) -> String {
	match peel(ty).0 {
		ConfType::Bool(_) => "Yes / No".to_owned(),
		ConfType::UInt(_) => "Whole number, 0 or more".to_owned(),
		ConfType::Int(_) => "Whole number".to_owned(),
		ConfType::Float(_) => "Number".to_owned(),
		ConfType::String(_) => "Text".to_owned(),
		ConfType::RegEx(_) => "Regular expression".to_owned(),
		ConfType::Enum(..) | ConfType::Tagged(..) => "Choice".to_owned(),
		ConfType::EnumValue => "No value".to_owned(),
		ConfType::Struct(fields) => format!("Group of {} fields", fields.len()),
		ConfType::Tuple(types) => format!("Group of {} values", types.len()),
		ConfType::Vec(item) => format!("List of: {}", describe(item).to_lowercase()),
		ConfType::Option(_) => String::new(),
	}
}

fn variants_description(variants: &ConfStruct) -> String {
	let items: String = variants
		.iter()
		.map(|(name, ty)| {
			let extra = match peel(ty).0 {
				ConfType::EnumValue => String::new(),
				other => format!(" <span class=\"text-slate-400\">({})</span>", esc(&describe(other))),
			};
			format!(r#"<li><span class="font-mono text-teal-200">{}</span>{extra}</li>"#, esc(name))
		})
		.collect();
	format!(r#"<ul class="list-disc list-inside text-xs text-slate-300 space-y-1">{items}</ul>"#)
}

/// The title and the HTML for the documentation pane for the field at `path`
pub fn doc_html(form: &Form, path: &str) -> Option<(String, String)> {
	let (_, ty) = form.locate(path).ok()?;
	let (inner, optional) = peel(ty);
	let row = |name: &str, value: &str| format!(r#"<div><div class="text-xs text-slate-400">{name}</div><div class="text-sm text-slate-100">{value}</div></div>"#);

	let mut html = String::from(r#"<div class="rounded-lg bg-white/5 border border-slate-800 p-3 space-y-3">"#);
	html.push_str(&row("Type", &esc(&describe(inner))));
	html.push_str(&row("Required", if optional { "No, can be left empty" } else { "Yes" }));
	match inner {
		ConfType::String(default) | ConfType::RegEx(default) if !default.is_empty() => html.push_str(&row("Example / Default", &format!(r#"<code class="font-mono">{}</code>"#, esc(default)))),
		ConfType::UInt(default) => html.push_str(&row("Example / Default", &default.to_string())),
		ConfType::Int(default) => html.push_str(&row("Example / Default", &default.to_string())),
		ConfType::Float(default) => html.push_str(&row("Example / Default", &default.to_string())),
		ConfType::Enum(variants, default) | ConfType::Tagged(_, variants, default) => {
			html.push_str(&row("Default", &format!(r#"<code class="font-mono">{}</code>"#, esc(default))));
			html.push_str(&row("Accepted values", &variants_description(variants)));
		},
		_ => {},
	}
	html.push_str("</div>");
	Some((readable_path(path), html))
}

/// The buttons of the "Create New" menu
pub fn nav_buttons_html(kinds: &[ConfigKind]) -> String {
	kinds
		.iter()
		.map(|kind| {
			format!(
				r#"<button type="button" class="block w-full rounded px-3 py-2 text-left text-sm hover:bg-slate-700" data-action="new" data-kind="{}">New {}</button>"#,
				esc(kind.id),
				esc(kind.title)
			)
		})
		.collect()
}

/// The text shown in the form if there is nothing to edit
pub fn empty_html(heading: &str, text: &str) -> String {
	format!(
		r#"<div class="text-center text-slate-500 py-12"><svg class="w-16 h-16 mx-auto mb-4 text-slate-300" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg><p class="text-lg font-medium">{}</p><p class="text-sm mt-1">{}</p></div>"#,
		esc(heading),
		esc(text)
	)
}

/// The severity of a message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
	Error,
	Warning,
	Info,
}

/// A box with a heading and a list of problems. Entries with a path link to the field.
pub fn messages_html(level: Level, heading: &str, items: &[Issue]) -> String {
	let colors = match level {
		Level::Error => "border-red-300 bg-red-50 text-red-800",
		Level::Warning => "border-amber-300 bg-amber-50 text-amber-900",
		Level::Info => "border-teal-300 bg-teal-50 text-teal-900",
	};
	let list: String = items
		.iter()
		.map(|issue| {
			if issue.path.is_empty() {
				format!("<li>{}</li>", esc(&issue.message))
			} else {
				format!(
					r#"<li><button type="button" class="underline font-medium" data-action="goto" data-path="{}">{}</button>: {}</li>"#,
					esc(&issue.path),
					esc(&readable_path(&issue.path)),
					esc(&issue.message)
				)
			}
		})
		.collect();
	let list = if list.is_empty() { String::new() } else { format!(r#"<ul class="list-disc list-inside mt-1 space-y-1">{list}</ul>"#) };
	format!(r#"<div class="rounded-md border p-3 text-sm {colors}" role="alert"><p class="font-semibold">{}</p>{list}</div>"#, esc(heading))
}

/// Asks which configuration type a loaded file is. The first `best` types are the recommended ones.
pub fn chooser_html(file_name: &str, kinds: &[&ConfigKind], best: usize) -> String {
	let buttons: String = kinds
		.iter()
		.enumerate()
		.map(|(index, kind)| {
			let colors = if index < best { "bg-teal-600 text-white hover:bg-teal-700" } else { "bg-slate-100 text-slate-700 hover:bg-slate-200" };
			format!(
				r#"<button type="button" class="px-3 py-2 text-sm rounded-md transition {colors}" data-action="choose-kind" data-kind="{}">{}</button>"#,
				esc(kind.id),
				esc(kind.title)
			)
		})
		.collect();
	format!(
		r#"<div class="rounded-lg border border-amber-300 bg-amber-50 p-4 text-amber-900"><p class="font-semibold">What kind of configuration is "{}"?</p><p class="text-sm mt-1">The file fits more than one type. The recommended ones are highlighted.</p><div class="flex flex-wrap gap-2 mt-3">{buttons}</div></div>"#,
		esc(file_name)
	)
}

/// Buttons to open a loaded file as another type, if the detected one was wrong
pub fn reopen_html(kinds: &[&ConfigKind], current: &str) -> String {
	let buttons: String = kinds
		.iter()
		.filter(|kind| kind.id != current)
		.map(|kind| {
			format!(
				r#"<button type="button" class="px-2 py-1 text-xs rounded border border-slate-300 bg-white text-slate-700 hover:bg-slate-100 transition" data-action="choose-kind" data-kind="{}">{}</button>"#,
				esc(kind.id),
				esc(kind.title)
			)
		})
		.collect();
	if buttons.is_empty() {
		return String::new();
	}
	format!(r#"<div class="rounded-md border border-slate-200 bg-white p-3 text-sm text-slate-700"><p>Not the right type? Open the file as:</p><div class="flex flex-wrap gap-2 mt-2">{buttons}</div></div>"#)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::kinds;
	use crate::model::tests::sample_form;

	fn count(haystack: &str, needle: &str) -> usize {
		haystack.matches(needle).count()
	}

	#[test]
	fn root_config_is_rendered_inline_with_required_flags() {
		let html = form_html(&sample_form());
		assert!(html.contains(r#"data-path="config.name""#));
		assert!(!html.contains(r#"data-details="config""#));
		let name = html.split(r#"data-node="config.name""#).nth(1).unwrap_or_default();
		assert!(name.split("</div>").next().unwrap_or_default().contains("Required"));
		let port = html.split(r#"data-node="config.port""#).nth(1).unwrap_or_default();
		assert!(port.split("</div>").next().unwrap_or_default().contains("Optional"));
		assert!(html.contains(r#"data-path="config.port" data-kind="text" data-type="uint" data-required="false""#));
		assert!(html.contains(r#"data-required="true" aria-required="true""#));
		assert_eq!(count(&html, "<details"), count(&html, "</details>"));
		assert_eq!(count(&html, "<div"), count(&html, "</div>"));
	}

	#[test]
	fn fields_are_in_schema_order() {
		let html = form_html(&sample_form());
		let positions: Vec<usize> = ["config.name\"", "config.port\"", "config.ratio\"", "config.follow\"", "config.active\""].iter().filter_map(|p| html.find(&format!("data-path=\"{p}"))).collect();
		assert_eq!(positions.len(), 5);
		assert!(positions.windows(2).all(|w| w[0] < w[1]));
	}

	#[test]
	fn optional_structs_have_add_and_remove_buttons() {
		let mut form = sample_form();
		let html = form_html(&form);
		assert!(html.contains(r#"data-action="opt-add" data-path="config.queue""#));
		assert!(!html.contains(r#"data-action="opt-remove""#));
		assert!(!html.contains(r#"data-path="config.queue.size""#));

		assert_eq!(form.set_present("config.queue", true), Ok(()));
		let html = form_html(&form);
		assert!(html.contains(r#"data-action="opt-remove" data-path="config.queue""#));
		assert!(html.contains(r#"data-path="config.queue.size""#));
		assert!(!html.contains(r#"data-action="opt-add" data-path="config.queue""#));
	}

	#[test]
	fn lists_have_add_and_per_entry_remove_buttons() {
		let mut form = sample_form();
		assert!(form_html(&form).contains(r#"data-action="list-add" data-path="config.entries""#));
		assert!(form_html(&form).contains("No entries yet"));
		assert_eq!(form.list_add("config.entries"), Ok(0));
		assert_eq!(form.list_add("config.entries"), Ok(1));
		let html = form_html(&form);
		assert!(html.contains(r#"data-action="list-remove" data-path="config.entries.0""#));
		assert!(html.contains(r#"data-action="list-remove" data-path="config.entries.1""#));
		assert!(html.contains(r#"data-path="config.entries.1.name""#));
		assert!(html.contains("Entries #2"));
	}

	#[test]
	fn enum_select_lists_all_variants_and_the_fields_of_the_selected_one() {
		let mut form = sample_form();
		assert_eq!(form.set_variant("config.login", "Basic"), Ok(()));
		let html = form_html(&form);
		let select = html.split(r#"id="f-config.login""#).nth(1).unwrap_or_default().split("</select>").next().unwrap_or_default();
		assert_eq!(count(select, "<option"), 4);
		assert!(select.contains(r#"<option value="Basic" selected>"#));
		assert!(!select.contains("not set"));
		assert!(html.contains(r#"data-path="config.login.Basic.user""#));
		assert!(!html.contains(r#"data-path="config.login.Bearer""#));

		// The optional enum has an empty option
		let select = html.split(r#"id="f-config.auth""#).nth(1).unwrap_or_default().split("</select>").next().unwrap_or_default();
		assert_eq!(count(select, "<option"), 5);
		assert!(select.contains(r#"<option value="" selected>"#));
	}

	#[test]
	fn tuple_and_tagged_enum_fields() {
		let mut form = sample_form();
		assert_eq!(form.set_variant("config.login", "Equals"), Ok(()));
		assert_eq!(form.set_present("config.pair", true), Ok(()));
		assert_eq!(form.list_add("config.fields"), Ok(0));
		assert_eq!(form.set_variant("config.fields.0", "Number"), Ok(()));
		let html = form_html(&form);
		assert!(html.contains(r#"data-path="config.login.Equals.0""#));
		assert!(html.contains(r#"data-path="config.login.Equals.1""#));
		assert!(html.contains(r#"data-path="config.pair.1" data-kind="text" data-type="uint""#));
		assert!(html.contains(r#"data-path="config.fields.0.Number.min""#));
		assert!(html.contains(r#"data-path="config.fields.0" data-kind="enum""#));
	}

	#[test]
	fn bool_is_a_checkbox_when_required_and_a_select_when_optional() {
		let mut form = sample_form();
		assert_eq!(form.set_text("config.active", "true"), Ok(()));
		assert_eq!(form.set_text("config.follow", "false"), Ok(()));
		let html = form_html(&form);
		assert!(html.contains(r#"type="checkbox" id="f-config.active""#));
		assert!(html.contains(r#"data-kind="check""#));
		assert!(html.contains("checked"));
		let select = html.split(r#"id="f-config.follow""#).nth(1).unwrap_or_default().split("</select>").next().unwrap_or_default();
		assert!(select.contains(r#"<option value="false" selected>"#));
	}

	#[test]
	fn user_input_is_escaped() {
		let mut form = sample_form();
		let nasty = r#""><script>alert('x')</script>&"#;
		assert_eq!(form.set_text("config.name", nasty), Ok(()));
		let html = form_html(&form);
		assert!(!html.contains("<script>"));
		assert!(html.contains("&quot;&gt;&lt;script&gt;alert(&#39;x&#39;)&lt;/script&gt;&amp;"));
		let issue = Issue::new("config.name", "<b>bad</b>");
		let messages = messages_html(Level::Error, "<i>x</i>", &[issue]);
		assert!(!messages.contains("<b>") && !messages.contains("<i>"));
	}

	#[test]
	fn render_path_renders_fields_and_list_entries() {
		let mut form = sample_form();
		assert_eq!(form.list_add("config.entries"), Ok(0));
		assert_eq!(form.list_add("config.fields"), Ok(0));

		let html = render_path(&form, "config.entries").unwrap_or_default();
		assert!(html.starts_with(r#"<div class="mb-4 border border-slate-300 rounded-lg shadow-sm" data-node="config.entries""#));

		let html = render_path(&form, "config.entries.0").unwrap_or_default();
		assert!(html.contains(r#"data-node="config.entries.0""#));
		assert!(html.contains(r#"data-action="list-remove""#));

		// A tagged enum as entry of a list is a list entry
		let html = render_path(&form, "config.fields.0").unwrap_or_default();
		assert!(html.contains(r#"data-action="list-remove" data-path="config.fields.0""#));
		assert!(html.contains(r#"data-kind="enum""#));

		let html = render_path(&form, "config.queue").unwrap_or_default();
		assert!(html.contains("opt-add"));

		assert!(render_path(&form, "config.entries.5").is_err());
		assert!(render_path(&form, "nothing").is_err());
	}

	#[test]
	fn every_implemented_kind_renders_a_complete_form() {
		for kind in kinds::all().iter().filter(|k| k.implemented()) {
			let form = Form::new((kind.schema)());
			let html = form_html(&form);
			assert!(html.contains(r#"data-path="config.name""#), "{}", kind.id);
			assert_eq!(count(&html, "<details"), count(&html, "</details>"), "{}", kind.id);
			assert_eq!(count(&html, "<div"), count(&html, "</div>"), "{}", kind.id);
			assert!(!html.contains("Unit"), "{}", kind.id);
		}
	}

	#[test]
	fn docs_describe_type_and_required() {
		let form = sample_form();
		let (title, html) = doc_html(&form, "config.login").unwrap_or_default();
		assert_eq!(title, "Login");
		assert!(html.contains("Choice") && html.contains("Basic") && html.contains("Yes"));
		let (_, html) = doc_html(&form, "config.port").unwrap_or_default();
		assert!(html.contains("8080") && html.contains("No, can be left empty"));
		assert!(doc_html(&form, "config.unknown").is_none());
	}

	#[test]
	fn messages_link_to_fields() {
		let html = messages_html(Level::Warning, "Check", &[Issue::new("config.parser.0.name", "missing"), Issue::new("", "general")]);
		assert!(html.contains(r#"data-action="goto" data-path="config.parser.0.name""#));
		assert!(html.contains("Parser #1 › Name"));
		assert!(html.contains("<li>general</li>"));
		assert!(!messages_html(Level::Info, "Only a heading", &[]).contains("<ul"));
	}

	#[test]
	fn chooser_and_navigation() {
		let found: Vec<&ConfigKind> = kinds::all().iter().take(2).collect();
		let html = chooser_html("a\"b.toml", &found, 1);
		assert_eq!(count(&html, "choose-kind"), 2);
		assert_eq!(count(&html, "bg-teal-600"), 1);
		assert!(html.contains("a&quot;b.toml"));

		let html = reopen_html(&found, found[0].id);
		assert_eq!(count(&html, "choose-kind"), 1);
		assert!(!html.contains(&format!(r#"data-kind="{}""#, found[0].id)));
		assert_eq!(reopen_html(&found[..1], found[0].id), "");
		assert_eq!(count(&nav_buttons_html(kinds::all()), r#"data-action="new""#), kinds::all().len());
	}
}
