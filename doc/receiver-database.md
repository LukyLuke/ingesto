# Database Receiver

The **Database-Receiver** runs individual queries on a database and processes each returned record.

*This is not implemented yet.*

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

[[config.database.queries]]
name = "db_table"
sql = ""

[config.queue]
...

[[config.parser]]
...
```

* For the `[config.queue]` part see [Queue Configuration](queue.md).
* For the `[config.parser]` part see [Parser Configuration](parser.md).
