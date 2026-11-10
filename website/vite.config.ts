import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { paraglideVitePlugin } from '@inlang/paraglide-js';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [
		tailwindcss(),
		paraglideVitePlugin({
			project: './project.inlang',
			outdir: './src/lib/paraglide',
			// English at /, French at /fr/…: the address says the language, so
			// every page can be prerendered as plain files.
			strategy: ['url', 'baseLocale']
		}),
		sveltekit({
			// Every page is a plain file served by nginx: nothing runs on the server.
			adapter: adapter({ fallback: '404.html', strict: true }),
			prerender: {
				// The French pages are reached through the language links.
				entries: ['*', '/fr']
			},
			// Only the site's own files, scripts by hash (SvelteKit's start-up
			// script is inline): no third party, no tracking, no framing.
			csp: {
				mode: 'hash',
				directives: {
					'default-src': ['self'],
					'script-src': ['self'],
					'style-src': ['self', 'unsafe-inline'],
					'img-src': ['self', 'data:'],
					'font-src': ['self'],
					'connect-src': ['self'],
					'object-src': ['none'],
					'base-uri': ['self'],
					'form-action': ['none']
				}
			}
		})
	]
});
