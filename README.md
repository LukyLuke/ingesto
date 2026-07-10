![ingesto](doc/logo.png)

# Ingesto

Ingesto is a simple but fully and heavily configurable data pipeline kind of log routing.

1. A **receiver** is started to eiter listen or fetch data
2. A **parser** is converting the data into a structured format
3. An **exporter** is sending the structured data to any kind of storage

## Receivers

A receiver is polling or listening for data and converts it to a structured format.

Every receiver has a `[config.queue]` section and a `[[config.parser]]` configuration section.
See at below for these general parts.


### Database

*Not yet*

### File

The **File-Receiver** has two modes.
Either it reads a file completely and parses each line.
Or it opens a file and listens for new lines which are parsed as soon as received.

#### Example Configuration

```toml
[config]
name = "Log-Reader XY"

[config.file]
path = "/var/log/log_file.log"
follow = true
interval = 5.5

[config.queue]
...

[[config.parser]]
...
```

Eiter use `follow = true` top have a `tail -f` like processor, or, if `follow = false`, define an `interval = 5.5` with the amount of seconds to read the file.



### Network

The **Network-Receiver** opens either an UPD or TCP Socket and listens for data.
The received data is the parsed.

#### Example Configuration

```toml
[config]
name = "Syslog-Reader XY"

[config.listen]
address = "0.0.0.0"
port = 1514
kind = "UDP"

[config.queue]
...

[[config.parser]]
...
```

Use `kind = "UDP` or `kind = "TCP"`.
How ever you start the listener, be aware of the ports `1-1024` which can be used only as root.


### Polling

The **Polling-Receiver** calls an HTTP-Endpoint, including paging, and parses the received data.

#### Authentication

The Authentication can be one of:

* `None` - (default) No Authentication
* `Basic` - Standard Basic-Auth defined by a user and password
* `Bearer` - Uses *Bearer* Authentication token
* `Header` - Use a custom Header name and value (can also be done manually with a header)

##### Example Configuration

```toml
[config.api.auth.None]

[config.api.auth.Basic]
user = "username"
pass = "password"

[config.api.auth.Bearer]
value = "Bearer Token with/out Bearer"

[config.api.auth.Header]
name = "Auth-Header-Name"
value = "Auth Header Value"
```

#### Paging

If a call to an API has multiple pages, every page has to be fetched individually.

* `param` - Defines which Parameter is added to the URI for all requests. A Parameter is a `name` and a `value` which can be a templating value
* `timeout` - How many milliseconds to wait between requests. This is to avoid rate limiting failures.
* `max_pages` - maximum number of paging calls. A guard clause to avoid endless loops and calls.
* `until` - A condition which defines whe the last page is reached. None, Empty Response, Status Code, Empty Value in the response or a condition to check in the response.

##### Paging-Examples:

```toml
[config.api.paging]
param = { name = "page", value = "{{ $response/paging/cursor }}" }
timeout = 500
max_pages = 10
until = { Empty }
```

The `until` parameter can be:
```toml
until = { None }
until = { Empty }
until = { StatusCode = 302 }
until = { EmptyValue = "{{ $response/paging/cursor }}" }
until = { Equals = [ "{{ $response/paging/last }}", "true" ] }
```


#### Templating

A Template-Parameter can be used in the URL, the Body and the Headers.
It is formed like `{{ ... }}`.

* `$uuid` - A simple UUID-v4.
* `$now(FORMAT)` - The current Date/Time. If `FORMAT` is not given, the value `%Y-%m-%d` is used.
* `$date(DATE#FORMAT)` - The Date/Time value given in `DATE` or the current DateTime value (now). If `FORMAT` is not given, the value `%Y-%m-%d` is used.
* `$response/JSON/POINTER` - A JSON-Pointer value which is evaluated against the last response. The format is `/key/key` for objects and in case of array indexes like `/key/0/key/33`.

##### Templating-Examples:

**UUID**:

```
given:  http://localhost/{{ $uuid }}/test
result: http://localhost/3d542a87-69ff-44d2-ae62-bffd5e32a7ca/test
```

**Date and Time**:

With a relative date string:
```
given:  http://localhost/get?from={{ $date(-1day#%Y-%m-%d) }}&upto={{ $now(%Y-%m-%d) }}
result: http://localhost/get?from=2026-01-02&upto=2026-02-03
```

Can also be combined with values from the response:
```
given:  http://localhost/get?from={{ $date($response/entry/startup#%Y-%m-%d) }}&upto={{ $now(%Y-%m-%d) }}
result: http://localhost/get?from=2025-11-12&upto=2026-02-03
```

**Response Value**:

```
given:  http://localhost/get?client={{ $response/agent/client/id }}
result: http://localhost/get?client=666
```


#### Example Configuration

```toml
[config]
name = "API-Polling XY"
timer = "* */5 * * * *"

[config.api]
uri = "https://exaple.com/api/v2/users"
method = "POST"
body = """
{ "some": "multiline",
  "string": "body" }
"""

[config.api.auth.Header]
name = "X-Auth-Header"
value = "Auth-Header-Value"

[config.api.paging]
timeout = 1000
max_pages = 20
until = { Equals = [ "{{ $response/paging/page }}", "{{ $response/paging/last }}" ] }

[config.api.paging.param]
name = "page"
value = "{{ $response/paging/cursor }}"

[config.queue]
...

[[config.parser]]
...
```



### Webhook

The **Webhook-Receiver** listens as an HTTP-Endpoint and parses the received data.

#### Example Configuration

```toml
[config]
name = "Webhook XY"

[config.listen]
address = "0.0.0.0"
port = 8080

[[config.routes]]
path = "/demo/get"
kind = "GET"

[[config.routes]]
path = "/demo/post"
kind = "POST"

[config.queue]
...

[[config.parser]]
...
```

### Default Configuration Parts

#### Queue Configuration

A Queue has parameters to define how many messages for how long should be in a queue max.
Not all receivers and exporters have implemented all settings.

The Value `max_size` is mostly for exporters to set a maximum message wize which can be processed by the receiver.
For example an Azure-DCR can process only messages with a max size of 2kb for example (just an assumption).

A Database-Exporter on the other nahd need each message by it's own, therefore there the `collect_messages` will be ignored to not group the messages into a JSON-List.

```toml
[config.queue]
max_messages = 1024
max_seconds = 60
max_size = 65535
collect_messages = true
```

#### Parser Configuration

The Parsers define how a message is parsed and processed.
A  receiver can have multiple parsers to handle mutliple message formats.

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

```

