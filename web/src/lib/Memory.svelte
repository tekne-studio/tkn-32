<script lang="ts">
	import type { Bench } from './bench.svelte';
	import type { Region } from './bench';

	let { bench }: { bench: Bench } = $props();

	const COLUMNS = 64;
	let map: HTMLCanvasElement;
	let typed = $state('');

	let seen = $derived(bench.seen);
	let rows = $derived.by(() => {
		if (!seen) return [];
		const bytes = new Uint8Array(seen.window.bytes);
		const changes = new Uint8Array(seen.window.changes);
		return Array.from({ length: Math.ceil(bytes.length / 16) }, (_, row) => {
			const at = seen.window.at + row * 16;
			const cells = Array.from(bytes.subarray(row * 16, row * 16 + 16), (byte, i) => ({
				byte,
				changed: changes[row * 16 + i] === 1,
				place: place(at + i)
			}));
			// The regions that begin in this row, or the one it opens inside.
			const starting = bench.regions.filter((region) => region.at >= at && region.at < at + 16);
			const within = row === 0 && !starting.some((region) => region.at === at) ? inside(at) : undefined;
			return { at, cells, starting: within ? [within, ...starting] : starting };
		});
	});

	/** The region an address lies in. */
	function inside(at: number): Region | undefined {
		return bench.regions.findLast((region) => region.at <= at && at < region.at + region.size);
	}

	/** An address as a person reads it: hex, and where in which region. */
	function place(at: number): string {
		const region = inside(at);
		return region ? `0x${hex(at, 6)}: ${region.name} + ${at - region.at}` : `0x${hex(at, 6)}`;
	}

	/** A region's size, as a person reads it. */
	const size = (bytes: number) => (bytes < 1024 ? `${bytes} bytes` : `${(bytes / 1024).toFixed(1)} KiB`);

	const hex = (n: number, width: number) => n.toString(16).padStart(width, '0');
	/** A byte as a letter: printable ASCII as itself, the rest a dot. */
	const letter = (byte: number) => (byte >= 0x20 && byte < 0x7f ? String.fromCharCode(byte) : '.');

	$effect(() => {
		const heat = bench.heat;
		const context = map.getContext('2d')!;
		const style = getComputedStyle(map);
		const lines = Math.max(1, Math.ceil(heat.length / COLUMNS));
		map.height = lines;
		context.fillStyle = style.getPropertyValue('--rule');
		context.fillRect(0, 0, COLUMNS, lines);
		context.fillStyle = style.getPropertyValue('--warm');
		heat.forEach((warm, i) => {
			if (warm < 0.02) return;
			context.globalAlpha = 0.25 + 0.75 * warm;
			context.fillRect(i % COLUMNS, Math.floor(i / COLUMNS), 1, 1);
		});
		context.globalAlpha = 1;
		const block = seen?.block ?? 256;
		const first = Math.floor(bench.at / block);
		context.strokeStyle = style.getPropertyValue('--accent');
		context.fillStyle = style.getPropertyValue('--accent');
		for (let i = first; i < first + bench.shown / block; i++) {
			context.fillRect(i % COLUMNS, Math.floor(i / COLUMNS), 1, 1);
		}
	});

	function jump(event: MouseEvent) {
		const box = map.getBoundingClientRect();
		const x = Math.floor(((event.clientX - box.left) / box.width) * COLUMNS);
		const y = Math.floor(((event.clientY - box.top) / box.height) * map.height);
		bench.show((y * COLUMNS + x) * (seen?.block ?? 256));
	}

	function go(event: SubmitEvent) {
		event.preventDefault();
		const at = parseInt(typed.replace(/^0x/i, ''), 16);
		if (Number.isFinite(at)) bench.show(at);
	}
</script>

<div class="memory">
	<div class="bar">
		<button onclick={() => bench.show(bench.at - bench.shown)} aria-label="Back">◀</button>
		<button onclick={() => bench.show(bench.at + bench.shown)} aria-label="On">▶</button>
		<form onsubmit={go}>
			<input bind:value={typed} placeholder="address, hex" size="10" spellcheck="false" />
		</form>
		{#if bench.regions.length}
			<!-- Where the view is, and where it can go. -->
			<select
				aria-label="Go to a static"
				value={inside(bench.at)?.at ?? ''}
				onchange={(event) => bench.show(+event.currentTarget.value)}
			>
				<option value="" disabled>no static</option>
				{#each bench.regions as region (region.at)}
					<option value={region.at}>{region.name}, {size(region.size)}</option>
				{/each}
			</select>
		{/if}
		<span class="size">{((seen?.size ?? 0) / 1024).toLocaleString('en')} KiB, {((seen?.size ?? 0) / 65536) | 0} pages</span>
	</div>
	<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<canvas bind:this={map} width={COLUMNS} height="1" onclick={jump} title="Each mark is a block; warm where an update wrote"></canvas>
	<div class="hex" role="table">
		{#each rows as row (row.at)}
			{#each row.starting as region (region.at)}
				<div class="region" role="row">
					{region.at < row.at ? '…' : hex(region.at, 6)}
					<b>{region.name}</b>
					{size(region.size)}
				</div>
			{/each}
			<div class="row" role="row">
				<span class="at">{hex(row.at, 6)}</span>
				<span class="bytes">
					{#each row.cells as cell, i (i)}<span
							class:changed={cell.changed}
							class:zero={cell.byte === 0}
							title={cell.place}>{hex(cell.byte, 2)}</span
						>{/each}
				</span>
				<span class="letters">
					{#each row.cells as cell, i (i)}<span class:changed={cell.changed}>{letter(cell.byte)}</span>{/each}
				</span>
			</div>
		{/each}
	</div>
</div>

<style>
	.bar {
		display: flex;
		align-items: center;
		gap: 6px;
		flex-wrap: wrap;
		margin-bottom: 8px;
	}
	.size {
		color: var(--dim);
		margin-left: auto;
	}
	canvas {
		width: 100%;
		height: auto;
		aspect-ratio: auto;
		cursor: pointer;
		margin-bottom: 8px;
		max-height: 128px;
	}
	.hex {
		overflow-x: auto;
		font-size: 12px;
		line-height: 1.5;
	}
	.row {
		display: flex;
		gap: 14px;
		white-space: pre;
	}
	.at {
		color: var(--faint);
	}
	.region {
		color: var(--faint);
		border-top: 1px solid var(--rule);
		margin-top: 4px;
		padding-top: 2px;
	}
	.region b {
		color: var(--accent);
		font-weight: normal;
		margin: 0 1ch;
	}
	.bytes {
		display: grid;
		grid-template-columns: repeat(16, 2ch);
		column-gap: 0.6ch;
	}
	.bytes span:nth-child(8) {
		margin-right: 0.6ch;
	}
	.zero {
		color: var(--faint);
	}
	.letters {
		color: var(--dim);
	}
	.changed {
		color: var(--ground);
		background: var(--warm);
		border-radius: 2px;
	}
</style>
