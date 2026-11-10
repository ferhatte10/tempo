# tempo.ferhatsn.fr

The project's website: presentation, installation, documentation, privacy.
English at `/`, French at `/fr/`. Design rules: [DESIGN.md](DESIGN.md).

All rights reserved, unlike the application (GPL-3.0): see [LICENSE](LICENSE).

A static site (SvelteKit with `adapter-static`): every page is a plain file,
nothing runs on the server, no cookie, no analytics, no third-party request.

```sh
bun install
bun run dev       # http://localhost:5173
bun run check     # types + contrast of every colour pair, both themes
bun run build     # → build/, served as is by nginx
```

`scripts/assets.js` (run by dev, check and build) copies from the app's own
sources what the site shows: the icon (`data/icons`) and the screenshots
(`docs/screenshots`, made by `make screenshots` from made-up timers). Those
copies are not committed.

`src/lib/links.ts` holds `AVAILABLE`: while it is false the site says “coming
soon” and shows no install command and no link to the source.

## Container image

What the site runs on: the built pages in an unprivileged nginx (uid 101,
port 8080, read-only, only `/tmp` written). Built from the repository root,
because the site copies the app's files at build time:

```sh
docker build -f website/Dockerfile -t tempo-website .
docker run --rm -p 8080:8080 --read-only --tmpfs /tmp --cap-drop ALL tempo-website
```

`nginx/default.conf` sets the caching (pages always checked again, hashed
files for a year) and `nginx/security-headers.conf` the headers sent with
every answer. `.github/workflows/website.yml` builds, scans (Trivy), serves and
tests this image on each pull request, and publishes it to GHCR from `main`;
the cluster then deploys it by itself.
