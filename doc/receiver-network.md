# Network Receiver

The **Network-Receiver** opens either a UPD or a TCP Socket and listens for data.

How ever you start the listener, be aware of the ports `1-1024` which can be used only as root.

## Example Configuration

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

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
