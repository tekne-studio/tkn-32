/**
 * The page's sound: the arcade's chip (`crates/chip`) in an AudioWorklet
 * (`crates/chip-worklet/worklet.js`), fed the tones each update played. A browser
 * lets a page sound only after the person does something on it, so the
 * chip wakes on the first key or click; until then the page is silent.
 */
import chip from './pkg/chip_worklet.wasm?url';
import worklet from './pkg/chip-worklet.js?url';
import { batch } from './tone';

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
		const context = new AudioContext({ latencyHint: 'interactive' });
		this.#context = context;
		const [module] = await Promise.all([
			fetch(chip).then((answer) => answer.arrayBuffer()),
			context.audioWorklet.addModule(worklet)
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
	 * The tones of updates run, one list of words an update, `perSecond`
	 * updates a second. A batch that played nothing goes too: it keeps the
	 * chip's cursor one step an update, so a tone after a silence plays as
	 * far from the last as the game played it.
	 */
	play(updates: number[][], perSecond: number) {
		if (!this.on || !this.#node || !updates.length) return;
		this.#send(batch(updates, perSecond));
	}

	/** Plays one tone now, waking the chip if it sleeps: the editor's. */
	async strike(words: number[]) {
		await this.wake();
		if (this.on) this.#send(batch([words], 0));
	}

	#send(played: Uint32Array) {
		this.#node?.port.postMessage(played, [played.buffer]);
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
