# Polling Receiver

The **Polling-Receiver** calls an HTTP-Endpoint, including paging, and parses the received data.

The timer is in cron format with seconds:

```
sec  min  hour  day  month  weekday  year
 *   */5    *    *     *      *      *
```

With the following possible values:

* `sec` - Seconds like `10,20,30`, a range like `10-20` or an interval like `*/10` for every 10 seconds
* `min` - Minutes like `10,20,30`, a range like `10-20` or an interval like `*/10` for every 10 minutes
* `hour` - Hours like `10,20,30`, a range like `10-20`, an interval like `*/10` for every 10 hours or `@hourly` to run it once every full hour
* `day` - Day of month like `1,15,28`, a range like `10-20`, an interval like `*/2` for every second day or `@daily` to run it once a day at midnight
* `month` - Name of the month like `Jan,Mar,Dec`, number of the month like `1,3,12`, a range like `Mar-Oct` or `@monthly` to run it every first day of the month
* `weekday` - Name of the day during week like `Mon,Thurs,Sat` or a range like `Mon-Fri`
* `year` - The Year to run like `2022,2024,2030`, a range of years like `2020-2030`, an interval like `*/2` to run it every second year or `@yearly` to run it once a year

> **Note:** A definition like `* */5 * * * * *` will trigger every second and not only every five minutes.
> Therefore always use `0` on the left side of an interval like `*/5` to prevent too many triggers.


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
* For `{{ .. }}` templates see [Templated Parameters](template.md).
