# AGENTS.md

## Crate overview
This crate contains a WASM project based on `webpack` and `tailwind`.

## Layout
- `Cargo.toml`: Contains some special parts for WASM and webpack
- `webpack/`: webpack configuration, html, javascript and css files
- `src/`: wasm source code
- `tests/`: wasm tests

## Commands
Run inside the confui directory.
- Upgrade node modules: `npx npm-check-updates -u`
- Install Node modules: `npm install`
- WASM Build only: `wasm-pack build`
- Run the project: `npm start`

## Rules
- Keep the UI dynamic with WASM-Calls.
- Use WASM-Calls and avoid Javascript where possible.
- Use CSS-Classes and try to avoid inline CSS.
