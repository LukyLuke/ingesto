require("./output.css");

import * as wasm from "../pkg";

// Everything else (menu, loading, saving, the dynamic form) is done by the WASM module
wasm.init();
