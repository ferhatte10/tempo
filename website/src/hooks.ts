import type { Reroute } from '@sveltejs/kit/hooks';
import { deLocalizeUrl } from '#lib/paraglide/runtime.js';

// /fr/install is the /install page in French.
export const reroute: Reroute = (request) => deLocalizeUrl(request.url).pathname;
