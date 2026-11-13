use shared::types::{ConfStruct, ConfType};
use thiserror::Error;

use crate::utils::split_path;

/// Errors when changing the form
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
	#[error("there is no field '{0}'")]
	UnknownPath(String),
	#[error("'{0}' is not set, add it first")]
	NotPresent(String),
	#[error("'{path}' is not {expected}")]
	WrongNode {
		path: String,
		expected: &'static str,
	},
	#[error("'{variant}' is not a variant of '{path}'")]
	UnknownVariant { path: String, variant: String },
	#[error("there is no entry {index} in '{path}'")]
	UnknownIndex { path: String, index: usize },
}

/// The value of a (part of the) configuration which is being edited.
///
/// A `Node` has always the same shape as the `ConfType` it was created from.
/// Scalars are kept as the text which is shown in the form, they are parsed when the configuration is generated.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
	/// The value of a variant without a value
	Unit,

	/// A string, regular expression, number or bool (`"true"`/`"false"`). An empty text is "not set".
	Text(String),

	/// An optional struct, tuple or enum
	Opt(Option<Box<Node>>),

	/// A struct with all of it's fields in the order of the schema
	Struct(Vec<(String, Node)>),

	/// A list of entries
	List(Vec<Node>),

	/// A tuple with a fixed number of values
	Tuple(Vec<Node>),

	/// An enum with the selected variant and it's value (also used for tagged enums)
	Enum { variant: String, payload: Box<Node> },
}

/// Removes the `Option` around a type and tells if there was one
pub fn peel(ty: &ConfType) -> (&ConfType, bool) {
	let mut ty = ty;
	let mut optional = false;
	while let ConfType::Option(inner) = ty {
		ty = inner;
		optional = true;
	}
	(ty, optional)
}

/// Types which can't be shown as a simple input and are therefore added and removed
fn is_composite(ty: &ConfType) -> bool {
	matches!(
		ty,
		ConfType::Struct(_) | ConfType::Tuple(_) | ConfType::Enum(..) | ConfType::Tagged(..)
	)
}

/// The variants of an enum
pub fn variants_of(ty: &ConfType) -> Option<&ConfStruct> {
	match peel(ty).0 {
		ConfType::Enum(variants, _) | ConfType::Tagged(_, variants, _) => Some(variants),
		_ => None,
	}
}

impl Node {
	/// The initial node for a field of this type: Required fields are empty/default, optional ones are not set
	pub fn new(ty: &ConfType) -> Node {
		let (inner, optional) = peel(ty);
		if optional && is_composite(inner) {
			Node::Opt(None)
		} else {
			Node::present(ty)
		}
	}

	/// A node with a value, also for optional types
	pub fn present(ty: &ConfType) -> Node {
		let (inner, optional) = peel(ty);
		match inner {
			ConfType::Bool(_) if !optional => Node::Text("false".to_owned()),
			ConfType::Bool(_)
			| ConfType::UInt(_)
			| ConfType::Int(_)
			| ConfType::Float(_)
			| ConfType::String(_)
			| ConfType::RegEx(_) => Node::Text(String::new()),
			ConfType::EnumValue => Node::Unit,
			ConfType::Struct(fields) => Node::Struct(
				fields
					.iter()
					.map(|(name, ty)| (name.clone(), Node::new(ty)))
					.collect(),
			),
			ConfType::Tuple(types) => Node::Tuple(types.iter().map(Node::new).collect()),
			ConfType::Vec(_) => Node::List(Vec::new()),
			ConfType::Enum(_, default) | ConfType::Tagged(_, _, default) => {
				Node::variant(inner, default).unwrap_or(Node::Unit)
			}
			ConfType::Option(_) => Node::Unit,
		}
	}

	/// A node of an enum with the given variant selected
	pub fn variant(ty: &ConfType, variant: &str) -> Option<Node> {
		let variant_ty = variants_of(ty)?.get(variant)?;
		Some(Node::Enum {
			variant: variant.to_owned(),
			payload: Box::new(Node::new(variant_ty)),
		})
	}
}

/// The form: The schema and the current values
#[derive(Debug, Clone)]
pub struct Form {
	pub schema: ConfType,
	pub root: Node,
}

fn locate_in<'a>(
	node: &'a Node,
	ty: &'a ConfType,
	segs: &[&str],
	path: &str,
) -> Result<(&'a Node, &'a ConfType), ModelError> {
	let Some((first, rest)) = segs.split_first() else {
		return Ok((node, ty));
	};
	let inner = peel(ty).0;
	match (node, inner) {
		(Node::Opt(None), _) => Err(ModelError::NotPresent(path.to_owned())),
		(Node::Opt(Some(child)), _) => locate_in(child, ty, segs, path),
		(Node::Struct(fields), ConfType::Struct(schema)) => {
			let child_ty = schema
				.get(first)
				.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
			let (_, child) = fields
				.iter()
				.find(|(name, _)| name == first)
				.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
			locate_in(child, child_ty, rest, path)
		}
		(Node::List(items), ConfType::Vec(item_ty)) => {
			let index: usize = first
				.parse()
				.map_err(|_| ModelError::UnknownPath(path.to_owned()))?;
			let child = items.get(index).ok_or_else(|| ModelError::UnknownIndex {
				path: path.to_owned(),
				index,
			})?;
			locate_in(child, item_ty, rest, path)
		}
		(Node::Tuple(items), ConfType::Tuple(types)) => {
			let index: usize = first
				.parse()
				.map_err(|_| ModelError::UnknownPath(path.to_owned()))?;
			let child = items.get(index).ok_or_else(|| ModelError::UnknownIndex {
				path: path.to_owned(),
				index,
			})?;
			let child_ty = types.get(index).ok_or_else(|| ModelError::UnknownIndex {
				path: path.to_owned(),
				index,
			})?;
			locate_in(child, child_ty, rest, path)
		}
		(
			Node::Enum { variant, payload },
			ConfType::Enum(variants, _) | ConfType::Tagged(_, variants, _),
		) if variant == first => {
			let child_ty = variants
				.get(variant)
				.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
			locate_in(payload, child_ty, rest, path)
		}
		_ => Err(ModelError::UnknownPath(path.to_owned())),
	}
}

fn locate_in_mut<'a>(
	node: &'a mut Node,
	ty: &'a ConfType,
	segs: &[&str],
	path: &str,
) -> Result<(&'a mut Node, &'a ConfType), ModelError> {
	let Some((first, rest)) = segs.split_first() else {
		return Ok((node, ty));
	};
	let inner = peel(ty).0;
	match (node, inner) {
		(Node::Opt(None), _) => Err(ModelError::NotPresent(path.to_owned())),
		(Node::Opt(Some(child)), _) => locate_in_mut(child, ty, segs, path),
		(Node::Struct(fields), ConfType::Struct(schema)) => {
			let child_ty = schema
				.get(first)
				.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
			let (_, child) = fields
				.iter_mut()
				.find(|(name, _)| name == first)
				.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
			locate_in_mut(child, child_ty, rest, path)
		}
		(Node::List(items), ConfType::Vec(item_ty)) => {
			let index: usize = first
				.parse()
				.map_err(|_| ModelError::UnknownPath(path.to_owned()))?;
			let child = items
				.get_mut(index)
				.ok_or_else(|| ModelError::UnknownIndex {
					path: path.to_owned(),
					index,
				})?;
			locate_in_mut(child, item_ty, rest, path)
		}
		(Node::Tuple(items), ConfType::Tuple(types)) => {
			let index: usize = first
				.parse()
				.map_err(|_| ModelError::UnknownPath(path.to_owned()))?;
			let child = items
				.get_mut(index)
				.ok_or_else(|| ModelError::UnknownIndex {
					path: path.to_owned(),
					index,
				})?;
			let child_ty = types.get(index).ok_or_else(|| ModelError::UnknownIndex {
				path: path.to_owned(),
				index,
			})?;
			locate_in_mut(child, child_ty, rest, path)
		}
		(
			Node::Enum { variant, payload },
			ConfType::Enum(variants, _) | ConfType::Tagged(_, variants, _),
		) => {
			if variant != first {
				return Err(ModelError::UnknownPath(path.to_owned()));
			}
			let child_ty = variants
				.get(variant)
				.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
			locate_in_mut(payload, child_ty, rest, path)
		}
		_ => Err(ModelError::UnknownPath(path.to_owned())),
	}
}

impl Form {
	/// An empty form for a schema
	pub fn new(schema: ConfStruct) -> Form {
		let schema = ConfType::Struct(schema);
		let root = Node::new(&schema);
		Form { schema, root }
	}

	/// Finds the node and the type of a path. Struct fields are addressed by name, lists and tuples by index
	/// and the value of an enum by the name of the selected variant: `config.auth.Basic.user`.
	/// Optional values which are set are transparent.
	pub fn locate(&self, path: &str) -> Result<(&Node, &ConfType), ModelError> {
		locate_in(&self.root, &self.schema, &split_path(path), path)
	}

	fn locate_mut(&mut self, path: &str) -> Result<(&mut Node, &ConfType), ModelError> {
		locate_in_mut(&mut self.root, &self.schema, &split_path(path), path)
	}

	/// Sets the text of a string, number or bool
	pub fn set_text(&mut self, path: &str, value: &str) -> Result<(), ModelError> {
		match self.locate_mut(path)? {
			(Node::Text(text), _) => {
				*text = value.to_owned();
				Ok(())
			}
			_ => Err(ModelError::WrongNode {
				path: path.to_owned(),
				expected: "a value",
			}),
		}
	}

	/// Adds a new empty entry at the end of a list and returns it's index
	pub fn list_add(&mut self, path: &str) -> Result<usize, ModelError> {
		match self.locate_mut(path)? {
			(Node::List(items), ty) => {
				let ConfType::Vec(item_ty) = peel(ty).0 else {
					return Err(ModelError::WrongNode {
						path: path.to_owned(),
						expected: "a list",
					});
				};
				items.push(Node::present(item_ty));
				Ok(items.len() - 1)
			}
			_ => Err(ModelError::WrongNode {
				path: path.to_owned(),
				expected: "a list",
			}),
		}
	}

	/// Removes the entry at `path` (`config.parser.1`) from it's list
	pub fn list_remove(&mut self, path: &str) -> Result<(), ModelError> {
		let (list_path, index) = path
			.rsplit_once('.')
			.ok_or_else(|| ModelError::UnknownPath(path.to_owned()))?;
		let index: usize = index
			.parse()
			.map_err(|_| ModelError::UnknownPath(path.to_owned()))?;
		match self.locate_mut(list_path)? {
			(Node::List(items), _) => {
				if index >= items.len() {
					return Err(ModelError::UnknownIndex {
						path: list_path.to_owned(),
						index,
					});
				}
				items.remove(index);
				Ok(())
			}
			_ => Err(ModelError::WrongNode {
				path: list_path.to_owned(),
				expected: "a list",
			}),
		}
	}

	/// Adds or removes an optional struct or tuple
	pub fn set_present(&mut self, path: &str, present: bool) -> Result<(), ModelError> {
		match self.locate_mut(path)? {
			(Node::Opt(slot), ty) => {
				match (present, slot.is_some()) {
					(true, false) => *slot = Some(Box::new(Node::present(peel(ty).0))),
					(false, _) => *slot = None,
					(true, true) => {}
				}
				Ok(())
			}
			_ => Err(ModelError::WrongNode {
				path: path.to_owned(),
				expected: "optional",
			}),
		}
	}

	/// Selects a variant of an enum. An empty variant removes an optional enum.
	/// Selecting the variant which is already selected keeps it's values.
	pub fn set_variant(&mut self, path: &str, variant: &str) -> Result<(), ModelError> {
		let (node, ty) = self.locate_mut(path)?;
		let inner = peel(ty).0;
		let Some(variants) = variants_of(inner) else {
			return Err(ModelError::WrongNode {
				path: path.to_owned(),
				expected: "an enum",
			});
		};
		let unknown = || ModelError::UnknownVariant {
			path: path.to_owned(),
			variant: variant.to_owned(),
		};

		match node {
			Node::Opt(slot) if variant.is_empty() => {
				*slot = None;
				Ok(())
			}
			Node::Opt(slot) => {
				if matches!(slot.as_deref(), Some(Node::Enum { variant: current, .. }) if current == variant)
				{
					return Ok(());
				}
				variants.get(variant).ok_or_else(unknown)?;
				*slot = Some(Box::new(Node::variant(inner, variant).ok_or_else(unknown)?));
				Ok(())
			}
			Node::Enum {
				variant: current, ..
			} => {
				if current == variant {
					return Ok(());
				}
				variants.get(variant).ok_or_else(unknown)?;
				*node = Node::variant(inner, variant).ok_or_else(unknown)?;
				Ok(())
			}
			_ => Err(ModelError::WrongNode {
				path: path.to_owned(),
				expected: "an enum",
			}),
		}
	}
}

#[cfg(test)]
pub(crate) mod tests {
	use super::*;

	fn s(name: &str, ty: ConfType) -> (String, ConfType) {
		(name.to_owned(), ty)
	}

	fn opt(ty: ConfType) -> ConfType {
		ConfType::Option(Box::new(ty))
	}

	/// A schema which contains all kinds of types
	pub(crate) fn sample_schema() -> ConfStruct {
		let auth = ConfType::Enum(
			ConfStruct::from([
				s("None", ConfType::EnumValue),
				s(
					"Basic",
					ConfType::Struct(ConfStruct::from([
						s("user", ConfType::String("")),
						s("pass", ConfType::String("")),
					])),
				),
				s("Bearer", ConfType::String("")),
				s(
					"Equals",
					ConfType::Tuple(vec![ConfType::String(""), ConfType::String("")]),
				),
			]),
			"None",
		);
		let tagged = ConfType::Tagged(
			"kind",
			ConfStruct::from([
				s(
					"Text",
					ConfType::Struct(ConfStruct::from([s("name", ConfType::String(""))])),
				),
				s(
					"Number",
					ConfType::Struct(ConfStruct::from([
						s("name", ConfType::String("")),
						s("min", opt(ConfType::Int(0))),
					])),
				),
			]),
			"Text",
		);
		ConfStruct::from([s(
			"config",
			ConfType::Struct(ConfStruct::from([
				s("name", ConfType::String("")),
				s("port", opt(ConfType::UInt(8080))),
				s("ratio", opt(ConfType::Float(1.0))),
				s("follow", opt(ConfType::Bool(false))),
				s("active", ConfType::Bool(false)),
				s("matcher", opt(ConfType::RegEx(".*"))),
				s("auth", opt(auth.clone())),
				s("login", auth),
				s(
					"queue",
					opt(ConfType::Struct(ConfStruct::from([s(
						"size",
						opt(ConfType::UInt(10)),
					)]))),
				),
				s(
					"entries",
					ConfType::Vec(Box::new(ConfType::Struct(ConfStruct::from([
						s("name", ConfType::String("")),
						s("tags", opt(ConfType::Vec(Box::new(ConfType::String(""))))),
					])))),
				),
				s("fields", opt(ConfType::Vec(Box::new(tagged)))),
				s(
					"pair",
					opt(ConfType::Tuple(vec![
						ConfType::String(""),
						ConfType::UInt(0),
					])),
				),
			])),
		)])
	}

	pub(crate) fn sample_form() -> Form {
		Form::new(sample_schema())
	}

	#[test]
	fn new_form_has_required_defaults_and_unset_optionals() {
		let form = sample_form();
		let Ok((Node::Text(name), _)) = form.locate("config.name") else {
			panic!("name")
		};
		assert_eq!(name, "");
		assert_eq!(
			form.locate("config.active").map(|(n, _)| n.clone()),
			Ok(Node::Text("false".into()))
		);
		// An optional bool is tri-state and starts unset
		assert_eq!(
			form.locate("config.follow").map(|(n, _)| n.clone()),
			Ok(Node::Text(String::new()))
		);
		assert_eq!(
			form.locate("config.queue").map(|(n, _)| n.clone()),
			Ok(Node::Opt(None))
		);
		assert_eq!(
			form.locate("config.auth").map(|(n, _)| n.clone()),
			Ok(Node::Opt(None))
		);
		// A required enum starts with it's default variant
		let Ok((Node::Enum { variant, payload }, _)) = form.locate("config.login") else {
			panic!("login")
		};
		assert_eq!(variant, "None");
		assert_eq!(**payload, Node::Unit);
		assert_eq!(
			form.locate("config.entries").map(|(n, _)| n.clone()),
			Ok(Node::List(vec![]))
		);
	}

	#[test]
	fn fields_keep_the_order_of_the_schema() {
		let form = sample_form();
		let Ok((Node::Struct(fields), _)) = form.locate("config") else {
			panic!("config")
		};
		let names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
		assert_eq!(&names[..4], &["name", "port", "ratio", "follow"]);
	}

	#[test]
	fn set_text_and_errors() {
		let mut form = sample_form();
		assert_eq!(form.set_text("config.name", "abc"), Ok(()));
		assert_eq!(
			form.locate("config.name").map(|(n, _)| n.clone()),
			Ok(Node::Text("abc".into()))
		);
		assert!(matches!(
			form.set_text("config.unknown", "x"),
			Err(ModelError::UnknownPath(_))
		));
		assert!(matches!(
			form.set_text("config", "x"),
			Err(ModelError::WrongNode { .. })
		));
		assert!(matches!(
			form.set_text("config.queue.size", "1"),
			Err(ModelError::NotPresent(_))
		));
		assert!(matches!(
			form.set_text("", "x"),
			Err(ModelError::WrongNode { .. })
		));
		assert!(matches!(
			form.set_text("config.name.deeper", "x"),
			Err(ModelError::UnknownPath(_))
		));
	}

	#[test]
	fn lists_add_and_remove() {
		let mut form = sample_form();
		assert_eq!(form.list_add("config.entries"), Ok(0));
		assert_eq!(form.list_add("config.entries"), Ok(1));
		assert_eq!(form.list_add("config.entries"), Ok(2));
		assert_eq!(form.set_text("config.entries.1.name", "second"), Ok(()));
		assert_eq!(form.set_text("config.entries.2.name", "third"), Ok(()));

		assert_eq!(form.list_remove("config.entries.0"), Ok(()));
		assert_eq!(
			form.locate("config.entries.0.name").map(|(n, _)| n.clone()),
			Ok(Node::Text("second".into()))
		);
		assert!(matches!(
			form.locate("config.entries.2"),
			Err(ModelError::UnknownIndex { index: 2, .. })
		));

		assert!(matches!(
			form.list_remove("config.entries.5"),
			Err(ModelError::UnknownIndex { .. })
		));
		assert!(matches!(
			form.list_remove("config.entries.x"),
			Err(ModelError::UnknownPath(_))
		));
		assert!(matches!(
			form.list_remove("config"),
			Err(ModelError::UnknownPath(_))
		));
		assert!(matches!(
			form.list_remove("config.name.0"),
			Err(ModelError::WrongNode { .. })
		));
		assert!(matches!(
			form.list_add("config.name"),
			Err(ModelError::WrongNode { .. })
		));
	}

	#[test]
	fn optional_lists_inside_list_items() {
		let mut form = sample_form();
		assert_eq!(form.list_add("config.entries"), Ok(0));
		assert_eq!(form.list_add("config.entries.0.tags"), Ok(0));
		assert_eq!(form.set_text("config.entries.0.tags.0", "t"), Ok(()));
		assert_eq!(form.list_remove("config.entries.0.tags.0"), Ok(()));
		assert_eq!(
			form.locate("config.entries.0.tags").map(|(n, _)| n.clone()),
			Ok(Node::List(vec![]))
		);
	}

	#[test]
	fn optional_structs_add_and_remove() {
		let mut form = sample_form();
		assert_eq!(form.set_present("config.queue", true), Ok(()));
		assert_eq!(form.set_text("config.queue.size", "5"), Ok(()));
		// Adding again does not throw away the values
		assert_eq!(form.set_present("config.queue", true), Ok(()));
		assert_eq!(
			form.locate("config.queue.size").map(|(n, _)| n.clone()),
			Ok(Node::Text("5".into()))
		);
		assert_eq!(form.set_present("config.queue", false), Ok(()));
		assert_eq!(
			form.locate("config.queue").map(|(n, _)| n.clone()),
			Ok(Node::Opt(None))
		);
		assert_eq!(form.set_present("config.queue", false), Ok(()));
		// A removed struct comes back empty
		assert_eq!(form.set_present("config.queue", true), Ok(()));
		assert_eq!(
			form.locate("config.queue.size").map(|(n, _)| n.clone()),
			Ok(Node::Text(String::new()))
		);

		assert!(matches!(
			form.set_present("config.name", true),
			Err(ModelError::WrongNode { .. })
		));
		assert!(matches!(
			form.set_present("config.nothing", true),
			Err(ModelError::UnknownPath(_))
		));
	}

	#[test]
	fn optional_tuple() {
		let mut form = sample_form();
		assert_eq!(form.set_present("config.pair", true), Ok(()));
		assert_eq!(form.set_text("config.pair.0", "a"), Ok(()));
		assert_eq!(form.set_text("config.pair.1", "7"), Ok(()));
		assert!(matches!(
			form.set_text("config.pair.2", "7"),
			Err(ModelError::UnknownIndex { .. })
		));
	}

	#[test]
	fn enum_variants() {
		let mut form = sample_form();
		assert_eq!(form.set_variant("config.login", "Basic"), Ok(()));
		assert_eq!(form.set_text("config.login.Basic.user", "me"), Ok(()));
		// Same variant keeps the values
		assert_eq!(form.set_variant("config.login", "Basic"), Ok(()));
		assert_eq!(
			form.locate("config.login.Basic.user")
				.map(|(n, _)| n.clone()),
			Ok(Node::Text("me".into()))
		);
		// Another variant resets them
		assert_eq!(form.set_variant("config.login", "Bearer"), Ok(()));
		assert_eq!(form.set_text("config.login.Bearer", "tok"), Ok(()));
		assert!(matches!(
			form.set_text("config.login.Basic.user", "x"),
			Err(ModelError::UnknownPath(_))
		));
		assert_eq!(form.set_variant("config.login", "Equals"), Ok(()));
		assert_eq!(form.set_text("config.login.Equals.1", "x"), Ok(()));

		assert!(matches!(
			form.set_variant("config.login", "Nope"),
			Err(ModelError::UnknownVariant { .. })
		));
		assert!(matches!(
			form.set_variant("config.login", ""),
			Err(ModelError::UnknownVariant { .. })
		));
		assert!(matches!(
			form.set_variant("config.name", "Basic"),
			Err(ModelError::WrongNode { .. })
		));
	}

	#[test]
	fn optional_enum_can_be_unset() {
		let mut form = sample_form();
		assert_eq!(form.set_variant("config.auth", "Bearer"), Ok(()));
		assert_eq!(form.set_text("config.auth.Bearer", "tok"), Ok(()));
		assert_eq!(form.set_variant("config.auth", "Bearer"), Ok(()));
		assert_eq!(
			form.locate("config.auth.Bearer").map(|(n, _)| n.clone()),
			Ok(Node::Text("tok".into()))
		);
		assert_eq!(form.set_variant("config.auth", ""), Ok(()));
		assert_eq!(
			form.locate("config.auth").map(|(n, _)| n.clone()),
			Ok(Node::Opt(None))
		);
		assert!(matches!(
			form.set_variant("config.auth", "Nope"),
			Err(ModelError::UnknownVariant { .. })
		));
	}

	#[test]
	fn tagged_enum_in_a_list() {
		let mut form = sample_form();
		assert!(
			form.set_present("config.fields", true).is_err(),
			"an optional list is not an Opt-Node"
		);
		assert_eq!(form.list_add("config.fields"), Ok(0));
		assert_eq!(
			form.locate("config.fields.0.Text.name")
				.map(|(n, _)| n.clone()),
			Ok(Node::Text(String::new()))
		);
		assert_eq!(form.set_variant("config.fields.0", "Number"), Ok(()));
		assert_eq!(form.set_text("config.fields.0.Number.min", "-3"), Ok(()));
	}
}
