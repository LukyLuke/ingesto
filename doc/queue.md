# Queue

Queues are used in different places:

* Received data is put into a queue, by receivers and exporters
* After the data is parsed before it's sent out to an exporter
* Exporters might also queue data before it's sent out

A Queue has parameters to define how many messages for how long should be in a queue max.
However, not all receivers and exporters have implemented all settings.


## Queue Configuration

The Value `max_size` is mostly for exporters to set a maximum message wize which can be processed by the receiver.
For example an Azure-DCR can process only messages with a max size of 2kb for example (just an assumption).

A Database-Exporter on the other nahd need each message by it's own, therefore there the `collect_messages` will be ignored to not group the messages into a JSON-List.

```toml
[config.queue]
max_messages = 1024
max_seconds = 60
max_size = 65535
collect_messages = true

[config.queue.otel_logger]
endpoint = "exporter-xy.containers.internal"
port = 4318
service = "ingesto"
```
