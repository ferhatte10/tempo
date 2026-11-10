// Every page is built once as a plain file (adapter-static).
export const prerender = true;
// /install/ → install/index.html: plain folders nginx serves as they are.
export const trailingSlash = 'always';
