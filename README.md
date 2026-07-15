![ingesto](doc/logo.png)

# Ingesto

Ingesto is a simple but fully and heavily configurable data pipeline kind of log routing.

1. A **receiver** is started to eiter listen or fetch data
2. A **parser** is converting the data into a structured format
3. An **exporter** is sending the structured data to any kind of storage

## Receivers

A receiver is polling or listening for data and converts it to a structured format.

The structured data is afterwards sent to an exporter or an OpenTelemetry Log Receiver.

* The [**Database-Receiver**](/doc/receiver-database.md) reads data from a database.
* The [**File-Receiver**](/doc/receiver-file.md) either reads a whole file or listens for changes.
* The [**Network-Receiver**](/doc/receiver-network.md) opens either a UPD or a TCP Socket and listens for data.
* The [**Polling-Receiver**](/doc/receiver-polling.md) calls an HTTP-Endpoint, including paging and templating parameters for more dynamic requests.
* The [**Webhook-Receiver**](/doc/receiver-webhook.md) listens as an HTTP-Endpoint and parses the received data.

## Exporters

Exporters receive OpenTelemetry Logs and try to process the contained data to store it in a specific format.

The log format must be structured JSON so different fields and values can be set in a expected storage like a database, key-value store or any other log storage format.

* The [**Database-Exporter**](doc/exporter-database.md) saves data in a PostgreSQL, Maria/MySQL or SqLite Database.
* The [**AzureDCR-Exporter**](doc/exporter-azuredcr.md) sends data to an Azure DCR (Data Collection Rule).

