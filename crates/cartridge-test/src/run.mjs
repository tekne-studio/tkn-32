// A cartridge run through the emulator's runner on the WebAssembly of the
// engine running this script, node's V8, bun's JavaScriptCore or
// SpiderMonkey's shell, by the host the emulator ships beside it. Prints
// what the run came to as `run.rs` writes it: the same hashes over the same
// bytes, the last screen in the colours this host draws.
//   node run.mjs <emulator folder> <file> <seed> <buttons, four bytes each>
//   js -m run.mjs -- <the same>

/** SpiderMonkey's shell: the engine without a runtime's files or a browser's text codecs. */
const shell = typeof process === 'undefined';
const [folder, file, seed, held] = shell ? scriptArgs : process.argv.slice(2);
const { readFileSync } = shell ? {} : await import('node:fs');
/** @returns {Uint8Array} */
const read = (path) => (shell ? os.file.readFile(path, 'binary') : new Uint8Array(readFileSync(path)));
const say = shell ? print : console.log;
if (shell) {
	// What the host encodes and decodes is the runner's own UTF-8.
	globalThis.TextEncoder ??= class {
		encode(text) {
			return Uint8Array.from(unescape(encodeURIComponent(text)), (c) => c.charCodeAt(0));
		}
	};
	globalThis.TextDecoder ??= class {
		decode(bytes = new Uint8Array()) {
			return decodeURIComponent(escape(String.fromCharCode(...bytes)));
		}
	};
}

const manifest = JSON.parse(new TextDecoder().decode(read(`${folder}/emulator.json`)));
const host = `${folder}/${manifest.host}`;
const { hosted } = await import(shell ? host : (await import('node:url')).pathToFileURL(host).href);
const emulator = await hosted(new WebAssembly.Module(read(`${folder}/${manifest.runner}`)));

const bytes = read(held);
const words = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
const buttons = Array.from({ length: bytes.length / 4 }, (_, at) => words.getUint32(at * 4, true));

/** FNV-1a over 32 bits. */
const fnv = (hash, bytes) => {
	for (const byte of bytes) hash = Math.imul(hash ^ byte, 0x01000193) >>> 0;
	return hash;
};
const hex = (n) => n.toString(16).padStart(8, '0');

const ID = 1;
emulator.open(ID, read(file), Number(seed), 0);
emulator.script(ID, buttons);
const pixels = emulator.width * emulator.height;
let hash = 0x811c9dc5;
let tones = 0x811c9dc5;
for (let update = 0; update < buttons.length && emulator.stop(ID) === null; update++) {
	emulator.step(ID, 1);
	for (const words of emulator.tones(emulator.sound(ID))) {
		for (const word of words) tones = fnv(tones, [word & 255, (word >>> 8) & 255, (word >>> 16) & 255, word >>> 24]);
	}
	if (emulator.stop(ID) === null) {
		const read = emulator.read(ID);
		hash = fnv(fnv(hash, read.subarray(read.length - pixels)), read.subarray(0, read.length - pixels));
	}
}
const rgba = fnv(0x811c9dc5, emulator.rgba(ID));
say(`${hex(hash)} ${hex(tones)} ${emulator.stop(ID) ?? 'running'} ${emulator.updates(ID)} ${hex(rgba)}`);
