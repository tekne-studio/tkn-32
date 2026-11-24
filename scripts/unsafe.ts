// Where the console writes `unsafe`. Its workspace forbids it; the names a
// WebAssembly host knows a module by are the exception: a cartridge's
// contract seven, the chip in its AudioWorklet three, and the runner's calls
// both ways, its engine's imports and its own exports, one file that allows
// it whole: each in one file, in a crate that denies the rest. An example writes none: `cartridge::cart!`
// writes its exports, so it denies `unsafe` too. Every other crate takes the
// workspace's lints.
import { existsSync, readdirSync, readFileSync } from 'node:fs';

/** The files that write `unsafe`, and how many times each allows it. */
const ABIS: Record<string, number> = {
	'crates/cartridge/src/abi.rs': 7,
	'crates/chip-worklet/src/lib.rs': 3,
	'crates/runner/src/abi.rs': 1
};

const DENIES = /^\[lints\.rust\]\nunsafe_code = "deny"$/m;
const INHERITS = /^\[lints\]\nworkspace = true$/m;

/** What breaks the rule in the console at `root`, each as `file: what`. */
export function unsafeProblems(root: string): string[] {
	const problems: string[] = [];
	const read = (file: string) => readFileSync(`${root}/${file}`, 'utf8');
	const cratesIn = (dir: string) =>
		readdirSync(`${root}/${dir}`)
			.filter((name) => existsSync(`${root}/${dir}/${name}/Cargo.toml`))
			.map((name) => `${dir}/${name}`);

	const said = Object.entries(ABIS).map(([file, marks]) => `${file} holds ${marks}`).join(', ');
	const sources = [...new Bun.Glob('{crates,examples}/**/*.rs').scanSync(root)].filter((file) => !file.includes('/target/'));
	for (const file of new Set([...sources, ...Object.keys(ABIS)])) {
		const marks = existsSync(`${root}/${file}`) ? (read(file).match(/allow\(unsafe_code\)/g)?.length ?? 0) : 0;
		if (marks !== (ABIS[file] ?? 0)) problems.push(`${file}: ${marks} allow(unsafe_code); ${said} and no other file any`);
	}
	const denying = new Set(Object.keys(ABIS).map((file) => file.slice(0, file.indexOf('/src/'))));
	for (const crate of cratesIn('crates')) {
		const toml = read(`${crate}/Cargo.toml`);
		if (denying.has(crate)) {
			if (!DENIES.test(toml)) problems.push(`${crate}/Cargo.toml: it denies unsafe_code, and allows it in its ABI file alone`);
		} else if (!INHERITS.test(toml)) {
			problems.push(`${crate}/Cargo.toml: no \`[lints] workspace = true\`, so \`unsafe\` is not forbidden there`);
		}
	}
	for (const example of cratesIn('examples')) {
		if (!DENIES.test(read(`${example}/Cargo.toml`))) {
			problems.push(`${example}/Cargo.toml: a cart denies unsafe_code; only the exports \`cartridge::cart!\` writes allow it`);
		}
	}
	return problems;
}
