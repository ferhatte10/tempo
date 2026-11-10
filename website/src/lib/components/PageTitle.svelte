<script lang="ts">
	import TriangleMark from './TriangleMark.svelte';

	// `tab` is the browser tab's title when the heading already names the app.
	let { title, lead, tab }: { title: string; lead?: string; tab?: string } = $props();
</script>

<svelte:head>
	<title>{tab ?? title} · Tempo</title>
</svelte:head>

<div class="enter flex flex-col gap-3 pt-10 pb-6 md:pt-14">
	<h1 class="m-0 flex items-center gap-3.5 text-[clamp(2.2rem,5vw,3.25rem)] leading-tight font-extrabold">
		<TriangleMark />{title}
	</h1>
	{#if lead}
		<p class="m-0 max-w-[48ch] text-[1.1rem] text-text-2">{lead}</p>
	{/if}
</div>
