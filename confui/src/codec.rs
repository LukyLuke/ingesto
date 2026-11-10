use serde_json::Value as Json;
use serde_yaml::value::{Tag, TaggedValue};
use serde_yaml::{Mapping, Number, Value};
use shared::types::{ConfStruct, ConfType};

use crate::kinds::ConfigKind;
use crate::model::{Form, Node, peel};
use crate::types::{Format, Issue};
use crate::utils::join_path;

const REQUIRED: &str = "This field is required";

// ---------------------------------------------------------------------------------------------
// Form -> Configuration
// ---------------------------------------------------------------------------------------------

/// Generates the configuration value of a form or returns all problems which have to be fixed first.
///
/// The value depends on the format because an enum with a value is a table `{ Variant = value }` in TOML,
/// but a YAML tag `!Variant value` for the YAML parser which reads the configurations.
pub fn form_to_value(form: &Form, format: Format) -> Result<Value, Vec<Issue>> {
	let mut issues = Vec::new();
	let value = emit(&form.schema, &form.root, "", format, &mut issues);
	if issues.is_empty() {
		Ok(value.unwrap_or(Value::Mapping(Mapping::new())))
	} else {
		Err(issues)
	}
}

fn number_error(path: &str, what: &str) -> Issue {
	Issue::new(path, format!("Not a valid {what}"))
}

fn emit_text(
	inner: &ConfType,
	optional: bool,
	text: &str,
	path: &str,
	issues: &mut Vec<Issue>,
) -> Option<Value> {
	if text.is_empty() {
		if !optional {
			issues.push(Issue::new(path, REQUIRED));
		}
		return None;
	}

	match inner {
		ConfType::Bool(_) => match text {
			"true" => Some(Value::Bool(true)),
			"false" => Some(Value::Bool(false)),
			_ => {
				issues.push(Issue::new(path, "Must be true or false"));
				None
			}
		},
		ConfType::UInt(_) => match text.trim().parse::<u64>() {
			Ok(n) => Some(Value::Number(Number::from(n))),
			Err(_) => {
				issues.push(number_error(path, "positive whole number"));
				None
			}
		},
		ConfType::Int(_) => match text.trim().parse::<i64>() {
			Ok(n) => Some(Value::Number(Number::from(n))),
			Err(_) => {
				issues.push(number_error(path, "whole number"));
				None
			}
		},
		ConfType::Float(_) => match text.trim().parse::<f64>() {
			Ok(n) if n.is_finite() => Some(Value::Number(Number::from(n))),
			_ => {
				issues.push(number_error(path, "number"));
				None
			}
		},
		ConfType::RegEx(_) => match regex::Regex::new(text) {
			Ok(_) => Some(Value::String(text.to_owned())),
			Err(e) => {
				let message = e.to_string();
				let first = message
					.lines()
					.last()
					.unwrap_or("invalid regular expression")
					.trim();
				issues.push(Issue::new(
					path,
					format!("Invalid regular expression: {first}"),
				));
				None
			}
		},
		_ => Some(Value::String(text.to_owned())),
	}
}

fn emit(
	ty: &ConfType,
	node: &Node,
	path: &str,
	format: Format,
	issues: &mut Vec<Issue>,
) -> Option<Value> {
	let (inner, optional) = peel(ty);
	match (node, inner) {
		(Node::Unit, _) | (Node::Opt(None), _) => None,
		(Node::Opt(Some(child)), _) => emit(inner, child, path, format, issues),
		(Node::Text(text), _) => emit_text(inner, optional, text, path, issues),
		(Node::Struct(fields), ConfType::Struct(schema)) => {
			let mut map = Mapping::new();
			for (name, child) in fields {
				let Some(child_ty) = schema.get(name) else {
					continue;
				};
				if let Some(value) = emit(child_ty, child, &join_path(path, name), format, issues) {
					map.insert(Value::String(name.clone()), value);
				}
			}
			Some(Value::Mapping(map))
		}
		(Node::List(items), ConfType::Vec(item_ty)) => {
			if items.is_empty() {
				if !optional {
					issues.push(Issue::new(path, "At least one entry is required"));
				}
				return None;
			}
			let values: Vec<Value> = items
				.iter()
				.enumerate()
				.filter_map(|(index, item)| {
					emit(
						item_ty,
						item,
						&join_path(path, &index.to_string()),
						format,
						issues,
					)
				})
				.collect();
			Some(Value::Sequence(values))
		}
		(Node::Tuple(items), ConfType::Tuple(types)) => {
			let values: Vec<Value> = items
				.iter()
				.zip(types)
				.enumerate()
				.filter_map(|(index, (item, item_ty))| {
					emit(
						item_ty,
						item,
						&join_path(path, &index.to_string()),
						format,
						issues,
					)
				})
				.collect();
			Some(Value::Sequence(values))
		}
		(Node::Enum { variant, payload }, ConfType::Enum(variants, _)) => {
			let variant_ty = variants.get(variant)?;
			if matches!(peel(variant_ty).0, ConfType::EnumValue) {
				return Some(Value::String(variant.clone()));
			}
			let value = emit(
				variant_ty,
				payload,
				&join_path(path, variant),
				format,
				issues,
			)?;
			if format == Format::Yaml {
				return Some(Value::Tagged(Box::new(TaggedValue {
					tag: Tag::new(variant),
					value,
				})));
			}
			let mut map = Mapping::new();
			map.insert(Value::String(variant.clone()), value);
			Some(Value::Mapping(map))
		}
		(Node::Enum { variant, payload }, ConfType::Tagged(tag, variants, _)) => {
			let variant_ty = variants.get(variant)?;
			let mut map = Mapping::new();
			map.insert(
				Value::String((*tag).to_owned()),
				Value::String(variant.clone()),
			);
			if let Some(Value::Mapping(fields)) = emit(
				variant_ty,
				payload,
				&join_path(path, variant),
				format,
				issues,
			) {
				map.extend(fields);
			}
			Some(Value::Mapping(map))
		}
		_ => {
			issues.push(Issue::new(
				path,
				"The value does not match the type of the field",
			));
			None
		}
	}
}

/// Converts the configuration value to the text of a file
pub fn to_text(value: &Value, format: Format) -> Result<String, String> {
	match format {
		Format::Toml => toml::to_string(value).map_err(|e| e.to_string()),
		Format::Yaml => serde_yaml::to_string(value).map_err(|e| e.to_string()),
	}
}

// ---------------------------------------------------------------------------------------------
// Configuration -> Form
// ---------------------------------------------------------------------------------------------

/// Everything which did not fit the schema while loading a file
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadReport {
	/// Required values which are not in the file
	pub missing: Vec<String>,
	/// Keys in the file which are not known and therefore ignored
	pub unknown: Vec<String>,
	/// Values of the wrong type which have been ignored
	pub mismatched: Vec<String>,
}

impl LoadReport {
	/// A number to compare how good a file fits a schema: lower is better
	pub fn score(&self) -> usize {
		self.missing.len() + self.unknown.len() + self.mismatched.len()
	}

	#[cfg(test)]
	pub fn is_clean(&self) -> bool {
		self.score() == 0
	}
}

/// Parses the text of a file into a generic value. YAML tags (`!Basic {..}`) become `{ Basic: {..} }`.
pub fn parse_text(text: &str, format: Format) -> Result<Json, String> {
	match format {
		Format::Toml => toml::from_str::<Json>(text).map_err(|e| e.to_string()),
		Format::Yaml => {
			let value = serde_yaml::from_str::<Value>(text).map_err(|e| e.to_string())?;
			yaml_to_json(&value)
		}
	}
}

fn yaml_to_json(value: &Value) -> Result<Json, String> {
	Ok(match value {
		Value::Null => Json::Null,
		Value::Bool(b) => Json::Bool(*b),
		Value::Number(n) => match (n.as_i64(), n.as_u64(), n.as_f64()) {
			(Some(i), _, _) => Json::from(i),
			(_, Some(u), _) => Json::from(u),
			(_, _, Some(f)) => serde_json::Number::from_f64(f)
				.map(Json::Number)
				.ok_or_else(|| format!("The number {f} is not supported"))?,
			_ => return Err("Unsupported number".to_owned()),
		},
		Value::String(s) => Json::String(s.clone()),
		Value::Sequence(items) => {
			Json::Array(items.iter().map(yaml_to_json).collect::<Result<_, _>>()?)
		}
		Value::Mapping(map) => {
			let mut out = serde_json::Map::new();
			for (key, value) in map {
				let key = match key {
					Value::String(s) => s.clone(),
					Value::Number(n) => n.to_string(),
					Value::Bool(b) => b.to_string(),
					_ => return Err("Only text is supported as key".to_owned()),
				};
				out.insert(key, yaml_to_json(value)?);
			}
			Json::Object(out)
		}
		Value::Tagged(tagged) => {
			let name = tagged.tag.to_string();
			let mut out = serde_json::Map::new();
			out.insert(
				name.trim_start_matches('!').to_owned(),
				yaml_to_json(&tagged.value)?,
			);
			Json::Object(out)
		}
	})
}

/// Builds a form with the values of a loaded file. Values which do not fit are reported and left empty.
pub fn form_from_value(schema: ConfStruct, value: &Json) -> (Form, LoadReport) {
	let mut report = LoadReport::default();
	let schema = ConfType::Struct(schema);
	let root = load(&schema, Some(value), "", &mut report);
	(Form { schema, root }, report)
}

fn scalar_text(value: &Json) -> Option<String> {
	match value {
		Json::String(s) => Some(s.clone()),
		Json::Number(n) => Some(n.to_string()),
		Json::Bool(b) => Some(b.to_string()),
		_ => None,
	}
}

fn wrap_present(inner: &ConfType, optional: bool, node: Node) -> Node {
	if optional
		&& matches!(
			inner,
			ConfType::Struct(_) | ConfType::Tuple(_) | ConfType::Enum(..) | ConfType::Tagged(..)
		) {
		Node::Opt(Some(Box::new(node)))
	} else {
		node
	}
}

fn load(ty: &ConfType, value: Option<&Json>, path: &str, report: &mut LoadReport) -> Node {
	let (inner, optional) = peel(ty);
	let value = match value {
		None | Some(Json::Null) => {
			if !optional && !matches!(inner, ConfType::EnumValue) {
				report.missing.push(path.to_owned());
			}
			return Node::new(ty);
		}
		Some(value) => value,
	};

	let mismatch = |report: &mut LoadReport| {
		report.mismatched.push(path.to_owned());
		Node::new(ty)
	};

	match inner {
		ConfType::String(_) | ConfType::RegEx(_) => match scalar_text(value) {
			Some(text) => Node::Text(text),
			None => mismatch(report),
		},
		ConfType::Bool(_) => match value {
			Json::Bool(b) => Node::Text(b.to_string()),
			_ => mismatch(report),
		},
		ConfType::UInt(_) | ConfType::Int(_) | ConfType::Float(_) => match value {
			Json::Number(n) => Node::Text(n.to_string()),
			_ => mismatch(report),
		},
		ConfType::EnumValue => Node::Unit,
		ConfType::Struct(schema) => match value {
			Json::Object(map) => wrap_present(
				inner,
				optional,
				load_struct(schema, map, None, path, report),
			),
			_ => mismatch(report),
		},
		ConfType::Tuple(types) => match value {
			Json::Array(items) if items.len() == types.len() => {
				let nodes = types
					.iter()
					.zip(items)
					.enumerate()
					.map(|(i, (ty, item))| {
						load(ty, Some(item), &join_path(path, &i.to_string()), report)
					})
					.collect();
				wrap_present(inner, optional, Node::Tuple(nodes))
			}
			_ => mismatch(report),
		},
		ConfType::Vec(item_ty) => match value {
			Json::Array(items) => Node::List(
				items
					.iter()
					.enumerate()
					.map(|(i, item)| {
						load(
							item_ty,
							Some(item),
							&join_path(path, &i.to_string()),
							report,
						)
					})
					.collect(),
			),
			_ => mismatch(report),
		},
		ConfType::Enum(variants, _) => {
			let found = match value {
				Json::String(name) => variants
					.get(name)
					.filter(|ty| matches!(peel(ty).0, ConfType::EnumValue))
					.map(|_| (name.as_str(), None)),
				Json::Object(map) if map.len() == 1 => {
					map.iter().next().and_then(|(name, payload)| {
						variants.get(name).map(|_| (name.as_str(), Some(payload)))
					})
				}
				_ => None,
			};
			match found {
				Some((name, payload)) => {
					let variant_ty = variants.get(name).unwrap_or(&ConfType::EnumValue);
					let payload_path = join_path(path, name);
					let payload = match (peel(variant_ty).0, payload) {
						(ConfType::EnumValue, _) => Node::Unit,
						(_, payload) => load(variant_ty, payload, &payload_path, report),
					};
					wrap_present(
						inner,
						optional,
						Node::Enum {
							variant: name.to_owned(),
							payload: Box::new(payload),
						},
					)
				}
				None => mismatch(report),
			}
		}
		ConfType::Tagged(tag, variants, _) => {
			let found = match value {
				Json::Object(map) => map
					.get(*tag)
					.and_then(Json::as_str)
					.and_then(|name| variants.get(name).map(|ty| (name, ty, map))),
				_ => None,
			};
			match found {
				Some((name, variant_ty, map)) => {
					let payload_path = join_path(path, name);
					let payload = match peel(variant_ty).0 {
						ConfType::Struct(schema) => {
							load_struct(schema, map, Some(tag), &payload_path, report)
						}
						_ => Node::Unit,
					};
					wrap_present(
						inner,
						optional,
						Node::Enum {
							variant: name.to_owned(),
							payload: Box::new(payload),
						},
					)
				}
				None => mismatch(report),
			}
		}
		ConfType::Option(_) => mismatch(report),
	}
}

fn load_struct(
	schema: &ConfStruct,
	map: &serde_json::Map<String, Json>,
	ignore: Option<&str>,
	path: &str,
	report: &mut LoadReport,
) -> Node {
	for key in map.keys() {
		if schema.get(key).is_none() && ignore != Some(key.as_str()) {
			report.unknown.push(join_path(path, key));
		}
	}
	Node::Struct(
		schema
			.iter()
			.map(|(name, ty)| {
				(
					name.clone(),
					load(ty, map.get(name), &join_path(path, name), report),
				)
			})
			.collect(),
	)
}

/// How well a loaded file fits a configuration type
#[derive(Clone, Copy)]
pub struct Fit {
	pub kind: &'static ConfigKind,
	/// Problems directly below the root (`config.api`): These say most about the type of a file
	pub top_level: usize,
	/// All problems
	pub total: usize,
}

impl Fit {
	fn rank(&self) -> (usize, usize) {
		(self.top_level, self.total)
	}
}

fn is_top_level(path: &str) -> bool {
	path.matches('.').count() <= 1
}

/// All implemented configuration types, the best fit for a loaded file first. The keys directly below
/// `config` are compared before the problems deeper inside, so a file with a few wrong values in
/// it's `api` still is a polling configuration.
pub fn rank(value: &Json, kinds: &'static [ConfigKind]) -> Vec<Fit> {
	let mut fits: Vec<Fit> = kinds
		.iter()
		.filter(|kind| kind.implemented())
		.map(|kind| {
			let report = form_from_value((kind.schema)(), value).1;
			let all = report
				.missing
				.iter()
				.chain(&report.unknown)
				.chain(&report.mismatched);
			Fit {
				kind,
				top_level: all.filter(|path| is_top_level(path)).count(),
				total: report.score(),
			}
		})
		.collect();
	fits.sort_by_key(Fit::rank);
	fits
}

/// The number of types at the beginning of the ranking which fit equally well. There can be more than
/// one if the files of different types are structurally alike.
pub fn best_count(fits: &[Fit]) -> usize {
	match fits.first() {
		Some(first) => fits
			.iter()
			.take_while(|fit| fit.rank() == first.rank())
			.count(),
		None => 0,
	}
}

/// The configuration types which fit best to a loaded file
#[cfg(test)]
pub fn detect(value: &Json, kinds: &'static [ConfigKind]) -> Vec<&'static ConfigKind> {
	let fits = rank(value, kinds);
	fits.iter()
		.take(best_count(&fits))
		.map(|fit| fit.kind)
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::kinds;
	use crate::model::tests::{sample_form, sample_schema};

	fn json(text: &str) -> Json {
		serde_yaml::from_str(text).unwrap_or(Json::Null)
	}

	fn fill_minimal(form: &mut Form) {
		assert_eq!(form.set_text("config.name", "n"), Ok(()));
		assert_eq!(form.list_add("config.entries"), Ok(0));
		assert_eq!(form.set_text("config.entries.0.name", "e"), Ok(()));
	}

	#[test]
	fn empty_form_lists_all_required_fields() {
		let issues = form_to_value(&sample_form(), Format::Toml)
			.err()
			.unwrap_or_default();
		let paths: Vec<&str> = issues.iter().map(|i| i.path.as_str()).collect();
		assert_eq!(paths, vec!["config.name", "config.entries"]);
		assert_eq!(issues[0].message, REQUIRED);
	}

	#[test]
	fn minimal_form_omits_all_optional_values() {
		let mut form = sample_form();
		fill_minimal(&mut form);
		let value = form_to_value(&form, Format::Yaml).unwrap_or(Value::Null);
		let text = to_text(&value, Format::Yaml).unwrap_or_default();
		assert_eq!(
			text,
			"config:\n  name: n\n  active: false\n  login: None\n  entries:\n  - name: e\n"
		);
	}

	#[test]
	fn all_optional_values_are_written_in_both_formats() {
		let mut form = sample_form();
		fill_minimal(&mut form);
		for (path, value) in [
			("config.port", "8081"),
			("config.ratio", "0.5"),
			("config.follow", "true"),
			("config.matcher", "^a.*$"),
		] {
			assert_eq!(form.set_text(path, value), Ok(()));
		}
		assert_eq!(form.set_variant("config.auth", "Basic"), Ok(()));
		assert_eq!(form.set_text("config.auth.Basic.user", "u"), Ok(()));
		assert_eq!(form.set_text("config.auth.Basic.pass", "p"), Ok(()));
		assert_eq!(form.set_variant("config.login", "Equals"), Ok(()));
		assert_eq!(form.set_text("config.login.Equals.0", "l"), Ok(()));
		assert_eq!(form.set_text("config.login.Equals.1", "r"), Ok(()));
		assert_eq!(form.set_present("config.queue", true), Ok(()));
		assert_eq!(form.list_add("config.fields"), Ok(0));
		assert_eq!(form.set_variant("config.fields.0", "Number"), Ok(()));
		assert_eq!(form.set_text("config.fields.0.Number.name", "f"), Ok(()));
		assert_eq!(form.set_text("config.fields.0.Number.min", "-1"), Ok(()));

		let value = form_to_value(&form, Format::Toml).unwrap_or(Value::Null);
		let toml = to_text(&value, Format::Toml).unwrap_or_default();
		let from_toml = parse_text(&toml, Format::Toml).unwrap_or(Json::Null);
		let value = form_to_value(&form, Format::Yaml).unwrap_or(Value::Null);
		let yaml = to_text(&value, Format::Yaml).unwrap_or_default();
		assert!(
			yaml.contains("!Basic") && yaml.contains("!Equals"),
			"{yaml}"
		);
		let from_yaml = parse_text(&yaml, Format::Yaml).unwrap_or(Json::Null);
		assert_eq!(from_toml, from_yaml);
		assert_eq!(from_toml["config"]["port"], 8081);
		assert_eq!(from_toml["config"]["ratio"], 0.5);
		assert_eq!(from_toml["config"]["auth"]["Basic"]["user"], "u");
		assert_eq!(
			from_toml["config"]["login"]["Equals"],
			serde_json::json!(["l", "r"])
		);
		assert_eq!(from_toml["config"]["queue"], serde_json::json!({}));
		assert_eq!(
			from_toml["config"]["fields"][0],
			serde_json::json!({"kind": "Number", "name": "f", "min": -1})
		);
	}

	#[test]
	fn invalid_values_are_reported_with_their_path() {
		let mut form = sample_form();
		fill_minimal(&mut form);
		assert_eq!(form.set_text("config.port", "-1"), Ok(()));
		assert_eq!(form.set_text("config.ratio", "abc"), Ok(()));
		assert_eq!(form.set_text("config.matcher", "(unclosed"), Ok(()));
		assert_eq!(form.set_variant("config.login", "Basic"), Ok(()));
		assert_eq!(form.set_text("config.login.Basic.user", "u"), Ok(()));
		assert_eq!(form.list_add("config.entries"), Ok(1));

		let issues = form_to_value(&form, Format::Toml).err().unwrap_or_default();
		let paths: Vec<&str> = issues.iter().map(|i| i.path.as_str()).collect();
		assert_eq!(
			paths,
			vec![
				"config.port",
				"config.ratio",
				"config.matcher",
				"config.login.Basic.pass",
				"config.entries.1.name"
			]
		);
		assert!(issues[2].message.starts_with("Invalid regular expression"));
	}

	#[test]
	fn number_edge_cases() {
		let mut form = sample_form();
		fill_minimal(&mut form);
		for (value, ok) in [
			("0", true),
			("18446744073709551615", true),
			("18446744073709551616", false),
			(" 7 ", true),
			("1.5", false),
			("1e3", false),
			("+5", true),
		] {
			assert_eq!(form.set_text("config.port", value), Ok(()));
			assert_eq!(
				form_to_value(&form, Format::Toml).is_ok(),
				ok,
				"port {value}"
			);
		}
		for (value, ok) in [
			("1e3", true),
			("-0.5", true),
			("NaN", false),
			("inf", false),
			("1,5", false),
		] {
			assert_eq!(form.set_text("config.port", ""), Ok(()));
			assert_eq!(form.set_text("config.ratio", value), Ok(()));
			assert_eq!(
				form_to_value(&form, Format::Toml).is_ok(),
				ok,
				"ratio {value}"
			);
		}
	}

	#[test]
	fn required_list_may_not_be_empty_but_optional_may() {
		let mut form = sample_form();
		assert_eq!(form.set_text("config.name", "n"), Ok(()));
		let issues = form_to_value(&form, Format::Toml).err().unwrap_or_default();
		assert_eq!(
			issues,
			vec![Issue::new(
				"config.entries",
				"At least one entry is required"
			)]
		);
	}

	#[test]
	fn strings_with_special_characters_survive_both_formats() {
		let nasty = "a \"quoted\" # not a comment\n\tsecond: line = 'x' [y] \\ ü 日本";
		let mut form = sample_form();
		fill_minimal(&mut form);
		assert_eq!(form.set_text("config.name", nasty), Ok(()));
		for format in [Format::Toml, Format::Yaml] {
			let value = form_to_value(&form, format).unwrap_or(Value::Null);
			let text = to_text(&value, format).unwrap_or_default();
			let parsed = parse_text(&text, format).unwrap_or(Json::Null);
			assert_eq!(parsed["config"]["name"], nasty, "{format:?}");
		}
	}

	#[test]
	fn load_and_save_roundtrip() {
		let mut form = sample_form();
		fill_minimal(&mut form);
		assert_eq!(form.set_variant("config.auth", "Bearer"), Ok(()));
		assert_eq!(form.set_text("config.auth.Bearer", "tok"), Ok(()));
		assert_eq!(form.set_present("config.pair", true), Ok(()));
		assert_eq!(form.set_text("config.pair.0", "x"), Ok(()));
		assert_eq!(form.set_text("config.pair.1", "3"), Ok(()));
		assert_eq!(form.list_add("config.fields"), Ok(0));
		assert_eq!(form.set_text("config.fields.0.Text.name", "f"), Ok(()));
		assert_eq!(form.list_add("config.entries.0.tags"), Ok(0));
		assert_eq!(form.set_text("config.entries.0.tags.0", "t"), Ok(()));

		for format in [Format::Toml, Format::Yaml] {
			let value = form_to_value(&form, format).unwrap_or(Value::Null);
			let text = to_text(&value, format).unwrap_or_default();
			let parsed = parse_text(&text, format).unwrap_or(Json::Null);
			let (loaded, report) = form_from_value(sample_schema(), &parsed);
			assert!(report.is_clean(), "{format:?}: {report:?}");
			assert_eq!(loaded.root, form.root, "{format:?}");
		}
	}

	#[test]
	fn load_reports_missing_unknown_and_mismatched() {
		let value = json(
			"config:\n  name: n\n  active: false\n  bogus: 1\n  port: abc\n  entries:\n    - name: e\n      extra: x\n  auth: {Basic: {user: u, pass: p, more: 1}}\n  login: Nonexistent\n  queue: [1]\n",
		);
		let (form, report) = form_from_value(sample_schema(), &value);
		assert_eq!(
			report.unknown,
			vec![
				"config.bogus",
				"config.auth.Basic.more",
				"config.entries.0.extra"
			]
		);
		assert_eq!(
			report.mismatched,
			vec!["config.port", "config.login", "config.queue"]
		);
		assert!(report.missing.is_empty(), "{:?}", report.missing);
		// Broken values come back empty / default
		assert_eq!(
			form.locate("config.port").map(|(n, _)| n.clone()),
			Ok(Node::Text(String::new()))
		);
		assert_eq!(
			form.locate("config.queue").map(|(n, _)| n.clone()),
			Ok(Node::Opt(None))
		);
		assert!(
			matches!(form.locate("config.login"), Ok((Node::Enum { variant, .. }, _)) if variant == "None")
		);
	}

	#[test]
	fn load_of_missing_required_values() {
		let (_, report) = form_from_value(sample_schema(), &json("other: 1"));
		assert_eq!(report.missing, vec!["config"]);
		assert_eq!(report.unknown, vec!["other"]);

		let (_, report) = form_from_value(sample_schema(), &json("config: {}"));
		assert_eq!(
			report.missing,
			vec![
				"config.name",
				"config.active",
				"config.login",
				"config.entries"
			]
		);

		for broken in [Json::Null, json("[]"), json("7"), json("\"text\"")] {
			let (form, _) = form_from_value(sample_schema(), &broken);
			assert!(matches!(form.root, Node::Struct(_)));
		}
	}

	#[test]
	fn load_accepts_a_number_for_a_string_but_not_the_other_way() {
		let (form, report) = form_from_value(
			sample_schema(),
			&json(
				"config:\n  name: 5\n  port: '5'\n  active: true\n  login: None\n  entries: []\n",
			),
		);
		assert_eq!(
			form.locate("config.name").map(|(n, _)| n.clone()),
			Ok(Node::Text("5".into()))
		);
		assert_eq!(report.mismatched, vec!["config.port"]);
	}

	#[test]
	fn tagged_unknown_or_missing_tag_is_a_mismatch() {
		let (_, report) = form_from_value(
			sample_schema(),
			&json(
				"config:\n  name: n\n  entries: []\n  fields:\n    - {kind: Nope, name: x}\n    - {name: y}\n    - {kind: Text, name: z}\n",
			),
		);
		assert_eq!(
			report.mismatched,
			vec!["config.fields.0", "config.fields.1"]
		);
	}

	#[test]
	fn detection_prefers_the_best_fit() {
		let polling = json("config:\n  name: x\n  api:\n    - name: a\n      uri: http://x\n");
		let found: Vec<&str> = detect(&polling, kinds::all())
			.iter()
			.map(|k| k.id)
			.collect();
		assert_eq!(found, vec!["recv_polling"]);

		let file = json("config:\n  name: x\n  file:\n    path: /var/log/x.log\n");
		let found: Vec<&str> = detect(&file, kinds::all()).iter().map(|k| k.id).collect();
		assert_eq!(found, vec!["recv_file"]);

		let nonsense = json("hello: world");
		assert!(!detect(&nonsense, kinds::all()).is_empty());
		assert!(detect(&nonsense, &[]).is_empty());
	}

	#[test]
	fn detection_returns_all_ties() {
		let value = json("config:\n  name: x\n  listen: {address: 0.0.0.0}\n");
		let found: Vec<&str> = detect(&value, kinds::all()).iter().map(|k| k.id).collect();
		assert_eq!(found, vec!["recv_webhook", "recv_network"]);

		let value = json("config:\n  name: x\n  listen: {address: 0.0.0.0, kind: TCP}\n");
		let found: Vec<&str> = detect(&value, kinds::all()).iter().map(|k| k.id).collect();
		assert_eq!(found, vec!["recv_network"]);
	}

	#[test]
	fn yaml_tags_are_loaded_as_enum_variants() {
		let parsed = parse_text("config:\n  auth: !Basic {user: u, pass: p}\n  login: !Bearer tok\n  pair: !Equals [a, b]\n  unit: None\n", Format::Yaml).unwrap_or(Json::Null);
		assert_eq!(
			parsed["config"]["auth"],
			serde_json::json!({"Basic": {"user": "u", "pass": "p"}})
		);
		assert_eq!(
			parsed["config"]["login"],
			serde_json::json!({"Bearer": "tok"})
		);
		assert_eq!(
			parsed["config"]["pair"],
			serde_json::json!({"Equals": ["a", "b"]})
		);
		assert_eq!(parsed["config"]["unit"], "None");
	}

	#[test]
	fn yaml_parsing_edge_cases() {
		assert!(parse_text("a: [", Format::Yaml).is_err());
		assert!(parse_text("[1, 2]: x", Format::Yaml).is_err());
		assert_eq!(
			parse_text("1: x\ntrue: y\n", Format::Yaml).unwrap_or(Json::Null),
			serde_json::json!({"1": "x", "true": "y"})
		);
		assert!(parse_text("a: .inf", Format::Yaml).is_err());
		assert_eq!(
			parse_text("a: 18446744073709551615", Format::Yaml).unwrap_or(Json::Null)["a"],
			18446744073709551615u64
		);
		assert_eq!(
			parse_text("", Format::Yaml).unwrap_or(Json::Bool(true)),
			Json::Null
		);
		assert!(parse_text("a = ", Format::Toml).is_err());
	}

	#[test]
	fn nested_problems_do_not_hide_the_type() {
		// The auth header has the wrong key names, but the keys of the file are those of a polling configuration
		let value = json(
			"config:\n  name: x\n  timer: '0 */1 * * * *'\n  queue: {}\n  api:\n    - name: a\n      uri: http://x\n      auth: {Header: {name: h, value: v}}\n  parser: []\n",
		);
		let found: Vec<&str> = detect(&value, kinds::all()).iter().map(|k| k.id).collect();
		assert_eq!(found, vec!["recv_polling"]);
	}

	#[test]
	fn rank_lists_all_implemented_kinds_best_first() {
		let value = json("config:\n  name: x\n  api:\n    - name: a\n      uri: http://x\n");
		let fits = rank(&value, kinds::all());
		assert_eq!(fits.len(), 5);
		assert_eq!(fits[0].kind.id, "recv_polling");
		assert_eq!(fits[0].total, 0);
		assert!(fits.windows(2).all(|w| w[0].rank() <= w[1].rank()));
		assert!(rank(&value, &[]).is_empty());
	}
}
