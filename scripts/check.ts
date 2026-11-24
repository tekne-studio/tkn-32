// What the console holds itself to, in CI and before a change lands, from
// any folder: `unsafe` in its ABI files alone (`unsafe.ts`), the emulator
// built as a host takes it, its crates formatted, linted and tested, the
// hostile and contract suites through the emulator's hosts among them, the
// emulator linted for the browser, the chip's goldens on wasm32, every cart
// built as the page runs it, the worklet against the goldens' sessions, the
// host's own calls past what a page asks, the page read against the emulator's host, and the scripts' and the page's
// own tests. The page is read from a copy in target/: checking it writes
// its generated files (`svelte-kit sync`), and a workbench running from
// this folder would reload on them.
import { cpSync, rmSync, symlinkSync } from 'node:fs';
import { $ } from 'bun';
import { ROOT } from './lib';
import { build, examples } from './carts';
import { EMULATOR, buildEmulator } from './emulator';
import { unsafeProblems } from './unsafe';
import { WEB } from './workbench';

/** Where the page is copied to be read, beside the host it reads, as in this folder. */
const READ = `${ROOT}/target/check`;
const HOST = 'crates/runner/host.js';

$.cwd(ROOT);
const unsafe = unsafeProblems(ROOT);
if (unsafe.length) throw new Error(unsafe.join('\n'));
await $`cargo fmt --all --check`;
// The suites run the emulator through its hosts, so it is built first.
await buildEmulator(EMULATOR);
await $`cargo clippy --workspace --all-targets -- -D warnings`;
await $`cargo test --workspace`;
await $`cargo clippy --target wasm32-unknown-unknown -p runner -p chip-worklet -- -D warnings`;
// The chip's samples are the same bits on wasm32, under node.
await $`cargo test -p chip --target wasm32-unknown-unknown --test golden`;
await build(await examples());
rmSync(READ, { recursive: true, force: true });
for (const file of ['package.json', 'tsconfig.json', 'vite.config.ts', 'src', 'static']) {
	cpSync(`${WEB}/${file}`, `${READ}/web/${file}`, { recursive: true, filter: (from) => !from.startsWith(`${WEB}/src/lib/pkg`) });
}
cpSync(`${ROOT}/${HOST}`, `${READ}/${HOST}`);
symlinkSync(`${WEB}/node_modules`, `${READ}/web/node_modules`);
// The worklet plays the goldens as a page feeds it, on the chip the emulator
// built; the host answers what no page asks, on the runner it built.
await $`bun test ./crates/chip-worklet ./crates/runner`;
await $`bun run check`.cwd(`${READ}/web`);
await $`bun test ./web/src ./scripts`;
console.log('the console passes');
