// What the console holds itself to, in CI and before a change lands, from
// any folder: `unsafe` in its ABI files alone (`unsafe.ts`), its crates
// formatted, linted and tested, the hostile and contract suites again on the
// browser's own WebAssembly, the workbench's engine linted for the browser,
// the chip's goldens on wasm32, every cart built as the page runs it, the
// worklet against the goldens' sessions, the page read against the engine's
// types, and the scripts' and the page's own tests. The page is read from a
// copy in target/: checking it writes its generated files (`svelte-kit
// sync`), and a workbench running from this folder would reload on them.
import { cpSync, rmSync, symlinkSync } from 'node:fs';
import { $ } from 'bun';
import { ROOT } from './lib';
import { build, examples } from './carts';
import { unsafeProblems } from './unsafe';
import { WEB, buildEngine } from './workbench';

/** Where the page is copied to be read. */
const READ = `${ROOT}/target/workbench-check`;

$.cwd(ROOT);
const unsafe = unsafeProblems(ROOT);
if (unsafe.length) throw new Error(unsafe.join('\n'));
await $`cargo fmt --all --check`;
await $`cargo clippy --workspace --all-targets -- -D warnings`;
await $`cargo test --workspace`;
// The hostile and contract suites again, on node's V8.
await $`cargo test --target wasm32-unknown-unknown -p cartridge-web --test hostile --test contract`;
await $`cargo clippy --target wasm32-unknown-unknown -p workbench -p chip-worklet -- -D warnings`;
// The chip's samples are the same bits on wasm32, under node.
await $`cargo test -p chip --target wasm32-unknown-unknown --test golden`;
await build(await examples());
rmSync(READ, { recursive: true, force: true });
for (const file of ['package.json', 'tsconfig.json', 'vite.config.ts', 'src', 'static']) {
	cpSync(`${WEB}/${file}`, `${READ}/${file}`, { recursive: true, filter: (from) => !from.startsWith(`${WEB}/src/lib/pkg`) });
}
symlinkSync(`${WEB}/node_modules`, `${READ}/node_modules`);
// Builds the chip as well, which the worklet's test reads.
await buildEngine(READ, 'types');
// The worklet plays the goldens as a page feeds it, on the chip just built.
await $`bun test ./crates/chip-worklet`;
await $`bun run check`.cwd(READ);
await $`bun test ./web/src ./scripts`;
console.log('the console passes');
