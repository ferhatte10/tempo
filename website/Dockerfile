# tempo.ferhatsn.fr: the static site, served by an unprivileged nginx.
# Built from the repository root, because the site copies the app's icon and
# screenshots at build time:
#
#   docker build -f website/Dockerfile -t tempo-website .
#
# Base images pinned by digest (update both the tag and the digest together).

FROM oven/bun:1.4.4-debian@sha256:93d5032fe06d163427822dc1598f420363759bb18155c9cc90eb1be05a64a19e AS build
WORKDIR /src
# What scripts/assets.js reads from the app.
COPY data/icons/ data/icons/
COPY docs/screenshots/ docs/screenshots/
COPY website/package.json website/bun.lock website/
RUN cd website && bun install --frozen-lockfile
COPY website/ website/
RUN cd website && bun run check && bun run build

FROM nginxinc/nginx-unprivileged:1.31-alpine@sha256:b5047b9b76845eac124358373c6852377c52e4b05834a47647e955dd91cb8f1e
# Security fixes released since the base image, and no curl: nothing here
# needs it.
USER root
RUN apk upgrade --no-cache && apk del --no-cache curl
USER 101
COPY website/nginx/default.conf /etc/nginx/conf.d/default.conf
COPY website/nginx/security-headers.conf /etc/nginx/security-headers.conf
COPY --from=build /src/website/build /usr/share/nginx/html
# nginx-unprivileged runs as uid 101 and listens on 8080; only /tmp is written.
EXPOSE 8080
