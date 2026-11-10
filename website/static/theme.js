// Applies the theme before the first paint: the one chosen with the toggle,
// else the system's. Storage may be refused (strict private browsing): the
// system's theme then applies.
(function () {
	var saved = null;
	try {
		saved = localStorage.getItem('theme');
	} catch (e) {}
	var dark =
		saved === 'dark' ||
		(saved !== 'light' && window.matchMedia('(prefers-color-scheme: dark)').matches);
	document.documentElement.dataset.theme = dark ? 'dark' : 'light';
})();
