// Copies what the site shows from the app's own sources, so nothing is kept
// twice: the icon, install.sh and the screenshots (`make screenshots` in the
// repository).
// Runs before dev, build and check. The copies are not committed (see
// .gitignore).

import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const site = join(dirname(fileURLToPath(import.meta.url)), '..');
const repo = join(site, '..');

function copy(from, to) {
	mkdirSync(dirname(to), { recursive: true });
	copyFileSync(from, to);
}

copy(join(repo, 'data/icons/hicolor/scalable/apps/fr.ferhatsn.Tempo.svg'), join(site, 'static/icon.svg'));
copy(join(repo, 'packaging/install.sh'), join(site, 'static/install.sh'));

// The site's name of each screenshot, and the app's (docs/screenshots/<lang>/
// <shot>-900-<theme>.png). Copied as <name>.png and <name>-dark.png.
const shots = {
	main: 'home',
	detail: 'detail-yours',
	failed: 'detail-failed',
	system: 'detail-system',
	review: 'review-warn',
	editor: 'editor-new',
	logs: 'logs-failed'
};
for (const lang of ['en', 'fr']) {
	const from = join(repo, 'docs/screenshots', lang);
	for (const [name, shot] of Object.entries(shots)) {
		copy(join(from, `${shot}-900-light.png`), join(site, 'static/screenshots', lang, `${name}.png`));
		copy(join(from, `${shot}-900-dark.png`), join(site, 'static/screenshots', lang, `${name}-dark.png`));
	}
}

// Fingerprints of the screenshots: the pages ask for
// `/screenshots/en/main.png?v=<fingerprint>` (src/lib/assets.ts), so a new
// version has a new address and these files can be cached for a year (nginx).
const fingerprints = {};
function fingerprint(dir) {
	for (const name of readdirSync(join(site, 'static', dir))) {
		const path = join(dir, name);
		if (statSync(join(site, 'static', path)).isDirectory()) {
			fingerprint(path);
		} else {
			const bytes = readFileSync(join(site, 'static', path));
			fingerprints['/' + path] = createHash('sha256').update(bytes).digest('hex').slice(0, 10);
		}
	}
}
fingerprint('screenshots');
mkdirSync(join(site, 'src/lib/generated'), { recursive: true });
writeFileSync(join(site, 'src/lib/generated/fingerprints.json'), JSON.stringify(fingerprints, null, '\t') + '\n');
