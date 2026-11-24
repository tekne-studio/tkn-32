// A cartridge run with no window by the console's runner (`crates/headless`):
// a strip of PNGs, and with --wav its tones through the chip, from a fixed
// seed and a script of buttons, or a session the workbench saved. The first
// argument is a .wasm, a cart under examples/ by its name or a cart
// elsewhere by its folder, built first in its workspace. The rest pass
// through, and a path among them is read from where the command runs:
//   bun run cart rocks --buttons '30:start 34:- 60:a+right' --frames 600 --out out/rocks
//   bun run cart rocks --buttons @session.txt --frames 351 --wav out/rocks.wav
//   bun run cart ../mygame --frames 120 --every 40
//   bun run cart path/to/game.wasm --frames 300 --every 30
import { basename, relative } from 'node:path';
import { $ } from 'bun';
import { build, built, cartNamed } from './carts';
import { ROOT } from './lib';

const [cart, ...args] = process.argv.slice(2);
if (!cart) throw new Error('bun run cart <name under examples/ | folder | file.wasm> [--frames N --out DIR] ...');
let file = cart;
if (!cart.endsWith('.wasm')) {
	const found = await cartNamed(cart);
	await build([found], true).catch(() => process.exit(1));
	file = built(found);
}
// A cartridge is named by the hash of its content, as every heavy file is.
const bytes = await Bun.file(file).bytes();
const hash = new Bun.CryptoHasher('sha256').update(bytes).digest('hex');
console.log(`${relative(process.cwd(), file)}: ${bytes.length} bytes, named ${hash}.wasm`);
if (!args.includes('--frames')) args.push('--frames', '600');
if (!args.includes('--out')) args.push('--out', `out/${basename(cart, '.wasm')}`);
const run = await $`cargo run --release -q --manifest-path ${ROOT}/Cargo.toml -p headless -- ${file} ${args}`.nothrow();
process.exit(run.exitCode);
