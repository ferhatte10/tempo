// Checks the WCAG contrast of every text/background pair the site uses, in
// both themes, from the tokens in src/app.css. Fails the check below 4.5:1
// for text (3:1 for icons).

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const css = readFileSync(join(dirname(fileURLToPath(import.meta.url)), '../src/app.css'), 'utf8');

function tokens(selector) {
	const start = css.indexOf(selector);
	const block = css.slice(css.indexOf('{', start) + 1, css.indexOf('}', start));
	return Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6})/gi)].map(([, name, hex]) => [name, hex]));
}

const light = tokens(':root {');
const dark = { ...light, ...tokens(":root[data-theme='dark'] {") };

function luminance(hex) {
	const [r, g, b] = [1, 3, 5].map((i) => {
		const c = parseInt(hex.slice(i, i + 2), 16) / 255;
		return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	});
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function ratio(a, b) {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

// [foreground, background, minimum]
const pairs = [
	['text', 'bg', 4.5],
	['text-2', 'bg', 4.5],
	['text-3', 'bg', 4.5],
	['text', 'bg-alt', 4.5],
	['text-2', 'bg-alt', 4.5],
	['text-3', 'bg-alt', 4.5],
	['link', 'bg', 4.5],
	['link', 'bg-alt', 4.5],
	['link', 'surface', 4.5],
	['text', 'surface', 4.5],
	['text-2', 'surface', 4.5],
	['code-text', 'code-bg', 4.5],
	['on-accent', 'accent', 4.5],
	['on-accent', 'accent-hover', 4.5],
	['tab-on-fg', 'tab-on-bg', 4.5],
	['icon', 'bg', 3],
	['icon', 'bg-alt', 3],
	['mark-red', 'bg', 3],
	['run-ok', 'bg', 3],
	['run-bad', 'bg', 3],
	['run-idle', 'bg', 3],
	['run-now', 'bg', 3],
	['run-ok', 'bg-alt', 3],
	['run-bad', 'bg-alt', 3],
	['run-idle', 'bg-alt', 3],
	['run-now', 'bg-alt', 3]
];

let failed = 0;
for (const [theme, colors] of [
	['light', light],
	['dark', dark]
]) {
	for (const [fg, bg, min] of pairs) {
		const value = ratio(colors[fg], colors[bg]);
		if (value < min) {
			failed++;
			console.error(`✗ ${theme}: --${fg} on --${bg} = ${value.toFixed(2)}:1 (needs ${min}:1)`);
		}
	}
}
if (failed) {
	process.exit(1);
}
console.log(`✓ contrast: ${pairs.length * 2} pairs pass in light and dark`);
