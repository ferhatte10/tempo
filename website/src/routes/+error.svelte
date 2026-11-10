<script lang="ts">
	import { page } from '$app/state';
	import { m } from '#lib/paraglide/messages.js';
	import { localizeHref } from '#lib/paraglide/runtime.js';
	import RunStrip from '#lib/components/RunStrip.svelte';

	const missing = $derived(page.status === 404);
</script>

<svelte:head>
	<title>{missing ? m.not_found_title() : m.error_title()} · Tempo</title>
</svelte:head>

<div class="mx-auto flex max-w-[40rem] flex-col items-start gap-5 px-5 py-24 md:px-8">
	<RunStrip />
	<h1 class="m-0 text-[clamp(2.2rem,5vw,3.25rem)] leading-tight font-extrabold">
		{missing ? m.not_found_title() : m.error_title()}
	</h1>
	<p class="m-0 text-[1.1rem] text-text-2">{m.not_found_text()}</p>
	<p class="m-0 flex flex-wrap gap-x-5 gap-y-2">
		<a href={localizeHref('/')}>{m.back_home()}</a>
		<a href={localizeHref('/install/')}>{m.nav_install()}</a>
		<a href={localizeHref('/docs/')}>{m.nav_docs()}</a>
	</p>
</div>
