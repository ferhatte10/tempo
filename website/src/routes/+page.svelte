<script lang="ts">
	import { Feather, Languages, Lock, Play, ScrollText, ShieldCheck } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { localizeHref } from '#lib/paraglide/runtime.js';
	import { AVAILABLE } from '#lib/links.js';
	import RunStrip from '#lib/components/RunStrip.svelte';
	import Screenshot from '#lib/components/Screenshot.svelte';
	import TriangleMark from '#lib/components/TriangleMark.svelte';
	import { reveal } from '#lib/reveal.js';

	// Each block: a real screenshot beside what it shows.
	const blocks = [
		{ shot: 'review', alt: m.review_alt, title: m.review_title, text: m.review_text },
		{ shot: 'editor', alt: m.editor_alt, title: m.editor_title, text: m.editor_text },
		{ shot: 'failed', alt: m.failed_alt, title: m.failed_title, text: m.failed_text },
		{ shot: 'system', alt: m.system_alt, title: m.system_title, text: m.system_text }
	];

	const features = [
		{ icon: ScrollText, title: m.f_logs_title, text: m.f_logs_text },
		{ icon: Play, title: m.f_try_title, text: m.f_try_text },
		{ icon: ShieldCheck, title: m.f_safe_title, text: m.f_safe_text },
		{ icon: Feather, title: m.f_keys_title, text: m.f_keys_text },
		{ icon: Languages, title: m.f_lang_title, text: m.f_lang_text },
		{ icon: Lock, title: m.f_local_title, text: m.f_local_text }
	];
</script>

<svelte:head>
	<title>{m.home_title()}</title>
</svelte:head>

<section class="container-site flex flex-wrap items-center gap-x-14 gap-y-14 pt-10 pb-24 md:pt-16">
	<div class="flex min-w-0 flex-[1_1_30rem] flex-col gap-6">
		<h1 class="enter m-0 text-[clamp(2.6rem,7vw,4.25rem)] leading-[1.02] font-extrabold tracking-[-0.02em]">
			{m.hero_title_1()}<br />{m.hero_title_2()}
		</h1>
		<p class="enter m-0 max-w-[36ch] text-text-2 md:text-[1.1rem]" style="--d: 1">{m.hero_lead()}</p>
		<div class="enter flex flex-col gap-2" style="--d: 2">
			{#if !AVAILABLE}
				<p class="m-0 font-bold text-link">{m.soon_title()}</p>
				<p class="m-0 max-w-[40ch] text-text-2">{m.hero_soon_text()}</p>
			{/if}
		</div>
		<div class="enter flex flex-wrap items-center gap-x-5 gap-y-3" style="--d: 3">
			<a
				href={localizeHref('/install/')}
				class="press inline-flex min-h-12 items-center gap-2.5 rounded-xl bg-accent px-5 font-bold text-on-accent no-underline hover:bg-accent-hover hover:text-on-accent"
			>
				{AVAILABLE ? m.hero_install() : m.hero_how()}
			</a>
			<a href={localizeHref('/docs/')} class="font-bold">{m.hero_docs()}</a>
		</div>
	</div>
	<div class="enter-settle flex min-w-0 flex-[1_1_28rem] flex-col gap-4" style="--d: 2">
		<div class="flex items-center gap-3 self-end text-[0.85rem] text-text-3">
			<span>{m.hero_strip()}</span>
			<RunStrip />
		</div>
		<Screenshot name="main" alt={m.hero_screenshot_alt()} eager class="rounded-xl shadow-lg" />
	</div>
</section>

<section class="container-site flex flex-col gap-20 pb-24">
	<h2
		use:reveal
		class="m-0 flex items-center gap-3.5 text-[clamp(1.9rem,4vw,2.5rem)] leading-tight font-extrabold"
	>
		<TriangleMark />{m.features_title()}
	</h2>
	{#each blocks as block, index (block.shot)}
		<div class="flex flex-wrap items-center gap-x-14 gap-y-8 {index % 2 ? 'flex-row-reverse' : ''}">
			<div use:reveal class="min-w-0 flex-[1_1_26rem]">
				<Screenshot name={block.shot} alt={block.alt()} class="rounded-xl shadow-lg" />
			</div>
			<div use:reveal style="--i: 1" class="flex min-w-0 flex-[1_1_18rem] flex-col gap-3">
				<h3 class="m-0 text-[1.85rem] leading-tight font-semibold">{block.title()}</h3>
				<p class="m-0 text-text-2">{block.text()}</p>
			</div>
		</div>
	{/each}
</section>

<section class="border-y border-border bg-bg-alt">
	<div class="container-site grid grid-cols-[repeat(auto-fit,minmax(15rem,1fr))] gap-x-12 gap-y-10 py-20">
		{#each features as feature, index (feature.title)}
			<div use:reveal style="--i: {index % 3}" class="flex flex-col gap-2">
				<feature.icon class="size-7 text-icon" aria-hidden="true" />
				<h3 class="m-0 text-[1.35rem] font-semibold">{feature.title()}</h3>
				<p class="m-0 text-text-2">{feature.text()}</p>
			</div>
		{/each}
	</div>
</section>

<section class="container-site flex flex-wrap gap-12 py-20">
	<div use:reveal class="min-w-0 flex-[1_1_20rem]">
		<h2 class="m-0 mb-3 text-[1.85rem] font-extrabold">{m.why_title()}</h2>
		<p class="m-0 text-text-2">{m.why_text()}</p>
	</div>
	<div use:reveal style="--i: 1" class="min-w-0 flex-[1_1_20rem]">
		<h2 class="m-0 mb-3 text-[1.85rem] font-extrabold">{m.runs_on_title()}</h2>
		<p class="m-0 text-text-2">
			{m.runs_on_text()} <a href={localizeHref('/install/')}>{m.runs_on_link()}</a>.
		</p>
	</div>
</section>
