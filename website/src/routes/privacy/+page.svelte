<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';
	import PageTitle from '#lib/components/PageTitle.svelte';
</script>

<div class="mx-auto max-w-[46rem] px-5 pb-24 md:px-8">
	<PageTitle title={m.privacy_title()} lead={m.privacy_lead()} />
	<div class="prose-site">
		<h2>{m.privacy_app_title()}</h2>
		<ul>
			<li>{m.privacy_app_1()}</li>
			<li>{m.privacy_app_2()}</li>
			<li>{m.privacy_app_3()}</li>
		</ul>
		<h2>{m.privacy_site_title()}</h2>
		<p>{m.privacy_site_text()}</p>
	</div>
</div>
