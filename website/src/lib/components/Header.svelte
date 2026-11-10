<script lang="ts">
	import { Check, Languages, Menu, X } from '@lucide/svelte';
	import { page } from '$app/state';
	import { m } from '#lib/paraglide/messages.js';
	import { deLocalizeHref, getLocale, locales, localizeHref } from '#lib/paraglide/runtime.js';
	import { SIDE_BY_SIDE, languageHref, nativeName, rememberLanguage } from '#lib/language.js';
	import LanguageMenu from './LanguageMenu.svelte';
	import ThemeToggle from './ThemeToggle.svelte';

	const links = [
		{ href: '/install/', label: m.nav_install },
		{ href: '/docs/', label: m.nav_docs },
		{ href: '/privacy/', label: m.nav_privacy }
	];

	const path = $derived(deLocalizeHref(page.url.pathname));

	// Phone menu: closed again after each navigation.
	let open = $state(false);
	$effect(() => {
		void page.url.pathname;
		open = false;
	});
</script>

<header class="container-site flex flex-wrap items-center gap-x-7 gap-y-3 py-4 md:py-5">
	<a
		href={localizeHref('/')}
		class="flex min-h-11 items-center gap-3 font-display text-[1.35rem] font-extrabold text-text no-underline hover:text-text"
	>
		<img src="/icon.svg" alt="" width="40" height="40" class="size-10" />
		Tempo
	</a>

	<!-- Wide screens: the links in a row. -->
	<nav aria-label={m.nav_label()} class="ml-auto hidden items-center gap-x-6 text-[0.9rem] md:flex">
		{#each links as link (link.href)}
			{@const current = path === link.href}
			<a
				href={localizeHref(link.href)}
				aria-current={current ? 'page' : undefined}
				class="border-b-2 pb-0.5 no-underline {current
					? 'border-mark-red text-text'
					: 'border-transparent text-text-2 hover:text-text'}">{link.label()}</a
			>
		{/each}
		<LanguageMenu />
		<ThemeToggle />
	</nav>

	<!-- Phones: theme button and a menu that unfolds under the header, with the
	     languages at its end (a third button would not fit beside the name). -->
	<div class="ml-auto flex items-center gap-2 md:hidden">
		<ThemeToggle />
		<button
			type="button"
			aria-expanded={open}
			aria-controls="phone-menu"
			aria-label={open ? m.menu_close() : m.menu_open()}
			onclick={() => (open = !open)}
			class="press inline-flex size-11 items-center justify-center rounded-xl bg-surface text-text"
		>
			{#if open}
				<X class="size-5" aria-hidden="true" />
			{:else}
				<Menu class="size-5" aria-hidden="true" />
			{/if}
		</button>
	</div>
	{#if open}
		<nav id="phone-menu" aria-label={m.nav_label()} class="w-full md:hidden">
			<ul class="m-0 flex list-none flex-col border-t border-border p-0">
				{#each links as link (link.href)}
					{@const current = path === link.href}
					<li class="border-b border-border">
						<a
							href={localizeHref(link.href)}
							aria-current={current ? 'page' : undefined}
							class="flex min-h-12 items-center gap-3 text-[1.05rem] no-underline {current
								? 'font-bold text-text'
								: 'text-text-2'}"
						>
							{#if current}<span class="h-5 w-1 rounded-full bg-mark-red" aria-hidden="true"></span>{/if}
							{link.label()}
						</a>
					</li>
				{/each}
			</ul>
			<!-- Languages: side by side while few (one line), else a list that
			     scrolls on its own, so the menu never grows past the screen. -->
			<div class="mt-5 mb-2 flex flex-wrap items-center justify-between gap-x-4 gap-y-3">
				<p id="phone-languages" class="m-0 flex items-center gap-2 text-[0.95rem] text-text-2">
					<Languages class="size-5" aria-hidden="true" />
					{m.language_label()}
				</p>
				{#if locales.length <= SIDE_BY_SIDE}
					<div aria-labelledby="phone-languages" role="group" class="flex rounded-xl bg-surface p-1">
						{#each locales as locale (locale)}
							{@const current = locale === getLocale()}
							<a
								href={languageHref(page.url, locale)}
								data-sveltekit-reload
								onclick={() => rememberLanguage(locale)}
								hreflang={locale}
								lang={locale}
								aria-current={current ? 'true' : undefined}
								class="press flex min-h-11 items-center rounded-lg px-4 text-[0.95rem] no-underline {current
									? 'bg-bg-alt font-bold text-text shadow-sm'
									: 'text-text-2 hover:text-text'}">{nativeName(locale)}</a
							>
						{/each}
					</div>
				{/if}
			</div>
			{#if locales.length > SIDE_BY_SIDE}
				<ul aria-labelledby="phone-languages" class="m-0 flex max-h-[40dvh] list-none flex-col overflow-y-auto p-0">
					{#each locales as locale (locale)}
						{@const current = locale === getLocale()}
						<li class="border-b border-border">
							<a
								href={languageHref(page.url, locale)}
								data-sveltekit-reload
								onclick={() => rememberLanguage(locale)}
								hreflang={locale}
								lang={locale}
								aria-current={current ? 'true' : undefined}
								class="flex min-h-12 items-center justify-between text-[1.05rem] no-underline {current
									? 'font-bold text-text'
									: 'text-text-2'}"
							>
								{nativeName(locale)}
								{#if current}<Check class="size-5 text-icon" aria-hidden="true" />{/if}
							</a>
						</li>
					{/each}
				</ul>
			{/if}
		</nav>
	{/if}
</header>
