# Parser

The Parsers define how a message is parsed and processed.
A receiver can have multiple parsers to handle mutliple message formats.


## Parser Configuration

* `parser.name` - Name of the parser, also used for referencing from a Field-Mapping
* `parser.matcher` - A Regular-Expression which is used to check if a parser is applicable for a message
* `parser.kind` - The message format: `RAW, REGEX, JSON, LEEF, CEF, CSV, STRUCTURED`
* `parser.settings` - Additional settings for some Parser-Kinds:
	* `parser.settings.Nothing` - No settings
	* `parser.settings.Regex` - Regular Rexpression to extract all fields for the final structured message. Use `(?<NAME>...)` for Named Match-Groups.
	* `parser.settings.Jpath` - Select the root path (in `JPath`-Syntax) in a Json to extract the needed fields.
* `parser.mapping` - This is the list of all Field-Value mappings for the final structured message
	* `parser.mapping.name` - Name of the field in the final message
	* `parser.mapping.source` - For named Regex-Match-Groups: Name of the group; For Json: A field name directly in the root object or a Json-Pointer (/field/name or /field/list/4/value) to a sub value.
	* `parser.mapping.index` - For Regex-Match-Groups without names: The number of the group, where `0` is the whole message.
	* `parser.mapping.parser` - Uses the value of the field and parses it with an other parser (see `parser.name`).
	* `parser.mapping.empty` - Just add an empty value on this field in the final message.
	* `parser.mapping.value` - A static string or templated value.


```toml
[[config.parser]]
name = "Parser Regex"
matcher = "^foo.*bar$"
kind = "REGEX"
settings = { Regex = "^foo(?<One>\w+)=(?<OneValue>\w+).*(Foo)=(\w+)" }

[[config.parser.mapping]]
name = "FieldName"
source = "One"

[[config.parser.mapping]]
name = "FieldValue"
source = "OneValue"

[[config.parser.mapping]]
name = "FieldFoo"
index = 3

[[config.parser.mapping]]
name = "FieldFooValue"
index = 4


[[config.parser]]
name = "Parser Json"
matcher = "^{.*}$"
kind = "JSON"
settings = { Jpath = "$.data" }

[[config.parser.mapping]]
name = "FieldName"
source = "/foo/bar/name"

[[config.parser.mapping]]
name = "FieldValue"
source = "/foo/bar/value"

[[config.parser.mapping]]
name = "FieldParser"
parser = "Parser Regex"

[[config.parser.mapping]]
name = "StaticValue"
static = "Static Text"

[[config.parser.mapping]]
name = "TemplatedValue"
static = "{{ @uuid }} - {{ $date(#%Y-%m-%d) }}"
```
