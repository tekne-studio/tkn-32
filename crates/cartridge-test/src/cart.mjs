// A prepared cartridge held to the contract as cartridge-run's Cart holds
// it, on the engine running this file: node is V8, bun is JavaScriptCore.
// It prints what the run came to, as the tests' support writes it for
// wasmi: the FNV-1a hash of every frame shown after an update and of every
// tone kept, in env.tone's words, why it stopped, and the updates it ran.
//
//   node cart.mjs <prepared.wasm> <seed> <buttons file> '<contract as JSON>'
//
// The buttons file holds one little-endian u32 an update. The contract's
// names, its exports in its order, and its numbers come from the Rust side,
// so nothing here can drift.

import { readFileSync } from 'node:fs';

const [file, seedText, buttonsFile, contractText] = process.argv.slice(2);
const contract = JSON.parse(contractText);
const [CART_ABI, INIT, UPDATE, FRAME, PALETTE] = contract.exports;
const [SCREEN] = contract.optional;
const held = readFileSync(buttonsFile);

/** The tones kept since last taken, each in env.tone's seven words: taken, as Cart takes them, after init and after each update. */
let heard = [];
/** Keeps a tone as cartridge-run's Tones does: within the channels, the waves and the most an update keeps, its loudness held. */
function tone(channel, hz, ms, wave, toHz, loud, toLoud) {
	const words = [channel, hz, ms, wave, toHz, loud, toLoud].map((word) => word >>> 0);
	if (heard.length >= contract.maxTones || words[0] >= contract.channels || words[3] >= contract.waves) return;
	words[5] = Math.min(words[5], contract.loudest);
	words[6] = Math.min(words[6], contract.loudest);
	heard.push(words);
}
const { instance } = await WebAssembly.instantiate(readFileSync(file), {
	[contract.tone[0]]: { [contract.tone[1]]: tone }
});
const exports = instance.exports;

let stop = null;
let updates = 0;
let hash = 0x811c9dc5;
let tones = 0x811c9dc5;
const fnv = (hash, byte) => Math.imul(hash ^ byte, 0x01000193) >>> 0;

/** Why the last call trapped, as the meter tells it. */
function why() {
	const units = exports[contract.meter].value;
	if (units === contract.outOfMemory) return 'Memory';
	if (units === contract.tooDeep) return 'Stack';
	return units < 0 ? 'Budget' : 'Trap';
}

/** Calls `f`; a trap, the cartridge's or the engine's own, stops the cartridge. */
function guarded(f) {
	try {
		return f();
	} catch (error) {
		if (!(error instanceof WebAssembly.RuntimeError || error instanceof RangeError)) throw error;
		stop = why();
		return undefined;
	}
}

/** The screen and the palette the cartridge points at, or none when they lie outside its memory. */
function shown() {
	const at = guarded(() => [exports[FRAME]() >>> 0, exports[PALETTE]() >>> 0]);
	if (stop) return null;
	const memory = new Uint8Array(exports[contract.memory].buffer);
	const [frame, palette] = at;
	const colours = contract.colours * 4;
	if (frame + contract.pixels > memory.length || palette + colours > memory.length) {
		stop = 'Screen';
		return null;
	}
	return [memory.subarray(frame, frame + contract.pixels), memory.subarray(palette, palette + colours)];
}

guarded(() => exports[contract.start]?.());
if (!stop) {
	const abi = guarded(() => exports[CART_ABI]() >>> 0);
	if (!stop && abi !== contract.abi) stop = `Abi(${abi})`;
	else if (!stop && exports[SCREEN]) {
		const step = guarded(() => exports[SCREEN]() >>> 0);
		if (!stop && step >= contract.screens) stop = `Size(${step})`;
	}
}
if (!stop) guarded(() => exports[INIT](Number(seedText) | 0));
heard = [];
if (!stop) shown();

for (let tick = 0; tick < held.length / 4 && !stop; tick++) {
	exports[contract.meter].value = contract.updateBudget;
	guarded(() => exports[UPDATE](held.readUInt32LE(tick * 4) & contract.buttons));
	if (stop) break;
	for (const words of heard) {
		for (const word of words) {
			for (let byte = 0; byte < 4; byte++) tones = fnv(tones, (word >>> (8 * byte)) & 0xff);
		}
	}
	heard = [];
	updates += 1;
	const screen = shown();
	if (!screen) break;
	for (const bytes of screen) {
		for (let i = 0; i < bytes.length; i++) hash = fnv(hash, bytes[i]);
	}
}

const hex = (hash) => hash.toString(16).padStart(8, '0');
console.log(`${hex(hash)} ${hex(tones)} ${stop ?? 'running'} ${updates}`);
