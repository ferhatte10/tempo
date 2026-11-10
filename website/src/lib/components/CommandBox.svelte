<script lang="ts">
	import { Check, Copy } from '@lucide/svelte';
	import { m } from '#lib/paraglide/messages.js';

	let { command, label }: { command: string; label?: string } = $props();

	let status = $state<'idle' | 'copied' | 'failed'>('idle');
	let timer: ReturnType<typeof setTimeout> | undefined;
	let code = $state<HTMLElement>();

	async function copy() {
		clearTimeout(timer);
		try {
			await navigator.clipboard.writeText(command);
			status = 'copied';
		} catch {
			// No clipboard access (old browser, refused): select the text instead.
			if (code) {
				const range = document.createRange();
				range.selectNodeContents(code);
				window.getSelection()?.removeAllRanges();
				window.getSelection()?.addRange(range);
			}
			status = 'failed';
		}
		timer = setTimeout(() => (status = 'idle'), 2500);
	}
</script>

<div class="flex flex-col gap-2">
	{#if label}
		<p class="m-0 text-[0.85rem] text-text-3">{label}</p>
	{/if}
	<div class="flex items-center gap-2 rounded-2xl bg-code-bg py-1.5 pr-1.5 pl-4">
		<code
			bind:this={code}
			class="min-w-0 flex-1 overflow-x-auto py-2.5 pr-3 font-mono text-[0.85rem] leading-relaxed whitespace-pre text-code-text [mask-image:linear-gradient(to_right,#000_calc(100%-1.5rem),transparent)] [scrollbar-width:none]"
			>{command}</code
		>
		<button
			type="button"
			onclick={copy}
			aria-label={m.copy_label()}
			class="press inline-flex h-11 min-w-11 flex-none items-center justify-center gap-2 rounded-xl bg-accent px-4 text-[0.85rem] font-bold text-on-accent hover:bg-accent-hover"
		>
			{#if status === 'copied'}
				<Check class="size-4" aria-hidden="true" />
				<span>{m.copied()}</span>
			{:else}
				<Copy class="size-4" aria-hidden="true" />
				<span class="max-sm:sr-only">{m.copy()}</span>
			{/if}
		</button>
	</div>
	<p class="sr-only" aria-live="polite">
		{status === 'copied' ? m.copied() : status === 'failed' ? m.copy_failed() : ''}
	</p>
	{#if status === 'failed'}
		<p class="m-0 text-[0.85rem] text-text-3">{m.copy_failed()}</p>
	{/if}
</div>
