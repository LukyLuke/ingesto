# Database Exporter

The **Database-Exporter** tries to process each received message and stores it in a database.

## Example Configuration

```toml
[config]
name = "Database XY"

[config.database]
database = "db_name"
kind = "PostgreSQL"

[config.database.connection]
host = "0.0.0.0"
port = 5432
ssl_mode = "Disabled"
root_cert = "/path/to/root/cert.crt"
ssl_cert = "/path/to/ssl/cert.crt"
ssl_key = "/path/to/ssl/cert.key"

[config.database.auth.Simple]
user = "db_user"
pass = "file:/secrets/db-pass.file"

[[config.database.tables]]
name = "db_table"
for_messages = ".*"

[[config.database.tables.fields]]
name = "db_field"
origin = "message_field"

[[config.database.tables.fields]]
name = "db_field"
origin = "message_field"


[config.queue]
...
```

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
