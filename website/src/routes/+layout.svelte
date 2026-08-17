<script lang="ts">
	import '@fontsource-variable/inter';
	// The app sets its editor in semibold and its results in bold.
	import '@fontsource/roboto-mono/600.css';
	import '@fontsource/roboto-mono/700.css';
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href="/favicon.png" />
</svelte:head>

{@render children()}
