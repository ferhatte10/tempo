import fingerprints from '#lib/generated/fingerprints.json';

/**
 * A screenshot or film of static/ with its content fingerprint
 * (scripts/assets.js): a new version gets a new address, so no browser keeps
 * showing the old one, and nginx can let browsers cache these for a year.
 */
export function versioned(path: string): string {
	const fingerprint = (fingerprints as Record<string, string>)[path];
	return fingerprint ? `${path}?v=${fingerprint}` : path;
}
