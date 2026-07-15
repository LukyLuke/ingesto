# Webhook Receiver

The **Webhook-Receiver** listens as an HTTP-Endpoint and parses the received data.


## Example Configuration

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

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
