//! Checks the schemas of all receivers and exporters against their real configuration types:
//! Fill a form completely (every variant of every enum), generate TOML and YAML, let the real
//! `Config` type parse it and load it again into a form.

use shared::types::ConfType;

use crate::codec::{form_from_value, form_to_value, parse_text, to_text};
use crate::kinds;
use crate::model::{Form, peel};
use crate::types::Format;
use crate::utils::join_path;

#[derive(Clone, Copy)]
enum Mode {
	/// Only the required fields
	Minimal,
	/// All fields, the number selects which variant of each enum is used
	Full(usize),
}

fn populate(form: &mut Form, path: &str, ty: &ConfType, mode: Mode) {
	let (inner, optional) = peel(ty);
	if optional && matches!(mode, Mode::Minimal) {
		return;
	}

	match inner {
		ConfType::Struct(fields) => {
			if optional {
				assert_eq!(form.set_present(path, true), Ok(()), "{path}");
			}
			for (name, field_ty) in fields {
				populate(form, &join_path(path, name), field_ty, mode);
			}
		}
		ConfType::Tuple(types) => {
			if optional {
				assert_eq!(form.set_present(path, true), Ok(()), "{path}");
			}
			for (index, item_ty) in types.iter().enumerate() {
				populate(form, &join_path(path, &index.to_string()), item_ty, mode);
			}
		}
		ConfType::Vec(item_ty) => {
			let entries = if matches!(mode, Mode::Minimal) { 1 } else { 2 };
			for _ in 0..entries {
				let index = form.list_add(path).unwrap_or_default();
				populate(form, &join_path(path, &index.to_string()), item_ty, mode);
			}
		}
		ConfType::Enum(variants, default) | ConfType::Tagged(_, variants, default) => {
			let names: Vec<&String> = variants.iter().map(|(name, _)| name).collect();
			let variant = match mode {
				Mode::Minimal => (*default).to_owned(),
				Mode::Full(pick) => names[pick % names.len()].clone(),
			};
			assert_eq!(form.set_variant(path, &variant), Ok(()), "{path}");
			if let Some(variant_ty) = variants.get(&variant)
				&& !matches!(peel(variant_ty).0, ConfType::EnumValue)
			{
				populate(form, &join_path(path, &variant), variant_ty, mode);
			}
		}
		ConfType::Bool(_) => assert_eq!(form.set_text(path, "true"), Ok(()), "{path}"),
		ConfType::UInt(_) => assert_eq!(form.set_text(path, "5"), Ok(()), "{path}"),
		ConfType::Int(_) => assert_eq!(form.set_text(path, "-5"), Ok(()), "{path}"),
		ConfType::Float(_) => assert_eq!(form.set_text(path, "1.5"), Ok(()), "{path}"),
		ConfType::RegEx(_) => assert_eq!(form.set_text(path, "^a.*$"), Ok(()), "{path}"),
		ConfType::String(_) => assert_eq!(form.set_text(path, "text"), Ok(()), "{path}"),
		ConfType::EnumValue | ConfType::Option(_) => {}
	}
}

#[test]
fn every_kind_roundtrips_in_every_variant_and_format() {
	let mut checked = 0;
	for kind in kinds::all().iter().filter(|k| k.implemented()) {
		let modes = std::iter::once(Mode::Minimal).chain((0..12).map(Mode::Full));
		for (round, mode) in modes.enumerate() {
			let mut form = Form::new((kind.schema)());
			let schema = form.schema.clone();
			let ConfType::Struct(root) = &schema else {
				panic!("root must be a struct")
			};
			for (name, ty) in root {
				populate(&mut form, name, ty, mode);
			}

			for format in [Format::Toml, Format::Yaml] {
				let value = match form_to_value(&form, format) {
					Ok(value) => value,
					Err(issues) => panic!("{} round {round}: {issues:?}", kind.id),
				};
				let text = to_text(&value, format).unwrap_or_default();
				if let Err(issue) = (kind.check)(&text, format) {
					panic!("{} round {round} {format:?}: {issue:?}\n{text}", kind.id);
				}

				let parsed = parse_text(&text, format).unwrap_or_default();
				let (loaded, report) = form_from_value((kind.schema)(), &parsed);
				assert!(
					report.is_clean(),
					"{} round {round} {format:?}: {report:?}\n{text}",
					kind.id
				);
				assert_eq!(
					loaded.root, form.root,
					"{} round {round} {format:?}",
					kind.id
				);
				checked += 1;
			}
		}
	}
	assert_eq!(checked, 5 * 13 * 2);
}

#[test]
fn empty_forms_only_complain_about_required_fields() {
	for kind in kinds::all().iter().filter(|k| k.implemented()) {
		let form = Form::new((kind.schema)());
		let issues = form_to_value(&form, Format::Toml).err().unwrap_or_default();
		assert!(!issues.is_empty(), "{}", kind.id);
		assert!(
			issues.iter().all(|i| i.path.starts_with("config.")),
			"{}: {issues:?}",
			kind.id
		);
	}
}

#[test]
fn a_real_world_polling_file_loads_without_errors() {
	let toml = r#"
[config]
name = "NFR SentinelOne API"
timer = "0 */1 * * * *"

[[config.api]]
name = "Agents"
uri = "https://example.com/web/api/v2.1/agents?limit=1000"
method = "GET"
auth = { Header = { header = "Authorization", value = "ApiToken x" } }

[[config.parser]]
name = "agents"
matcher = ".*"
kind = "JSON"

[[config.parser.mapping]]
name = "host"
source = "$.computerName"
"#;
	let value = parse_text(toml, Format::Toml).unwrap_or_default();
	let found = crate::codec::detect(&value, kinds::all());
	assert_eq!(
		found.iter().map(|k| k.id).collect::<Vec<_>>(),
		vec!["recv_polling"]
	);
	let (form, report) = form_from_value((found[0].schema)(), &value);
	assert!(report.is_clean(), "{report:?}");
	assert!(form_to_value(&form, Format::Toml).is_ok());
}

#[test]
fn polling_endpoint_name_is_required() {
	let value = parse_text(
		"[config]\nname = \"p\"\n[[config.api]]\nuri = \"http://x\"\n",
		Format::Toml,
	)
	.unwrap_or_default();
	let (form, report) = form_from_value(
		(kinds::find("recv_polling")
			.map(|k| k.schema)
			.unwrap_or(|| Default::default()))(),
		&value,
	);
	assert_eq!(report.missing, vec!["config.api.0.name"]);
	let issues = form_to_value(&form, Format::Toml).err().unwrap_or_default();
	assert!(
		issues.iter().any(|i| i.path == "config.api.0.name"),
		"{issues:?}"
	);
}
