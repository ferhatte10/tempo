<script lang="ts">
	import '../app.css';
	import { onNavigate } from '$app/navigation';
	import { page } from '$app/state';
	import { m } from '#lib/paraglide/messages.js';
	import { deLocalizeHref, localizeHref, locales } from '#lib/paraglide/runtime.js';
	import Header from '#lib/components/Header.svelte';
	import Footer from '#lib/components/Footer.svelte';
	import { versioned } from '#lib/assets.js';
	import displayFont from '@fontsource-variable/bricolage-grotesque/files/bricolage-grotesque-latin-wght-normal.woff2?url';
	import textFont from '@fontsource/atkinson-hyperlegible/files/atkinson-hyperlegible-latin-400-normal.woff2?url';
	import textBoldFont from '@fontsource/atkinson-hyperlegible/files/atkinson-hyperlegible-latin-700-normal.woff2?url';

	let { children } = $props();

	// Pages fade through each other (DESIGN.md, Motion; styles in app.css).
	onNavigate((navigation) => {
		if (!document.startViewTransition || matchMedia('(prefers-reduced-motion: reduce)').matches) {
			return;
		}
		return new Promise((resolve) => {
			const transition = document.startViewTransition(async () => {
				resolve();
				// Aborted by a newer click: that navigation brings its own transition.
				await navigation.complete.catch(() => {});
			});
			// Skipped (page in the background, another click): nothing to animate.
			transition.ready.catch(() => {});
		});
	});

	const origin = 'https://tempo.ferhatsn.fr';
	const path = $derived(deLocalizeHref(page.url.pathname));
</script>

<svelte:head>
	<!-- Fetched with the page, not after the stylesheet: the first paint
	     already has the site's fonts, nothing swaps a moment later. -->
	{#each [displayFont, textFont, textBoldFont] as font (font)}
		<link rel="preload" href={font} as="font" type="font/woff2" crossorigin="anonymous" />
	{/each}
	<meta name="description" content={m.site_description()} />
	<link rel="canonical" href={origin + localizeHref(path)} />
	{#each locales as locale (locale)}
		<link rel="alternate" hreflang={locale} href={origin + localizeHref(path, { locale })} />
	{/each}
	<meta property="og:site_name" content="Tempo" />
	<meta property="og:image" content={origin + versioned('/screenshots/en/main.png')} />
</svelte:head>

<a
	href="#content"
	class="sr-only z-10 rounded-lg bg-accent px-4 py-2 text-on-accent focus:not-sr-only focus:absolute focus:top-3 focus:left-3"
	>{m.skip_to_content()}</a
>
<Header />
<main id="content" class="flex-1">
	{@render children()}
</main>
<Footer />
