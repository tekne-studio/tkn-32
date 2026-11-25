/**
 * A session is a cartridge's seed and the buttons of each update: the same
 * seed and buttons give the same frames, so a session is the game played
 * again, exactly. It is kept as the script `bun run cart --buttons @FILE`
 * reads, `UPDATE:NAMES` items, each held from that update (counted from 0)
 * to the next item's, the names joined by `+`, `-` for none, `#` a comment
 * to the line's end. A name is the first pad's; a later pad's takes its
 * number, counted from 1, before it (`2.a`). What the workbench adds rides
 * in comments. The names and the pads are the console's, as its emulator
 * gives them.
 */
import type { Console } from './bench';

/**
 * The buttons as the console names them, how many pads hold them and how far apart, and the most updates a session
 * holds, the longest script its runner plays: a file is hostile input.
 */
export type Layout = Pick<Console, 'buttons' | 'pads' | 'padBits' | 'longest'>;

export interface Session {
	cart: string;
	seed: number;
	/** The buttons held in each update, from the first. */
	buttons: number[];
	/** The frames hash after the last update, when it was kept. */
	hash?: string;
}

/** Each button of every pad by its name in a script, and its bit in the number. */
function named({ buttons, pads, padBits }: Layout): [string, number][] {
	return Array.from({ length: pads }, (_, pad) =>
		buttons.map(([name, bit]): [string, number] => [pad ? `${pad + 1}.${name}` : name, (bit << (pad * padBits)) >>> 0])
	).flat();
}

/** The session as a script, with its cart, seed, length and hash in comments. */
export function write(session: Session, file: string, layout: Layout): string {
	const every = named(layout);
	/** The names of the buttons in `held`, or `-` for none. */
	const names = (held: number) => every.filter(([, bit]) => held & bit).map(([name]) => name).join('+') || '-';
	const { cart, seed, buttons, hash } = session;
	const lines = [
		`# workbench session: ${cart}, seed ${seed}, ${buttons.length} updates`,
		`# cart ${cart}`,
		`# seed ${seed}`,
		`# updates ${buttons.length}`,
		...(hash ? [`# hash ${hash}`] : []),
		`# bun run cart ${cart} --seed ${seed} --frames ${buttons.length} --buttons @${file}`
	];
	let items: string[] = [];
	buttons.forEach((held, update) => {
		if (update === 0 || held !== buttons[update - 1]) items.push(`${update}:${names(held)}`);
		if (items.length === 8) {
			lines.push(items.join(' '));
			items = [];
		}
	});
	if (items.length) lines.push(items.join(' '));
	return lines.join('\n') + '\n';
}

/**
 * A script read back: the buttons of each update up to `updates` (the
 * comment's, or the last item's update and one), and what the comments say.
 * Throws with the item it could not read.
 */
export function read(text: string, layout: Layout): Partial<Session> & { buttons: number[] } {
	const every = new Map(named(layout));
	const said = (key: string) => new RegExp(`^#\\s*${key}\\s+(\\S+)`, 'm').exec(text)?.[1];
	const items: [number, number][] = [];
	for (const line of text.split('\n')) {
		for (const item of line.replace(/#.*/, '').split(/\s+/).filter(Boolean)) {
			const [from, held] = item.split(':');
			const update = Number(from);
			if (!Number.isInteger(update) || update < 0 || held === undefined) throw new Error(`${item}: wants UPDATE:NAMES`);
			let bits = 0;
			for (const name of held.split('+').filter((name) => name !== '-')) {
				const bit = every.get(name);
				if (bit === undefined) throw new Error(`${item}: no button ${name}`);
				bits = (bits | bit) >>> 0;
			}
			if (items.length && update < items[items.length - 1][0]) throw new Error(`${item}: the updates go in order`);
			items.push([update, bits]);
		}
	}
	const kept = Number(said('updates'));
	const updates = Number.isInteger(kept) && kept >= 0 ? kept : items.length ? items[items.length - 1][0] + 1 : 0;
	if (updates > layout.longest) throw new Error(`${updates} updates: a session is at most ${layout.longest}`);
	const buttons: number[] = [];
	let at = 0;
	for (let update = 0; update < updates; update++) {
		while (at + 1 < items.length && items[at + 1][0] <= update) at++;
		buttons.push(items.length && items[at][0] <= update ? items[at][1] : 0);
	}
	const seed = Number(said('seed'));
	return {
		cart: said('cart'),
		seed: Number.isInteger(seed) ? seed : undefined,
		hash: said('hash'),
		buttons
	};
}
