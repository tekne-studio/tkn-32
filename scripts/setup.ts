// The tools the console's commands run beside cargo and bun, at the
// versions its locks name, once after a clone, and again when a pin moves:
// - wasm-bindgen-cli, whose test runner runs the chip's goldens on wasm32
//   under node, at the `wasm-bindgen` crate's version in Cargo.lock;
// - SpiderMonkey's shell, Firefox's engine without the browser, the third
//   engine the suites run the emulator's host on ([`SPIDERMONKEY`]).
// The Rust target comes from rust-toolchain.toml. Each tool by its name,
// every one when none is named: a job installs only what it runs.
//   bun run setup [wasm-bindgen] [spidermonkey]
import { $ } from 'bun';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { ROOT } from './lib';

const TOOLS = ['wasm-bindgen', 'spidermonkey'];
const asked = process.argv.slice(2);
const unknown = asked.filter((tool) => !TOOLS.includes(tool));
if (unknown.length) throw new Error(`no tool named ${unknown.join(', ')}: the tools are ${TOOLS.join(', ')}`);
const wanted = (tool: string) => asked.length === 0 || asked.includes(tool);

async function wasmBindgen() {
	const lock = await Bun.file(`${ROOT}/Cargo.lock`).text();
	const version = lock.match(/^name = "wasm-bindgen"\nversion = "([^"]+)"$/m)?.[1];
	if (!version) throw new Error('no wasm-bindgen in Cargo.lock');
	const have = await $`wasm-bindgen --version`.nothrow().quiet().text();
	if (have.includes(version)) console.log(`wasm-bindgen ${version} is here`);
	else await $`cargo install wasm-bindgen-cli --locked --version ${version}`;
}

/**
 * SpiderMonkey's shell, pinned to a Firefox release: each platform's
 * archive on Mozilla's archive, by the sha256 its release's `SHA256SUMS`
 * names. The archive is checked before it is unpacked, into
 * `.tools/spidermonkey` (ignored), where the suites find `js`.
 */
const SPIDERMONKEY = {
	release: '160.0.1',
	archives: {
		'darwin-arm64': ['jsshell-mac.zip', 'bf44292236c42319df4472348c01b785b57dfaca590a3bf400455958707bf29b'],
		'darwin-x64': ['jsshell-mac.zip', 'bf44292236c42319df4472348c01b785b57dfaca590a3bf400455958707bf29b'],
		'linux-x64': ['jsshell-linux-x86_64.zip', 'b547d9ed0bc564ba4387a32cc09aa7aa490c4bdc32998a807c38d36a8078ad69'],
		'linux-arm64': ['jsshell-linux-aarch64.zip', '1c2bd0e53de20a391f0ae9f9163f79479c441fe6c464a45ebe3244c7080a4ddb']
	} as Record<string, [string, string]>
};

async function spiderMonkey() {
	const shell = `${ROOT}/.tools/spidermonkey`;
	const pinned = `${SPIDERMONKEY.release}\n`;
	const platform = `${process.platform}-${process.arch}`;
	const archive = SPIDERMONKEY.archives[platform];
	if (existsSync(`${shell}/release`) && readFileSync(`${shell}/release`, 'utf8') === pinned) {
		console.log(`SpiderMonkey ${SPIDERMONKEY.release} is here`);
	} else if (!archive) {
		console.warn(`SpiderMonkey: Mozilla builds no shell for ${platform}; the suites run without it`);
	} else {
		const [file, sha] = archive;
		const url = `https://archive.mozilla.org/pub/firefox/releases/${SPIDERMONKEY.release}/jsshell/${file}`;
		const response = await fetch(url);
		if (!response.ok) throw new Error(`${url}: ${response.status}`);
		const bytes = new Uint8Array(await response.arrayBuffer());
		const found = new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
		if (found !== sha) throw new Error(`${url}: hashes to ${found}, not to the pinned ${sha}`);
		// Unpacked from a folder of its own, then put in place whole.
		const fresh = mkdtempSync(`${tmpdir()}/spidermonkey-`);
		writeFileSync(`${fresh}/${file}`, bytes);
		await $`unzip -q ${`${fresh}/${file}`} -d ${`${fresh}/shell`}`;
		rmSync(shell, { recursive: true, force: true });
		mkdirSync(`${ROOT}/.tools`, { recursive: true });
		await $`mv ${`${fresh}/shell`} ${shell}`;
		rmSync(fresh, { recursive: true, force: true });
		writeFileSync(`${shell}/release`, pinned);
		console.log(`SpiderMonkey ${SPIDERMONKEY.release} -> ${shell}`);
	}
}

if (wanted('wasm-bindgen')) await wasmBindgen();
if (wanted('spidermonkey')) await spiderMonkey();
