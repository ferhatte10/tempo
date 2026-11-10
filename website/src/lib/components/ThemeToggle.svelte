<script lang="ts">
	import { Moon, Sun } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';

	// The icon and the label follow <html data-theme> (set by /theme.js before
	// the first paint) through CSS: right from the first frame, with no state
	// to sync after the page starts.
	function apply() {
		const dark = document.documentElement.dataset.theme !== 'dark';
		document.documentElement.dataset.theme = dark ? 'dark' : 'light';
		try {
			localStorage.setItem('theme', dark ? 'dark' : 'light');
		} catch {
			// Storage refused (private browsing): the choice lasts for this page.
		}
	}

	// The new theme spreads as a circle from the button (DESIGN.md, Motion);
	// at once where View Transitions are missing or motion is reduced.
	function toggle(event: MouseEvent) {
		const root = document.documentElement;
		if (!document.startViewTransition || matchMedia('(prefers-reduced-motion: reduce)').matches) {
			apply();
			return;
		}
		const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
		const x = box.left + box.width / 2;
		const y = box.top + box.height / 2;
		const radius = Math.hypot(Math.max(x, innerWidth - x), Math.max(y, innerHeight - y));
		root.classList.add('theme-switching');
		const transition = document.startViewTransition(apply);
		transition.ready
			.then(() =>
				root.animate(
					{ clipPath: [`circle(0 at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`] },
					{
						duration: 550,
						easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
						pseudoElement: '::view-transition-new(root)'
					}
				)
			)
			// Skipped (page in the background): the theme is applied all the same.
			.catch(() => {});
		transition.finished.finally(() => root.classList.remove('theme-switching'));
	}
</script>

<button
	type="button"
	onclick={toggle}
	class="press inline-flex size-11 items-center justify-center rounded-xl bg-surface text-text hover:text-link"
>
	<Moon class="size-5 dark:hidden" aria-hidden="true" />
	<Sun class="hidden size-5 dark:block" aria-hidden="true" />
	<span class="sr-only dark:hidden">{m.theme_to_dark()}</span>
	<span class="sr-only hidden dark:inline">{m.theme_to_light()}</span>
</button>
