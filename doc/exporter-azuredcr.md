# Azure DCR Exporter

The **AzureDCR-Exporter** sends data out to an Azure Data Collection Rule.

*Not implemented yet*


## Example Configuration

```toml
[config]
name = "Azure DCR XY"

[[config.azuredcr]]
domain = "azure-dcr.endpoint.logs.microsoft.com"
dcr = "dcr-name"
for_messages = ".*"

[[config.azuredcr.auth.Simple]]
user = "username"
pass = "file:/secrets/azure-dcr-pass"

[[config.azuredcr.fields]]
name = "dcr-field"
origin = "message-field"

[[config.azuredcr.fields]]
name = "dcr-field"
origin = "message-field"


[config.queue]
...
```

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
