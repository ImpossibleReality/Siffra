import init, { evaluate, install_rates } from './wasm/siffra.js';
import wasmUrl from './wasm/siffra_bg.wasm?url';

export type Segment = { token: string | null; text: string };

export type Output =
	| { kind: 'value'; text: string; repetend: [number, number] | null }
	| { kind: 'error'; message: string; description: string | null };

export type Line = { segments: Segment[]; result: Output | null };

/**
 * Width of the result column in characters. The app derives this from its own layout; the demo
 * pins it to the width the screenshot was taken at, which keeps long decimals readable.
 */
export const BUDGET = 8;

// The engine's own source, api.frankfurter.dev, sends no CORS headers, so a browser cannot read it.
const RATES_URL = 'https://open.er-api.com/v6/latest/EUR';

/** Rates relative to the euro, used until (or unless) the live ones arrive. */
const FALLBACK_RATES: Record<string, number> = {
	USD: 1.15,
	GBP: 0.86,
	JPY: 172.0,
	CHF: 0.94,
	CAD: 1.6,
	AUD: 1.79,
	SEK: 11.2,
	NOK: 11.7,
	DKK: 7.46,
	CNY: 8.24,
	INR: 100.9
};

let ready: Promise<void> | null = null;

/**
 * Loads the engine once; further calls await the same load. `onRatesChanged` fires if live exchange
 * rates arrive after the load, so anything already on screen can be recomputed.
 */
export function load(onRatesChanged?: () => void): Promise<void> {
	ready ??= (async () => {
		await init({ module_or_path: wasmUrl });
		applyRates(FALLBACK_RATES);
		void refreshRates(onRatesChanged);
	})();
	return ready;
}

function applyRates(rates: Record<string, number>) {
	install_rates(Object.keys(rates), Float64Array.from(Object.values(rates)));
}

/**
 * The engine reaches the network itself on desktop, but cannot from wasm, so the page fetches
 * today's rates and hands them over. A failure leaves the fallback table in place.
 */
async function refreshRates(onRatesChanged?: () => void) {
	try {
		const response = await fetch(RATES_URL);
		if (!response.ok) return;
		const body = (await response.json()) as { rates?: Record<string, number> };
		if (!body.rates) return;
		applyRates(body.rates);
		onRatesChanged?.();
	} catch {
		// Offline, or the API is down: the fallback rates still give sensible answers.
	}
}

/** Evaluates a document, returning one entry per line. */
export function run(document: string): Line[] {
	return evaluate(document, BUDGET) as Line[];
}
