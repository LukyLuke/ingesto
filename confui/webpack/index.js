require("./output.css");

import * as wasm from "../pkg";

wasm.greet("user");

let main_nav = document.getElementById('main-navigation');
wasm.get_config_list().forEach((conf) => {
	console.log(conf);
	let btn = document.createElement('button');
	btn.type = 'button';
	btn.className = 'px-3 py-2 rounded-md text-sm text-slate-100 hover:bg-slate-800/70 hover:text-white transition';
	btn.setAttribute('data-action', 'new_' + conf.Call);
	btn.innerText = 'New ' + conf.Title;

	btn.addEventListener("click", () => {
		wasmCall(btn.getAttribute('data-action'), {});
	})

	main_nav.appendChild(btn);
});
