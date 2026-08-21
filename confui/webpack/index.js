require("./output.css");

import * as wasm from "../pkg";

let main_nav = document.getElementById('main-navigation');
wasm.get_config_list().forEach((conf) => {
	let btn = document.createElement('button');
	btn.type = 'button';
	btn.className = 'px-3 py-2 rounded-md text-sm text-slate-100 hover:bg-slate-800/70 hover:text-white transition';
	btn.setAttribute('data-action', 'newConfig');
	btn.setAttribute('data-type', conf.call);
	btn.innerText = 'New ' + conf.title;

	btn.addEventListener("click", () => {
		wasmCall(btn.getAttribute('data-action'), { type: btn.getAttribute('data-type') });
	})

	main_nav.appendChild(btn);
});
