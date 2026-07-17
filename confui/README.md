# Ingesto Configuration UI

This is a WASM Package to easily read, change and create Configuration-Files for all the different **receivers** and **exporters**.


## Dev Notes for Node

Follow [WASM-bindgen guide](https://wasm-bindgen.github.io/wasm-bindgen/)

### Upgarde all node-modules

```
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
