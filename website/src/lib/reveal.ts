/**
 * Shows an element with a fade and a short rise once it scrolls into view
 * (DESIGN.md, Motion; styles in app.css). Left alone: elements already on
 * screen when the page opens (never hide what is being read), reduced
 * motion, and browsers without IntersectionObserver.
 */
export function reveal(node: HTMLElement) {
	if (
		!('IntersectionObserver' in window) ||
		matchMedia('(prefers-reduced-motion: reduce)').matches ||
		node.getBoundingClientRect().top < innerHeight
	) {
		return;
	}
	node.dataset.reveal = 'pending';
	const observer = new IntersectionObserver(
		([entry]) => {
			if (entry?.isIntersecting) {
				node.dataset.reveal = 'shown';
				observer.disconnect();
			}
		},
		{ rootMargin: '0px 0px -8% 0px' }
	);
	observer.observe(node);
	return { destroy: () => observer.disconnect() };
}
