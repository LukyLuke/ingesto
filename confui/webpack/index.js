require("./output.css");

import * as wasm from "../pkg";

// Call WASM to initialize a new Configuration-Form based on the payload.type
function wasmCall(action, payload) {
	try {
		if (action == 'newConfig') {
			let conf = wasm.get_config_schema(payload.type);
			window.document.querySelectorAll("button.add-array-item").forEach((btn) => {
				btn.addEventListener("click", (evt) => {
					wasm.add_button_handler(evt.target, conf);
				});
			});

		} else {
			console.debug("Called:", "wasmCall", action, payload);
			console.error(action, "not yet implemented");
		}
	} catch (e) {
		console.error(e);
	}
}

let sub_nav = document.getElementById('sub-navigation');
wasm.get_config_list().forEach((conf) => {
	let btn = document.createElement('button');
	btn.type = 'button';
	btn.className = 'block w-full rounded px-3 py-2 text-left text-sm hover:bg-slate-700';
	btn.setAttribute('data-action', 'newConfig');
	btn.setAttribute('data-type', conf.call);
	btn.innerText = 'New ' + conf.title;

	btn.addEventListener("click", () => {
		wasmCall(btn.getAttribute('data-action'), { type: btn.getAttribute('data-type') });
	})

	sub_nav.appendChild(btn);
});

wasm.get_config_schema("init", { type: ""});
