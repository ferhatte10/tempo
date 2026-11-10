<script lang="ts">
	// The app's run strip: ten past runs, oldest first, then the next one (an
	// open circle). The site's one decoration (DESIGN.md, Identity): the bars
	// grow one after the other on load, then the next run appears. Colours are
	// the app's states; the strip is decoration, its meaning is in the text.
	const runs = ['ok', 'ok', 'ok', 'idle', 'ok', 'ok', 'ok', 'ok', 'bad', 'ok'] as const;
	const colour = { ok: 'var(--run-ok)', bad: 'var(--run-bad)', idle: 'var(--run-idle)' };
</script>

<svg aria-hidden="true" viewBox="0 0 172 32" class="h-8 w-auto overflow-visible">
	{#each runs as run, index (index)}
		<rect class="run-bar" style="--b: {index}" x={index * 14} y="2" width="8" height="28" rx="4" fill={colour[run]} />
	{/each}
	<circle class="run-next" cx="157" cy="16" r="9" fill="none" stroke="var(--run-now)" stroke-width="3" />
</svg>
