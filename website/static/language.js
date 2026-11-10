// First visit of the home page: a browser whose first language is French
// opens the French site, any other language stays on English (the base).
// Only on "/", so /fr/ addresses and shared links keep their language; a
// language chosen in the site's menu wins (kept in this browser only, like
// the theme: no cookie). Runs before the page paints, so nothing flashes.
(function () {
	if (location.pathname !== '/') return;
	var chosen = null;
	try {
		chosen = localStorage.getItem('language');
	} catch (e) {}
	var first = (navigator.languages && navigator.languages[0]) || navigator.language || '';
	var wanted = chosen || (/^fr(-|$)/i.test(first) ? 'fr' : 'en');
	if (wanted === 'fr') location.replace('/fr/' + location.search + location.hash);
})();
