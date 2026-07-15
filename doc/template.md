# Templated Parameters

A Template-Parameter can be used in the URL, the Body and the Headers.
It is formed like `{{ ... }}`.

* `$uuid` - A simple UUID-v4.
* `$now(FORMAT)` - The current Date/Time. If `FORMAT` is not given, the value `%Y-%m-%d` is used.
* `$date(DATE#FORMAT)` - The Date/Time value given in `DATE` or the current DateTime value (now). If `FORMAT` is not given, the value `%Y-%m-%d` is used.
* `$response/JSON/POINTER` - A JSON-Pointer value which is evaluated against the last response. The format is `/key/key` for objects and in case of array indexes like `/key/0/key/33`.

## Templating-Examples:

**UUID**:

```
given:  http://localhost/{{ $uuid }}/test
result: http://localhost/3d542a87-69ff-44d2-ae62-bffd5e32a7ca/test
```

**Date and Time**:

Date-Time values are always in UTC!

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

Or have a special formatter like "iso8601" as a shortcut:
```
given:  {{ $now(#iso8601) }}
result: 2026-02-03T12:13:14.123456+00:00
```


**Response Value**:

```
given:  http://localhost/get?client={{ $response/agent/client/id }}
result: http://localhost/get?client=666
```
