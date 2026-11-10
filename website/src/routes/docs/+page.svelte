<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { AVAILABLE, ISSUES } from '#lib/links.js';
	import PageTitle from '#lib/components/PageTitle.svelte';

	const sections = [
		{ id: 'kinds', title: m.docs_kinds_title, text: m.docs_kinds_text },
		{ id: 'actions', title: m.docs_actions_title, text: m.docs_actions_text },
		{ id: 'analysis', title: m.docs_analysis_title, text: m.docs_analysis_text },
		{ id: 'conditions', title: m.docs_conditions_title, text: m.docs_conditions_text },
		{ id: 'logs', title: m.docs_logs_title, text: m.docs_logs_text }
	];

	const keys = [
		{ keys: 'Ctrl+O, Ctrl+N', does: m.key_open },
		{ keys: 'F5, Ctrl+R', does: m.key_refresh },
		{ keys: 'Esc, Alt+←', does: m.key_back },
		{ keys: 'Ctrl+Q', does: m.key_quit }
	];
</script>

<div class="mx-auto max-w-[46rem] px-5 pb-24 md:px-8">
	<PageTitle title={m.docs_title()} lead={m.docs_lead()} />
	<nav aria-label={m.docs_title()} class="mb-4 flex flex-wrap gap-x-5 gap-y-2 text-[0.9rem]">
		<a href="#getting-started">{m.docs_start_title()}</a>
		{#each sections as section (section.id)}
			<a href="#{section.id}">{section.title()}</a>
		{/each}
		<a href="#keys">{m.docs_keys_title()}</a>
		<a href="#troubleshooting">{m.docs_trouble_title()}</a>
	</nav>
	<div class="prose-site">
		<h2 id="getting-started">{m.docs_start_title()}</h2>
		<ol>
			<li>{m.docs_start_1()}</li>
			<li>{m.docs_start_2()}</li>
			<li>{m.docs_start_3()}</li>
		</ol>
		{#each sections as section (section.id)}
			<h2 id={section.id}>{section.title()}</h2>
			<p>{section.text()}</p>
		{/each}
		<h2 id="keys">{m.docs_keys_title()}</h2>
		<ul>
			{#each keys as key (key.keys)}
				<li><code>{key.keys}</code> {key.does()}</li>
			{/each}
		</ul>
		<h2 id="troubleshooting">{m.docs_trouble_title()}</h2>
		<ul>
			<li>{m.docs_trouble_logout()}</li>
			<li>{m.docs_trouble_moved()}</li>
			<li>{m.docs_trouble_edited()}</li>
			{#if AVAILABLE}
				<li><a href={ISSUES}>{m.docs_trouble_more()}</a></li>
			{/if}
		</ul>
	</div>
</div>
