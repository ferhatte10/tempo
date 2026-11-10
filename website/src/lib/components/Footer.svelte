<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import { localizeHref } from '#lib/paraglide/runtime.js';
	import { AUTHOR_NAME, AUTHOR_SITE, AVAILABLE, GITHUB } from '#lib/links.js';
</script>

<footer class="mt-auto border-t border-border">
	<div class="container-site flex flex-wrap gap-x-8 gap-y-3 py-8 text-[0.85rem] text-text-3">
		<span>{m.footer_license()} <a href={AUTHOR_SITE}>{AUTHOR_NAME}</a>.</span>
		<span class="flex flex-wrap gap-x-4 gap-y-1 sm:ml-auto">
			{#if AVAILABLE}<a href={GITHUB}>{m.nav_source()}</a>{/if}
			<a href={localizeHref('/docs/')}>{m.nav_docs()}</a>
			<a href={localizeHref('/privacy/')}>{m.nav_privacy()}</a>
		</span>
	</div>
</footer>
