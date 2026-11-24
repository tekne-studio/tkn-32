import { existsSync, readFileSync, watch } from 'node:fs';
import { join } from 'node:path';
import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, type Plugin } from 'vite';

/**
 * The carts `bun run workbench` built, served at `/carts/` from the folder
 * it names (`WORKBENCH_CARTS`), outside the page's own files so a new
 * build never reloads the page. Each time it rebuilds one it writes how
 * the build went to `build.json` there; the page, told, fetches the new
 * file or shows the compiler's words.
 */
function carts(): Plugin {
	const dir = process.env.WORKBENCH_CARTS;
	return {
		name: 'workbench-carts',
		configureServer(server) {
			if (!dir) return;
			server.middlewares.use('/carts', (request, response, next) => {
				const name = decodeURIComponent((request.url ?? '').split('?')[0]).replace(/^\//, '');
				if (!/^[a-z0-9_-]+(\.symbols)?\.(wasm|json)$/.test(name) || !existsSync(join(dir, name))) return next();
				response.setHeader('content-type', name.endsWith('.wasm') ? 'application/wasm' : 'application/json');
				response.setHeader('cache-control', 'no-store');
				response.end(readFileSync(join(dir, name)));
			});
			watch(dir, (_, file) => {
				if (file !== 'build.json') return;
				try {
					const built = JSON.parse(readFileSync(join(dir, file), 'utf8'));
					server.ws.send({ type: 'custom', event: 'workbench:build', data: built });
				} catch {
					// Read while being written: the next change tells it.
				}
			});
		}
	};
}

export default defineConfig({
	plugins: [
		carts(),
		sveltekit({
			compilerOptions: {
				// Runes mode for every project file; libraries keep their own mode.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			// The workbench is only files: it runs a cartridge wholly in the
			// browser, so any static host serves it.
			adapter: adapter({ fallback: '404.html' })
		})
	],
	// The bench's worker loads its engine as a module of its own.
	worker: { format: 'es' },
	server: { port: 5180 }
});
