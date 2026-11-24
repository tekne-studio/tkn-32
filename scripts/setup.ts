// The tools the console's commands run beside cargo and bun, at the
// versions its locks name, once after a clone: wasm-bindgen-cli, which binds
// the emulator and the workbench's engine, at the `wasm-bindgen` crate's
// version in Cargo.lock. The Rust target comes from rust-toolchain.toml.
//   bun run setup
import { $ } from 'bun';
import { ROOT } from './lib';

const lock = await Bun.file(`${ROOT}/Cargo.lock`).text();
const version = lock.match(/^name = "wasm-bindgen"\nversion = "([^"]+)"$/m)?.[1];
if (!version) throw new Error('no wasm-bindgen in Cargo.lock');
const have = await $`wasm-bindgen --version`.nothrow().quiet().text();
if (have.includes(version)) console.log(`wasm-bindgen ${version} is here`);
else await $`cargo install wasm-bindgen-cli --locked --version ${version}`;
