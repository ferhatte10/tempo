<script lang="ts">
	import { DropdownMenu } from 'bits-ui';
	import { Check, Languages } from '@lucide/svelte';
	import { page } from '$app/state';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale, locales, type Locale } from '#lib/paraglide/runtime.js';
	import { languageHref, nativeName, rememberLanguage } from '#lib/language.js';

	const current = getLocale();

	// A new document, not a client-side navigation (see language.ts).
	function choose(locale: string) {
		if (locale === current) return;
		rememberLanguage(locale as Locale);
		window.location.assign(languageHref(page.url, locale as Locale));
	}
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		aria-label={m.language_choose({ language: nativeName(current) })}
		class="press inline-flex h-11 items-center gap-1.5 rounded-xl bg-surface px-3 text-text hover:text-link"
	>
		<Languages class="size-5" aria-hidden="true" />
		<span class="text-[0.85rem] font-bold uppercase" aria-hidden="true">{current}</span>
	</DropdownMenu.Trigger>
	<DropdownMenu.Portal>
		<DropdownMenu.Content
			align="end"
			sideOffset={8}
			class="z-20 max-h-[min(22rem,var(--bits-dropdown-menu-content-available-height))] min-w-44 overflow-y-auto rounded-xl border border-border bg-bg-alt p-1.5 shadow-md"
		>
			<DropdownMenu.RadioGroup value={current} onValueChange={choose}>
				{#each locales as locale (locale)}
					<DropdownMenu.RadioItem
						value={locale}
						lang={locale}
						class="flex min-h-11 items-center justify-between gap-4 rounded-lg px-3 text-text outline-none data-highlighted:bg-surface"
					>
						{#snippet children({ checked })}
							{nativeName(locale)}
							{#if checked}<Check class="size-4 text-icon" aria-hidden="true" />{/if}
						{/snippet}
					</DropdownMenu.RadioItem>
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Portal>
</DropdownMenu.Root>
