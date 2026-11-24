// Shared by every script in this folder.
import { fileURLToPath } from 'node:url';

/** The console's folder, wherever a script runs from. */
export const ROOT = fileURLToPath(new URL('..', import.meta.url)).replace(/\/$/, '');

/** Where the workbench's page listens. */
export const HOST = '127.0.0.1';

/**
 * `preferred` when nothing holds it on `HOST`, else the first port above it
 * that is free, as Vite picks the page's: several checkouts run the
 * workbench side by side, each on a port one can remember.
 */
export function freePort(preferred: number): number {
	for (let port = preferred; port < preferred + 100; port++) {
		try {
			Bun.serve({ hostname: HOST, port, fetch: () => new Response() }).stop(true);
			return port;
		} catch (error) {
			if ((error as { code?: string }).code !== 'EADDRINUSE') throw error;
		}
	}
	throw new Error(`no free port from ${preferred} on`);
}
