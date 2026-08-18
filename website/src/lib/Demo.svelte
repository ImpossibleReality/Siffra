<script lang="ts">
	import { onMount } from 'svelte';
	import { load, run, type Line } from './siffra';

	// The toolbar wears the desktop app's own icons rather than copies of them. They are authored
	// with black strokes and fills, which the toolbar recolours to whatever it sets as its colour.
	import help from '../../../desktop/assets/icons/help.svg?raw';
	import pin from '../../../desktop/assets/icons/pin.svg?raw';
	import plus from '../../../desktop/assets/icons/plus.svg?raw';
	import search from '../../../desktop/assets/icons/search.svg?raw';

	const TOOLS = [plus, search, help, pin].map((icon) =>
		icon.replaceAll('"black"', '"currentColor"')
	);

	const SCRIPT = [
		'# Simple calculation',
		'5 + 2 / 4 - 2',
		'5 ^ log_(2+3)(100)',
		'3E00 / 4E62',
		'',
		'# Advanced calculations',
		'5 m + 1\'3"',
		'5 g O2 in mol * avogadro',
		'accel = 1 m/s^2',
		'0.5(accel)(5 yr)^2 -> ly',
		'5 USD in EUR',
		'1936-11-12 to today as yr,mo,d',
		'9pm EST to PST',
		'',
		'# Natural Language',
		'5% of what is 20',
		'$22 as a % on $20',
	].join('\n');

	/** Milliseconds per character, and the pause held at the end of each line. */
	const KEY_DELAY = 5;
	const KEY_JITTER = 40;
	const LINE_PAUSE = 30;

	/** The window keeps the finished document's height from the start, so it never jumps. */
	const ROWS = SCRIPT.split('\n').length + 2;

	let document = $state('');
	let lines = $state<Line[]>([]);
	let engineReady = $state(false);
	/** While a line is still being typed, its errors are half-finished input, not results. */
	let typingLine = $state<number | null>(null);
	let typing = $state(false);
	let focused = $state(false);

	let cancelled = false;
	let frame = 0;

	const rowCount = $derived(Math.max(lines.length + 1, ROWS));

	function refresh() {
		if (!engineReady) return;
		lines = run(document);
	}

	function resultOf(line: Line, index: number) {
		if (!line.result) return null;
		if (line.result.kind === 'error' && index === typingLine) return null;
		return line.result;
	}

	/**
	 * Typed off the frame clock rather than a timer, so the animation simply pauses while the tab
	 * is in the background and picks up where it left off instead of racing to catch up.
	 */
	function type() {
		typing = true;
		let index = 0;
		let due = performance.now();

		const step = (now: number) => {
			if (cancelled) return;

			// Returning to a backgrounded tab should resume, not fast-forward.
			if (now - due > LINE_PAUSE) due = now;

			let advanced = false;
			while (index < SCRIPT.length && now >= due) {
				const key = SCRIPT[index++];
				due += key === '\n' ? LINE_PAUSE : KEY_DELAY + Math.random() * KEY_JITTER;
				advanced = true;
			}

			if (advanced) {
				document = SCRIPT.slice(0, index);
				typingLine = document.split('\n').length - 1;
				refresh();
			}

			if (index >= SCRIPT.length) finish();
			else frame = requestAnimationFrame(step);
		};

		frame = requestAnimationFrame(step);
	}

	/** Hands the editor over to the visitor, whether the script ran out or they interrupted it. */
	function finish() {
		cancelled = true;
		cancelAnimationFrame(frame);
		typing = false;
		typingLine = null;
		document = SCRIPT;
		refresh();
	}

	function takeOver() {
		if (typing) finish();
	}

	/** Without the engine the demo still shows the document, just with no results beside it. */
	function showWithoutEngine() {
		document = SCRIPT;
		lines = SCRIPT.split('\n').map((text) => ({
			segments: [{ token: null, text }],
			result: null
		}));
	}

	onMount(() => {
		let stopped = false;
		load(() => !stopped && refresh()).then(
			() => {
				if (stopped) return;
				engineReady = true;
				const still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
				if (still) {
					document = SCRIPT;
					refresh();
					return;
				}
				refresh();
				type();
			},
			() => !stopped && showWithoutEngine()
		);
		return () => {
			stopped = true;
			cancelled = true;
			cancelAnimationFrame(frame);
		};
	});
</script>

<div class="window" class:live={!typing}>
	<header>
		<div class="lights" aria-hidden="true">
			<span class="light close"></span>
			<span class="light minimise"></span>
			<span class="light zoom"></span>
		</div>
		<span class="title">Live Demo</span>
		<div class="tools" aria-hidden="true">
			<!-- eslint-disable-next-line svelte/no-at-html-tags -- the app's own icon files -->
			{#each TOOLS as icon, i (i)}{@html icon}{/each}
		</div>
	</header>

	<div class="editor" style="--rows: {rowCount}">
		<div class="rows" aria-hidden="true">
			{#each lines as line, index (index)}
				{@const result = resultOf(line, index)}
				{@const heading = line.segments[0]?.token === 'header'}
				<div class="row" class:heading>
					<span class="input"
						>{#each line.segments as segment, i (i)}<span class={segment.token ?? 'plain'}
								>{segment.text}</span
							>{/each}{#if typing && index === typingLine}<span class="caret"></span>{/if}</span
					>
					{#if result}
						<span class="ruler"></span>
						{#if result.kind === 'error'}
							<span class="result error">{result.message}</span>
						{:else if result.repetend}
							{@const [from, to] = result.repetend}
							<span class="result"
								>{result.text.slice(0, from)}<span class="repetend"
									>{result.text.slice(from, to)}</span
								>{result.text.slice(to)}</span
							>
						{:else}
							<span class="result">{result.text}</span>
						{/if}
					{/if}
				</div>
			{/each}
		</div>

		<textarea
			bind:value={document}
			oninput={refresh}
			onpointerdown={takeOver}
			onkeydown={takeOver}
			onfocus={() => {
				focused = true;
				takeOver();
			}}
			onblur={() => (focused = false)}
			spellcheck="false"
			autocapitalize="off"
			autocomplete="off"
			aria-label="Siffra demo editor"></textarea>
	</div>

	{#if !typing && !focused}
		<p class="hint">Click anywhere in the window to try your own calculations.</p>
	{/if}
</div>

<style>
	.window {
		--gutter: 1.15rem;
		--line: 1.8em;
		position: relative;
		width: min(100%, 40rem);
		margin-inline: auto;
		background: var(--app-bg);
		border-radius: 0.9rem;
		border: 1px solid rgba(255, 255, 255, 0.07);
		box-shadow:
			0 1px 0 rgba(255, 255, 255, 0.06) inset,
			0 2.5rem 5rem -1rem rgba(0, 0, 0, 0.55),
			0 0.5rem 1.5rem -0.5rem rgba(0, 0, 0, 0.45);
		overflow: hidden;
	}

	header {
		display: grid;
		grid-template-columns: 1fr auto 1fr;
		align-items: center;
		height: 2.1rem;
		padding-inline: 0.6rem;
	}

	.lights {
		display: flex;
		gap: 0.5rem;
	}

	.light {
		width: 0.75rem;
		height: 0.75rem;
		border-radius: 50%;
	}

	.close {
		background: #ff5f57;
	}
	.minimise {
		background: #febc2e;
	}
	.zoom {
		background: #28c840;
	}

	.title {
		font-size: 0.8rem;
		font-weight: 600;
		color: var(--app-title);
		white-space: nowrap;
	}

	.tools {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 0.6rem;
		color: var(--app-title);
	}

	/* The icons arrive through {@html}, which Svelte's scoping does not reach into. */
	.tools :global(svg) {
		width: 1.05rem;
		height: 1.05rem;
	}

	.editor {
		position: relative;
		font-family: var(--mono);
		font-size: var(--code-size);
		font-weight: 600;
		line-height: var(--line);
		padding: 0.4rem var(--gutter) 1.4rem;
		/* Room for the script plus a couple of blank lines to type into. */
		min-height: calc(var(--rows) * var(--line) + 4rem);
	}

	.row {
		position: relative;
		display: flex;
		align-items: center;
		height: var(--line);
		white-space: pre;
	}

	/* Roboto Mono keeps one advance width across weights, so bold headings stay
	   aligned with the transparent textarea the visitor actually types into. */
	.row.heading {
		font-weight: 700;
		color: var(--tok-header);
	}

	/* The heading rule the app draws in the gutter. */
	.row.heading::before {
		content: '';
		position: absolute;
		left: -0.45em;
		width: 2px;
		height: 1.1em;
		border-radius: 1px;
		background: var(--tok-rule);
	}

	.input {
		position: relative;
		flex: 0 1 auto;
		min-width: 0;
		overflow: hidden;
	}

	.ruler {
		flex: 1 1 auto;
		min-width: 0.6rem;
		height: 2px;
		margin: 0 0.35rem 0 0.65rem;
		background: rgba(255, 255, 255, 0.09);
	}

	.result {
		flex: 0 0 auto;
		font-weight: 700;
		color: var(--tok-result);
	}

	.result.error {
		color: var(--tok-error);
		font-weight: 400;
		font-size: 0.9em;
	}

	/* The vinculum the app draws over a repetend. */
	.repetend {
		border-top: 1px solid currentColor;
	}

	.caret {
		display: inline-block;
		width: 2px;
		height: 1.05em;
		margin-inline: -1px;
		vertical-align: text-bottom;
		background: var(--app-text);
		animation: blink 1.06s steps(1, end) infinite;
	}

	@keyframes blink {
		0%,
		50% {
			opacity: 1;
		}
		50.01%,
		100% {
			opacity: 0;
		}
	}

	textarea {
		position: absolute;
		inset: 0.4rem var(--gutter) 1.4rem;
		width: calc(100% - 2 * var(--gutter));
		border: 0;
		padding: 0;
		margin: 0;
		resize: none;
		outline: none;
		background: transparent;
		color: transparent;
		caret-color: var(--app-text);
		font: inherit;
		line-height: inherit;
		white-space: pre;
		overflow: hidden;
	}

	textarea::selection {
		background: var(--app-selection);
	}

	/* The caret belongs to the animation until the visitor takes over. */
	.window:not(.live) textarea {
		caret-color: transparent;
	}

	.hint {
		position: absolute;
		inset-inline: 0;
		bottom: 0.5rem;
		margin: 0;
		text-align: center;
		font-size: 0.7rem;
		color: var(--app-title);
		opacity: 0.55;
		pointer-events: none;
	}

	.plain,
	.variable,
	.header {
		color: var(--tok-text);
	}
	.comment {
		color: var(--tok-comment);
	}
	.chemical {
		color: var(--tok-chemical);
	}
	.number {
		color: var(--tok-number);
	}
	.unit {
		color: var(--tok-unit);
	}
	.operation {
		color: var(--tok-operation);
	}

	/* Narrow screens shrink the type instead of clipping the longest line. */
	@media (max-width: 46rem) {
		.window {
			--gutter: 0.8rem;
			--code-size: 0.78rem;
		}
	}
</style>
