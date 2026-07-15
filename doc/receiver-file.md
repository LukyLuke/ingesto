# File Receiver

The **File-Receiver** is for reading files and processing the content.
It has two modes: Read the whole file or listen for changes.

## Example Configuration

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

Either use `follow = true` top have a `tail -f` like processor, or, if `follow = false`, define an `interval = 5.5` with the amount of seconds to read the file.

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
