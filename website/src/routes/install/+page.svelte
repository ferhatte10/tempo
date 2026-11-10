<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import {
		AVAILABLE,
		INSTALL_COMMAND,
		INSTALL_SCRIPT_SOURCE,
		RELEASES,
		SOURCE_BUILD,
		SOURCE_DEPS,
		SOURCE_REMOVE,
		SOURCE_RUST,
		UNINSTALL_COMMAND
	} from '#lib/links.js';
	import CommandBox from '#lib/components/CommandBox.svelte';
	import PageTitle from '#lib/components/PageTitle.svelte';

	const packageRemove = 'sudo apt remove tempo   # or: sudo dnf remove tempo';
</script>

<div class="mx-auto max-w-[52rem] px-5 pb-24 md:px-8">
	<PageTitle
		title={m.install_page_title()}
		lead={AVAILABLE ? m.install_page_lead() : m.install_soon_lead()}
		tab={m.nav_install()}
	/>

	{#if AVAILABLE}
		<h2 class="mt-6 mb-2 text-[1.75rem] font-extrabold">{m.install_one_title()}</h2>
		<p class="m-0 mb-6 text-text-2">{m.install_one_text()}</p>
		<CommandBox command={INSTALL_COMMAND} />
		<p class="mt-3 mb-0 text-[0.85rem] text-text-3">
			<a href={INSTALL_SCRIPT_SOURCE}>{m.install_links_script()}</a>
		</p>
	{:else}
		<div class="enter flex flex-col gap-2 rounded-2xl bg-surface p-6" style="--d: 1">
			<p class="m-0 font-display text-xl font-semibold">{m.soon_title()}</p>
			<p class="m-0 text-text-2">{m.install_soon_text()}</p>
		</div>
	{/if}

	<h2 class="mt-16 mb-2 text-[1.75rem] font-extrabold">{m.requirements_title()}</h2>
	<p class="m-0 text-text-2">{m.requirements_text()}</p>

	{#if AVAILABLE}
		<h2 class="mt-16 mb-2 text-[1.75rem] font-extrabold">{m.packages_title()}</h2>
		<p class="m-0 text-text-2">{m.packages_text()} <a href={RELEASES}>{m.packages_link()}</a>.</p>

		<h2 class="mt-16 mb-2 text-[1.75rem] font-extrabold">{m.source_title()}</h2>
		<p class="m-0 mb-6 text-text-2">{m.source_lead()}</p>
		<div class="flex flex-col gap-6">
			<CommandBox command={SOURCE_DEPS} label={m.source_deps()} />
			<CommandBox command={SOURCE_RUST} label={m.source_rust()} />
			<CommandBox command={SOURCE_BUILD} label={m.source_build()} />
		</div>

		<h2 class="mt-16 mb-2 text-[1.75rem] font-extrabold">{m.uninstall_title()}</h2>
		<p class="m-0 mb-6 text-text-2">{m.uninstall_text()}</p>
		<div class="flex flex-col gap-5">
			<CommandBox command={UNINSTALL_COMMAND} label={m.uninstall_script()} />
			<CommandBox command={packageRemove} label={m.uninstall_package()} />
			<CommandBox command={SOURCE_REMOVE} label={m.uninstall_source()} />
		</div>
	{/if}
</div>
