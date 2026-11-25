<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { replaceState } from '$app/navigation';
	import '$lib/ui/base.css';
	import Panel from '$lib/ui/Panel.svelte';
	import Screen from '$lib/Screen.svelte';
	import Palette from '$lib/Palette.svelte';
	import Cover from '$lib/Cover.svelte';
	import Meter from '$lib/Meter.svelte';
	import Memory from '$lib/Memory.svelte';
	import Lanes from '$lib/Lanes.svelte';
	import ToneEditor from '$lib/ToneEditor.svelte';
	import { wordsOf, type Tone } from '$lib/tone';
	import { Bench } from '$lib/bench.svelte';
	import type { Region } from '$lib/bench';
	import { Sound } from '$lib/sound.svelte';
	import { read, write, type Layout } from '$lib/session';
	import { keysOf } from '$lib/keys';

	/** A cart the page serves, as `bun run workbench` lists them. */
	interface Listed {
		name: string;
		/** How the console's commands take it: its name, or its folder when it lives elsewhere. */
		called: string;
	}

	const SPEEDS = [0.25, 0.5, 1, 2, 4];
	const ARROWS: Record<string, string> = { left: '←', right: '→', up: '↑', down: '↓' };
	/** How a button's name reads on the page: the stick's as arrows, the others in capitals. */
	const label = (name: string) => ARROWS[name] ?? name.toUpperCase();

	const sound = new Sound();
	const bench = new Bench(sound);
	// In development the bench is reachable from the console, for whoever
	// drives the page by script.
	if (import.meta.env.DEV) Object.assign(globalThis, { bench });
	let carts: Listed[] = $state([]);
	let chosen = $state('');
	/** The cart of the list last opened, for the address. */
	let opened = $state('');
	let scale = $state(4);
	let pointed: { x: number; y: number; index: number } | null = $state(null);
	let seen = $derived(bench.seen);
	/** Each pad played at the keyboard: its buttons by their bits in the number, and the keys of each. */
	let pads = $derived.by(() => {
		if (!bench.console) return [];
		const { buttons: named, padBits } = bench.console;
		return Array.from({ length: bench.players }, (_, pad) => {
			const buttons = named.map(([name, bit]) => ({ label: label(name), bit: (bit << (pad * padBits)) >>> 0 }));
			const keys = buttons.map(({ label, bit }) => `${label}: ${keysOf(bench.keys, bit)}`);
			return { buttons, keys: keys.join(' · ') };
		});
	});
	/** The running cart as `bun run cart` takes it: a file opened goes by its name. */
	let called = $derived(carts.find(({ name }) => name === bench.name)?.called ?? (bench.name || 'rocks'));
	/** How the last build went, when it failed: the compiler's words. */
	let failed: { name: string; message: string } | null = $state(null);
	/** The tone in the editor: a struck one to start, any tone clicked in the lanes after. */
	let tone: Tone = $state({ channel: 0, hz: 440, ms: 300, wave: 0, toHz: 110, loud: 255, toLoud: 0 });
	/** Why a session file could not be read. */
	let unread = $state('');

	/** What `bun run workbench` says after it rebuilt a cart (`vite.config.ts`). */
	interface Built {
		name: string;
		ok: boolean;
		message: string;
	}

	onMount(() => {
		import.meta.hot?.on('workbench:build', async (built: Built) => {
			if (built.name !== chosen) return;
			if (!built.ok) {
				failed = { name: built.name, message: built.message };
				return;
			}
			failed = null;
			const answer = await fetch(`/carts/${encodeURIComponent(built.name)}.wasm?at=${Date.now()}`);
			if (answer.ok) bench.rebuilt(await answer.arrayBuffer(), await regions(built.name));
		});
	});

	onMount(async () => {
		carts = await fetch('/carts/carts.json')
			.then((answer) => (answer.ok ? answer.json() : []))
			.catch(() => []);
		const asked = new URLSearchParams(location.search);
		// Written in the address, the people at the keyboard win over the game's
		// count, up to it.
		const players = asked.get('players');
		if (players !== null) bench.players = Number(players);
		const first = carts.find(({ name }) => name === asked.get('cart')) ?? carts[0];
		if (first) await choose(first.name);
	});
	onDestroy(() => {
		bench.close();
		sound.close();
	});

	async function choose(name: string) {
		chosen = name;
		const answer = await fetch(`/carts/${encodeURIComponent(name)}.wasm`);
		if (!answer.ok) return;
		bench.open(await answer.arrayBuffer(), name, await regions(name));
		opened = name;
	}

	// The address says the cart opened and, when a person asked, the people at
	// the keyboard, so a reload keeps both: the players as played, so a count
	// past the pads the game plays never reaches it, and again once its file
	// says them.
	$effect(() => {
		if (!opened) return;
		const players = bench.asked ? `&players=${bench.players}` : '';
		replaceState(`?cart=${encodeURIComponent(opened)}${players}`, {});
	});

	/** What lies where in a built cart's memory; nothing when its build made no map. */
	async function regions(name: string): Promise<Region[]> {
		return fetch(`/carts/${encodeURIComponent(name)}.symbols.json?at=${Date.now()}`)
			.then((answer) => (answer.ok ? answer.json() : []))
			.catch(() => []);
	}

	async function load(file: File | undefined) {
		if (!file) return;
		if (file.name.endsWith('.txt')) return replay(file);
		if (!file.name.endsWith('.wasm')) {
			unread = `${file.name}: a cartridge is a .wasm, a session a .txt`;
			return;
		}
		chosen = '';
		bench.players = null;
		bench.open(await file.arrayBuffer(), file.name);
	}

	/** The console's buttons and pads, as a session names them; it is ready before any game runs. */
	function layout(): Layout {
		if (!bench.console) throw new Error('the emulator has not started');
		return bench.console;
	}

	/** Saves the game played so far as a session file. */
	function save() {
		const session = bench.session();
		const name = `${session.cart.replace(/\.wasm$/, '')}-seed${session.seed}-${session.buttons.length}.txt`;
		const link = document.createElement('a');
		link.href = URL.createObjectURL(new Blob([write(session, name, layout())], { type: 'text/plain' }));
		link.download = name;
		link.click();
		URL.revokeObjectURL(link.href);
	}

	/** Plays a session file back from the start, on the cart it names when the page has it. */
	async function replay(file: File | undefined) {
		if (!file) return;
		unread = '';
		try {
			const session = read(await file.text(), layout());
			if (session.cart && session.cart !== chosen && carts.some(({ name }) => name === session.cart)) {
				await choose(session.cart);
			}
			if (session.seed !== undefined) bench.seed = session.seed;
			bench.restart(session.buttons);
			bench.playing = true;
		} catch (error) {
			unread = `${file.name}: ${(error as Error).message}`;
		}
	}

	/** Whether a key belongs to a field being typed in, not to the game. */
	const typing = (event: KeyboardEvent) =>
		event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement;

	function down(event: KeyboardEvent) {
		void sound.wake();
		if (typing(event) || event.metaKey || event.ctrlKey) return;
		if (bench.press(event.code)) event.preventDefault();
		else if (event.code === 'KeyM') sound.toggle();
		else if (event.code === 'KeyP') bench.playing = !bench.playing;
		else if (event.code === 'Period') bench.step();
		else if (event.code === 'KeyR') bench.restart();
	}

	function up(event: KeyboardEvent) {
		if (bench.release(event.code)) event.preventDefault();
	}

	/**
	 * A choice made in a select or a file picker gives the keys back to the
	 * game: a select kept focused would swallow them, and an arrow would
	 * choose again, another cart among them.
	 */
	function chose(event: Event) {
		const target = event.target;
		if (target instanceof HTMLSelectElement || (target instanceof HTMLInputElement && target.type === 'file')) target.blur();
	}

	$effect(() => {
		window.addEventListener('change', chose, { capture: true });
		return () => window.removeEventListener('change', chose, { capture: true });
	});
</script>

<svelte:window onkeydown={down} onkeyup={up} onpointerdown={() => sound.wake()} />
<svelte:document onvisibilitychange={() => (bench.away = document.hidden)} />
<svelte:body
	ondragover={(event) => event.preventDefault()}
	ondrop={(event) => {
		event.preventDefault();
		load(event.dataTransfer?.files[0]);
	}}
/>

<main>
	<header>
		<h1>Workbench</h1>
		<select
			value={chosen}
			onchange={(event) => {
				// Another game seats as many as it plays.
				bench.players = null;
				choose(event.currentTarget.value);
			}}
			aria-label="Cartridge"
		>
			{#if !chosen}<option value="">{bench.name || 'a file'}</option>{/if}
			{#each carts as cart (cart.name)}
				<option value={cart.name}>{cart.name}</option>
			{/each}
		</select>
		<label class="file">
			Open a file…
			<input type="file" accept=".wasm" onchange={(event) => load(event.currentTarget.files?.[0])} />
		</label>
		<label>
			seed
			<input type="number" bind:value={bench.seed} min="0" max="4294967295" size="10" />
		</label>
		{#if bench.plays > 1}
			<label title="Two share the keyboard, a half each">
				players
				<select
					value={bench.players}
					onchange={(event) => (bench.players = Number(event.currentTarget.value))}
				>
					{#each { length: bench.plays }, at (at)}<option value={at + 1}>{at + 1}</option>{/each}
				</select>
			</label>
		{/if}
		<button onclick={() => bench.restart()} title="R">Restart</button>
	</header>

	<div class="columns">
		<div class="column">
			<Panel title="Screen">
				{#snippet aside()}
					{#if pointed}x {pointed.x} y {pointed.y} · colour {pointed.index.toString(32)}{:else}160 × 120{/if}
				{/snippet}
				{#if bench.console}
					<Screen
						width={bench.console.width}
						height={bench.console.height}
						rgba={seen?.rgba}
						indices={seen?.indices}
						colours={Math.max(1, (seen?.palette.length ?? 4) / 4)}
						{scale}
						bind:pointed
					/>
				{:else}
					<p class="hint">{bench.broken ? `The emulator did not start: ${bench.broken}` : 'The emulator starts…'}</p>
				{/if}
				<div class="transport">
					<button aria-pressed={bench.playing} onclick={() => (bench.playing = !bench.playing)} title="P">
						{bench.playing ? 'Pause' : 'Play'}
					</button>
					<button onclick={() => bench.step()} title=". (period)">Step</button>
					<label>
						speed
						<select bind:value={bench.speed}>
							{#each SPEEDS as speed (speed)}<option value={speed}>×{speed}</option>{/each}
						</select>
					</label>
					<label>
						scale
						<select bind:value={scale}>
							{#each [1, 2, 3, 4, 5, 6] as n (n)}<option value={n}>×{n}</option>{/each}
						</select>
					</label>
					<button aria-pressed={sound.on && sound.awake} onclick={() => sound.toggle()} title="M">
						{!sound.awake ? 'Sound waits for a key' : sound.on ? 'Sound on' : 'Sound off'}
					</button>
					<span class="updates">update {seen?.updates ?? 0}</span>
				</div>
				{#each pads as pad, at (at)}
					<div class="buttons" aria-label="Pad {at + 1}'s buttons held">
						{#if pads.length > 1}<span class="pad">pad {at + 1}</span>{/if}
						{#each pad.buttons as { label, bit } (bit)}
							<span class:held={bench.held & bit}>{label}</span>
						{/each}
						<span class="keys">{pad.keys}</span>
					</div>
				{/each}
				<div class="buttons"><span class="keys">P pause · . step · R restart · M sound</span></div>
				{#if seen?.stop}<p class="stop">Stopped: {seen.stop}</p>{/if}
				{#if bench.console && bench.broken}<p class="stop">{bench.broken}</p>{/if}
			</Panel>
			<Panel title="Session">
				{#snippet aside()}seed {bench.started} · {bench.updates} updates · frames hash {seen?.hash ?? '-'}{/snippet}
				<div class="session">
					<button onclick={save} disabled={!bench.updates}>Save</button>
					<label class="file">
						Open…
						<input type="file" accept=".txt" onchange={(event) => replay(event.currentTarget.files?.[0])} />
					</label>
					<button onclick={() => bench.again()} disabled={!bench.updates}>Play again</button>
					{#if bench.replay}
						<span class="replaying">replaying {bench.updates} of {bench.replay.length}; a key takes over</span>
						<button onclick={() => bench.toEnd()}>To the end</button>
					{:else if bench.seeking !== null}
						<span class="replaying">running to update {bench.seeking}</span>
					{/if}
				</div>
				<p class="hint">
					Every update's buttons are kept from the start. A saved session plays again exactly here and headless:
					<code>bun run cart {called} --seed {bench.started} --frames {bench.updates} --buttons @FILE</code>
					prints the same frames hash. Saving a file in the cart's folder rebuilds it and runs the new build to this update.
				</p>
				{#if unread}<p class="stop">{unread}</p>{/if}
			</Panel>
			<Panel title="Sound">
				{#snippet aside()}the four channels over the last 600 updates{/snippet}
				<Lanes played={bench.played} now={bench.updates} ticks={bench.console?.ticks ?? 0} onpick={(picked) => (tone = picked)} />
				<ToneEditor bind:tone onplay={(played) => sound.strike(wordsOf(played))} />
				<p class="hint">
					The tones play through the chip a cabinet plays them through; a cabinet then places the sound where it
					stands.
				</p>
			</Panel>
			<Panel title="Meter">
				{#snippet aside()}units an update spent; the line is a quarter of the budget{/snippet}
				{#if bench.console}<Meter spent={bench.spent} budget={bench.console.budget} />{/if}
			</Panel>
		</div>
		<div class="column">
			{#if failed}
				<Panel title="Build">
					{#snippet aside()}{failed?.name} did not build; the page runs the last build{/snippet}
					<pre class="failed">{failed.message}</pre>
				</Panel>
			{/if}
			{#if bench.cover && bench.console}
				<Panel title="Cover">
					{#snippet aside()}what a cabinet shows before anyone plays{/snippet}
					<Cover cover={bench.cover} width={bench.console.width} height={bench.console.height} />
				</Panel>
			{/if}
			<Panel title="Palette">
				<Palette palette={seen?.palette ?? []} lit={pointed?.index ?? null} />
			</Panel>
			<Panel title="Memory">
				{#snippet aside()}marked: what the last update wrote{/snippet}
				<Memory {bench} />
			</Panel>
		</div>
	</div>
</main>

<style>
	main {
		max-width: 1400px;
		margin: 0 auto;
		padding: 16px;
	}
	header {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: var(--gap);
		margin-bottom: var(--gap);
	}
	h1 {
		margin: 0 8px 0 0;
		font-size: 15px;
		letter-spacing: 0.04em;
	}
	label {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		color: var(--dim);
	}
	.file {
		border: 1px solid var(--rule);
		border-radius: var(--radius);
		padding: 4px 8px;
		color: var(--ink);
		cursor: pointer;
	}
	.file input {
		display: none;
	}
	.columns {
		display: grid;
		grid-template-columns: minmax(0, auto) minmax(0, 1fr);
		gap: var(--gap);
		align-items: start;
	}
	.column {
		display: grid;
		gap: var(--gap);
		min-width: 0;
	}
	@media (max-width: 1000px) {
		.columns {
			grid-template-columns: minmax(0, 1fr);
		}
	}
	.transport,
	.buttons {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
		margin-top: 10px;
	}
	.updates {
		margin-left: auto;
		color: var(--dim);
	}
	.buttons span {
		color: var(--faint);
	}
	.buttons span.held {
		color: var(--accent);
	}
	.buttons .pad {
		color: var(--dim);
	}
	.buttons .keys {
		margin-left: auto;
		font-size: 11px;
	}
	.stop {
		color: var(--alert);
		margin: 10px 0 0;
	}
	.session {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
	}
	.replaying {
		color: var(--warm);
	}
	.hint {
		/* Wraps to the screen's width instead of widening its column. */
		contain: inline-size;
		color: var(--dim);
		font-size: 12px;
		margin: 10px 0 0;
	}
	code {
		color: var(--ink);
		word-break: break-all;
	}
	.failed {
		margin: 0;
		max-height: 320px;
		overflow: auto;
		color: var(--alert);
		font-size: 12px;
		white-space: pre-wrap;
	}
	button:disabled {
		opacity: 0.4;
		cursor: default;
	}
</style>
