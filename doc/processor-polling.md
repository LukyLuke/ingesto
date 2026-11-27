# Polling Processor

The **Polling-Processor** receives OpenTelemetry Logs, calls an HTTP-Endpoint for each message with values from the message and forwards the answer as an OpenTelemetry Log.
It is the counterpart of the [Polling-Receiver](receiver-polling.md): Where the receiver calls an API in a fixed interval, the processor calls it for each received message and uses values of this message in the request.

```
OTEL-Log ──> Listener ──> Message ──> Web-Request ──> Answer ──> Parser ──> OTEL-Log
             (listener)   (JSON)      (api)                      (parser)   (queue.otel_logger)
```

1. The `listener` receives OpenTelemetry Logs, the same way as the exporters do. The body of each log is handled as JSON.
2. For each message a new thread is started, so a slow API does not block other messages.
3. The first `api` entry whose `for_messages` regular expression matches the message is used.
4. The URI, body, headers and paging parameter are rendered as [Templated Parameters](template.md) with the message as `$response`.
5. The answer of the API is processed by the `parser` and sent to the OpenTelemetry Log-Receiver defined in `queue.otel_logger`.

A message is dropped and an error is logged if:

* no `api` entry matches the message
* the message is not valid JSON
* the request fails or the API answers with a status other than `200`


## The `$response` Parameter

In all templates `$response` is a JSON value which can be read with a JSON-Pointer like `{{ $response/user/id }}`:

* In the **first request** `$response` is the received message.
* In every following **paging request** `$response` is the JSON answer of the previous request.

> **Note:** Because `$response` is replaced by the answer in paging requests, values of the received message are only available in the first request.
> Values which are not found, or are no simple values (objects, arrays, `null`), result in an empty string.

> **Note:** A single `{` directly in front of a `{{ ... }}` template is not rendered correctly.
> Therefore JSON bodies like `{"id":"{{ $response/id }}"}` do not work yet. Use `{{ ... }}` at the start of the value or place the template after a character
> which is not a `{`, for example in the URI or in a form encoded body like `id={{ $response/id }}`.


## Endpoints

Each entry in `api` defines one endpoint:

* `name` - Name of the endpoint, shown in the logs.
* `for_messages` - Regular Expression to select messages for this endpoint. Default is `.*`. The first matching endpoint is used, so put the most specific ones first.
* `uri` - The URI to call. Can contain template parameters.
* `body` - The body in case of a `POST`. Can contain template parameters.
* `method` - `GET` (default), `POST`, `HEAD` or `OPTION`. `OPTION` is currently handled as `GET`.
* `auth` - Authentication, see below.
* `header` - A list of additional headers with a `name` and a `value`. The value can contain template parameters.
* `paging` - Paging configuration, see below.

### Authentication

The Authentication can be one of:

* `None` - (default) No Authentication
* `Basic` - Standard Basic-Auth defined by a `user` and `pass`
* `Bearer` - Uses *Bearer* Authentication token, with or without the `Bearer ` prefix
* `Header` - Use a custom Header `header` name and `value`. The value can contain template parameters.

Secrets can be read from a file with `file:/path/to/file` or from an environment variable with `env:VARIABLE`.

```toml
auth = "None"
auth = { Basic = { user = "username", pass = "env:API_PASSWORD" } }
auth = { Bearer = "file:/app/secrets/token" }
auth = { Header = { header = "Authorization", value = "ApiToken {{ $response/token }}" } }
```

### Paging

If a call to an API has multiple pages, every page has to be fetched individually.

* `param` - Defines which Parameter is added to the URI for all requests. A Parameter is a `name` and a `value` which can be a templating value
* `timeout` - How many milliseconds to wait between requests. This is to avoid rate limiting failures.
* `max_pages` - maximum number of paging calls. A guard clause to avoid endless loops and calls. With the default `0` only one request is sent.
* `until` - A condition which defines when the last page is reached.

```toml
until = "None"
until = "Empty"
until = { StatusCode = 302 }
until = { EmptyValue = "{{ $response/paging/cursor }}" }
until = { Equals = [ "{{ $response/paging/last }}", "true" ] }
```


## Example Configuration

```toml
[config]
name = "Agent Enrichment"

[config.listener]
address = "0.0.0.0"
port = 4318
path = "/v1/logs"

[[config.api]]
name = "Agent details"
for_messages = "\"agent_id\""
uri = "https://example.com/api/v2/agents/{{ $response/agent_id }}"
method = "GET"
auth = { Header = { header = "Authorization", value = "env:API_TOKEN" } }
header = [ { name = "X-Request-Id", value = "{{ $uuid }}" } ]

[[config.api]]
name = "Everything else"
uri = "https://example.com/api/v2/search?q={{ $response/message }}"

[config.api.paging]
timeout = 500
max_pages = 10
until = { EmptyValue = "{{ $response/paging/cursor }}" }
param = { name = "cursor", value = "{{ $response/paging/cursor }}" }

[config.queue]
...

[config.queue.otel_logger]
endpoint = "receiver-xy.containers.internal"
port = 4318
service = "ingesto"

[[config.parser]]
...
```

* For the `[config.queue]` part see [Queue Configuration](queue.md). The `otel_logger` is where the answers are sent to. Without it, all answers are dropped.
* For the `[config.parser]` part see [Parser Configuration](parser.md).
* For `{{ .. }}` templates see [Templated Parameters](template.md).
