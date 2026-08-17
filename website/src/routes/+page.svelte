<script lang="ts">
	import Demo from '$lib/Demo.svelte';

	const REPO = 'https://github.com/ImpossibleReality/Siffra';
	const RELEASES = `${REPO}/releases/latest`;

	const platform = $derived.by(() => {
		if (typeof navigator === 'undefined') return null;
		const agent = navigator.userAgent;
		if (/Mac/.test(agent)) return 'macOS';
		if (/Win/.test(agent)) return 'Windows';
		if (/Linux|X11/.test(agent)) return 'Linux';
		return null;
	});
</script>

<svelte:head>
	<title>Siffra</title>
	<meta
		name="description"
		content="Siffra is a lightweight, fast, and accurate calculator with dimensional analysis, built with Rust and GPUI."
	/>
	<meta property="og:title" content="Siffra" />
	<meta
		property="og:description"
		content="A lightweight, fast, and accurate calculator with dimensional analysis."
	/>
</svelte:head>

<main>
	<div class="stage">
		<section class="hero">
			<img src="/icon.png" alt="" width="72" height="72" />
			<h1>Siffra</h1>
			<p class="tagline">A lightweight, fast, and accurate calculator</p>
		</section>

		<section class="demo">
			<Demo />
		</section>

		<section class="download">
			<div class="actions">
				<a class="primary" href={RELEASES}>
					Download{platform ? ` for ${platform}` : ''}
				</a>
				<a class="secondary" href={REPO}>View source</a>
			</div>
			<p class="note">Free and open source. Support for macOS, Windows, and Linux</p>
		</section>
	</div>

	<footer>
		<a href={REPO}>GitHub</a>
		<span>·</span>
		<a href={RELEASES}>Releases</a>
		<span>·</span>
		<a href="{REPO}/blob/main/LICENCE.txt">MIT</a>
	</footer>
</main>

<style>
	main {
		width: min(100% - 2.5rem, 64rem);
		margin-inline: auto;
		padding-block: clamp(3.5rem, 10vh, 7rem) 3rem;
	}

	.hero {
		text-align: center;
		margin-bottom: 2rem;
	}

	.hero img {
		border-radius: 1rem;
		margin-bottom: 1.25rem;
	}

	h1 {
		margin: 0 0 0.75rem;
		font-size: clamp(2.6rem, 7vw, 3.6rem);
		font-weight: 700;
		letter-spacing: -0.03em;
	}

	.tagline {
		margin: 0 auto;
		max-width: 34rem;
		text-wrap: balance;
		font-size: clamp(1rem, 2.4vw, 1.2rem);
		line-height: 1.55;
		color: var(--page-muted);
	}

	.download {
		text-align: center;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: 0.75rem;
		margin-top: 2rem;
	}

	.actions a {
		display: inline-block;
		padding: 0.7rem 1.4rem;
		border-radius: 0.6rem;
		font-size: 0.95rem;
		font-weight: 600;
		text-decoration: none;
		transition:
			background 0.15s ease,
			border-color 0.15s ease,
			transform 0.15s ease;
	}

	.actions a:active {
		transform: translateY(1px);
	}

	.primary {
		background: var(--tok-result);
		color: #1a2113;
	}

	.primary:hover {
		background: #a9d189;
	}

	.secondary {
		border: 1px solid var(--page-line);
		color: var(--page-text);
	}

	.secondary:hover {
		background: rgba(255, 255, 255, 0.05);
	}

	.note {
		margin: 1rem 0 0;
		font-size: 0.8rem;
		color: var(--page-muted);
		opacity: 0.75;
	}

	footer {
		margin-top: 4rem;
		display: flex;
		justify-content: center;
		gap: 0.6rem;
		padding-top: 2rem;
		border-top: 1px solid var(--page-line);
		font-size: 0.85rem;
		color: var(--page-muted);
	}

	footer a {
		text-decoration: none;
	}

	footer a:hover {
		color: var(--page-text);
	}

	/*
	 * Wide enough for two columns and tall enough for the demo window to stand beside the pitch
	 * without either being cut off: the page becomes one viewport, footer at its base. Grid areas
	 * do the rearranging so the narrow layout keeps its source order of hero, demo, download.
	 */
	@media (min-width: 64rem) and (min-height: 48rem) {
		main {
			display: flex;
			flex-direction: column;
			min-height: 100dvh;
			width: min(100% - 4rem, 78rem);
			padding-block: 2rem 0;
		}

		.stage {
			flex: 1;
			display: grid;
			grid-template-columns: minmax(0, 1fr) minmax(0, 1.05fr);
			grid-template-areas:
				'hero demo'
				'download demo';
			align-content: center;
			column-gap: clamp(2.5rem, 5vw, 4.5rem);
			padding-block: 1.5rem;
		}

		/*
		 * The demo spans both rows and is the taller column, so the two rows stretch to meet it.
		 * Anchoring the pitch to the row boundary keeps it one centred block rather than spreading
		 * it over the demo's full height.
		 */
		.hero {
			grid-area: hero;
			align-self: end;
			text-align: left;
			margin-bottom: 0;
		}

		.tagline {
			margin-inline: 0;
		}

		.demo {
			grid-area: demo;
			align-self: center;
		}

		.download {
			grid-area: download;
			align-self: start;
			text-align: left;
		}

		.actions {
			justify-content: flex-start;
		}

		footer {
			margin-top: 0;
			padding-bottom: 2rem;
		}
	}
</style>
