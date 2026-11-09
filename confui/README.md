# Ingesto Configuration UI

This is a WASM Package to easily read, change and create Configuration-Files for all the different **receivers** and **exporters**.

## Usage

* **Create New** opens an empty form for a receiver or exporter.
* **Load** reads a `.toml`, `.yaml` or `.yml` file. The type is detected from the content; if a file fits several types (e.g. Webhook and Network Listener) you are asked which one to open.
  Missing required values, values of the wrong type and unknown keys are listed after loading.
* Fields marked **Required** must be filled; **Optional** fields can stay empty and are then left out of the file.
* Lists have **+ Add** and a **Remove** button per entry, optional groups have **+ Add** / **Remove**.
* **Save TOML** / **Save YAML** validate the form, then check the result with the real configuration type of the receiver/exporter before downloading it.
  Nothing is sent to a server.

Note: Enums with a value are written as YAML tags (`auth: !Basic {user: u, pass: p}`), because the YAML parser used by the receivers requires them. In TOML they are tables (`auth = { Basic = { user = "u", pass = "p" } }`).

## Code layout

* `src/kinds.rs`: all receivers/exporters (schema and a typed check)
* `src/model.rs`: the form values and the operations on them (set, add/remove list entries, optional groups, variants)
* `src/codec.rs`: form to TOML/YAML and back, validation, type detection
* `src/render.rs`: HTML generation (pure functions, unit tested)
* `src/app.rs`: DOM state and event handling (4 delegated listeners)


## Dev Notes for Node

Follow [WASM-bindgen guide](https://wasm-bindgen.github.io/wasm-bindgen/)

### Upgarde all node-modules

```
$ cd webpack
$ npx npm-check-updates -u
$ npm install
```

### Install and Run

* Build the WASM Package in confui/pkg
* Start node server with webpack to serve the WASM
* Open [INGESTO ConfUI](http://localhost:8081/)

```
$ wasm-pack build
$ cd webpack
$ npm start
```
