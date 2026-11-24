/**
 * The page's sound: the emulator's chip in its AudioWorklet
 * (`crates/chip-worklet`), fed the batches of tones the emulator gives. A
 * browser lets a page sound only after the person does something on it, so
 * the chip wakes on the first key or click; until then the page is silent.
 */

// Globs resolve to nothing when the emulator was never built: the chip then
// never wakes, and the bench says why.
const manifests = import.meta.glob<{ chip: string; processor: string }>('./pkg/emulator.json', {
	eager: true,
	import: 'default'
});
const files = import.meta.glob<string>('./pkg/*.{wasm,js}', { eager: true, query: '?url', import: 'default' });

export class Sound {
	/** Whether the person wants to hear it. */
	on = $state(true);
	/** Whether the chip runs: after the first key or click. */
	awake = $state(false);

	#context: AudioContext | undefined;
	#node: AudioWorkletNode | undefined;
	#waking: Promise<void> | undefined;

	/** Starts the chip, once; called from a key or a click. */
	wake(): Promise<void> {
		this.#waking ??= this.#start().catch((error) => {
			console.error('the chip did not start', error);
			this.#waking = undefined;
		});
		return this.#waking;
	}

	async #start() {
		const manifest = manifests['./pkg/emulator.json'];
		if (!manifest) throw new Error('the emulator was never built');
		const context = new AudioContext({ latencyHint: 'interactive' });
		this.#context = context;
		const [module] = await Promise.all([
			fetch(files[`./pkg/${manifest.chip}`]).then((answer) => answer.arrayBuffer()),
			context.audioWorklet.addModule(files[`./pkg/${manifest.processor}`])
		]);
		const node = new AudioWorkletNode(context, 'chip', {
			numberOfInputs: 0,
			outputChannelCount: [2],
			processorOptions: { module }
		});
		node.connect(context.destination);
		this.#node = node;
		if (context.state === 'suspended') await context.resume();
		this.awake = true;
	}

	/**
	 * A batch of updates' tones, as the emulator gave it, transferred. One
	 * whose updates played nothing goes too: it keeps the chip's cursor one
	 * step an update, so a tone after a silence plays as far from the last
	 * as the game played it.
	 */
	play(batch: ArrayBuffer) {
		if (!this.on || !this.#node) return;
		this.#node.port.postMessage(new Uint32Array(batch), [batch]);
	}

	/** Plays one tone now, its words, waking the chip if it sleeps: the editor's. */
	async strike(words: number[]) {
		await this.wake();
		if (this.on) this.#node?.port.postMessage({ tone: words });
	}

	/** Holds the stream where it is, eased out, what waits kept. */
	hold() {
		this.#node?.port.postMessage({ hold: true });
	}

	/** Eases the stream in from where it held. */
	resume() {
		this.#node?.port.postMessage({ resume: true });
	}

	/** Drops what waits and lets every channel go out. */
	hush() {
		this.#node?.port.postMessage({ hush: true });
	}

	toggle() {
		this.on = !this.on;
		if (!this.on) this.hush();
	}

	close() {
		void this.#context?.close();
	}
}
