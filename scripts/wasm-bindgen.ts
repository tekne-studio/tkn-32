// wasm-bindgen-cli at the version of the `wasm-bindgen` crate in Cargo.lock:
// only that version binds a module built on the crate and runs the wasm32
// tests, under the test runner .cargo/config.toml names. `bun run setup`
// runs it, and so does a job that needs this tool alone.
//   bun scripts/wasm-bindgen.ts
import { $ } from 'bun';
import { ROOT } from './lib';

const lock = await Bun.file(`${ROOT}/Cargo.lock`).text();
const version = lock.match(/^name = "wasm-bindgen"\nversion = "([^"]+)"$/m)?.[1];
if (!version) throw new Error('no wasm-bindgen in Cargo.lock');
const have = await $`wasm-bindgen --version`.nothrow().quiet().text();
if (have.includes(version)) console.log(`wasm-bindgen ${version} is here`);
else await $`cargo install wasm-bindgen-cli --locked --version ${version}`;
