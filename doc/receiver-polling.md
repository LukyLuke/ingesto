# Polling Receiver

The **Polling-Receiver** calls an HTTP-Endpoint, including paging, and parses the received data.

## Authentication

The Authentication can be one of:

* `None` - (default) No Authentication
* `Basic` - Standard Basic-Auth defined by a user and password
* `Bearer` - Uses *Bearer* Authentication token
* `Header` - Use a custom Header name and value (can also be done manually with a header)

### Example Configuration

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

## Paging

If a call to an API has multiple pages, every page has to be fetched individually.

* `param` - Defines which Parameter is added to the URI for all requests. A Parameter is a `name` and a `value` which can be a templating value
* `timeout` - How many milliseconds to wait between requests. This is to avoid rate limiting failures.
* `max_pages` - maximum number of paging calls. A guard clause to avoid endless loops and calls.
* `until` - A condition which defines whe the last page is reached. None, Empty Response, Status Code, Empty Value in the response or a condition to check in the response.

### Paging-Examples:

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


## Templating

A Template-Parameter can be used in the URL, the Body and the Headers.
It is formed like `{{ ... }}`.

* `$uuid` - A simple UUID-v4.
* `$now(FORMAT)` - The current Date/Time. If `FORMAT` is not given, the value `%Y-%m-%d` is used.
* `$date(DATE#FORMAT)` - The Date/Time value given in `DATE` or the current DateTime value (now). If `FORMAT` is not given, the value `%Y-%m-%d` is used.
* `$response/JSON/POINTER` - A JSON-Pointer value which is evaluated against the last response. The format is `/key/key` for objects and in case of array indexes like `/key/0/key/33`.

### Templating-Examples:

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


## Example Configuration

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

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
