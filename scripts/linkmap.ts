// A cart's statics, read from the map its link writes (`cartridge_build::link`):
// each named static's address in memory, its size and its name, for the
// workbench to name the memory by. The map is the build's own, never the
// file's, so a cart opened from a file has none.
import type { Region } from '../web/src/lib/bench';

/** A line of a static: its address, its offset in the file, its size, its symbol. */
const STATIC = /^\s*([0-9a-f]+)\s+[0-9a-f]+\s+([0-9a-f]+)\s+(_R\S+|_ZN\S+)\s*$/;
/** A line of the data's first section, which starts where the stack ends. */
const SECTION = /^\s*([0-9a-f]+)\s+[0-9a-f]+\s+[0-9a-f]+\s+\.(rodata|data|bss)\s*$/;

/** The regions a map names, by address: the stack first, then every named static. */
export function regions(map: string): Region[] {
	const found: Region[] = [];
	let data = Infinity;
	for (const line of map.split('\n')) {
		const section = SECTION.exec(line);
		if (section) data = Math.min(data, parseInt(section[1], 16));
		const fixed = STATIC.exec(line);
		if (!fixed) continue;
		const size = parseInt(fixed[2], 16);
		if (size > 0) found.push({ at: parseInt(fixed[1], 16), size, name: demangle(fixed[3]) });
	}
	if (Number.isFinite(data) && data > 0) found.push({ at: 0, size: data, name: 'stack' });
	return found.sort((a, b) => a.at - b.at);
}

/**
 * A Rust symbol's path, `rocks::play::SHIP`, from its v0 (`_R`) or legacy
 * (`_ZN`) mangling; the symbol as it is when its path is more than modules
 * and items (an impl's, a generic's).
 */
export function demangle(symbol: string): string {
	const bare = symbol.replace(/\.llvm\.\d+$/, '');
	if (bare.startsWith('_ZN')) {
		const parts: string[] = [];
		let at = 3;
		while (at < bare.length && bare[at] !== 'E') {
			const digits = /^\d+/.exec(bare.slice(at))?.[0];
			if (!digits) return symbol;
			at += digits.length;
			parts.push(bare.slice(at, at + +digits));
			at += +digits;
		}
		if (/^h[0-9a-f]{16}$/.test(parts.at(-1) ?? '')) parts.pop();
		return parts.join('::');
	}
	let at = 2;
	/** A disambiguator, `s<base62>_`, which a path carries and a reader skips. */
	const skip = () => {
		if (bare[at] !== 's') return;
		const end = bare.indexOf('_', at);
		if (end < 0) throw new Error('unended');
		at = end + 1;
	};
	const ident = (): string => {
		const digits = /^\d+/.exec(bare.slice(at))?.[0];
		if (!digits) throw new Error('no identifier');
		at += digits.length;
		if (bare[at] === '_') at++;
		const name = bare.slice(at, at + +digits);
		at += +digits;
		return name;
	};
	const path = (): string => {
		const tag = bare[at++];
		if (tag === 'C') {
			skip();
			return ident();
		}
		if (tag === 'N') {
			at++; // the namespace: a type's or a value's
			const parent = path();
			skip();
			return `${parent}::${ident()}`;
		}
		throw new Error(`a path of kind ${tag}`);
	};
	try {
		return path();
	} catch {
		return symbol;
	}
}
