# Deploying

`./scripts/build-web.sh` produces `dist/web/`: a self-contained folder of
static files (index.html, gl.js, audio.js, the wasm binary, and the tileset)
with no external requests. It runs anywhere you can put a folder of files.

## This repo's GitHub Pages

CI builds `dist/web/` and publishes it to this repo's GitHub Pages on every
push to `main`. One-time setup: repo **Settings → Pages → Source: "GitHub
Actions"**. Releases also carry native binaries and a zipped web bundle.

## wirenook.net

The intended home is <https://wirenook.net/amoeba-rl/>, hosted from the
[cassidyprather.github.io](https://github.com/CassidyPrather/cassidyprather.github.io)
Hugo site. Hugo copies `static/` through to the published site verbatim, so
deployment is:

```sh
./scripts/build-web.sh
rm -rf ../cassidyprather.github.io/static/amoeba-rl
cp -r dist/web ../cassidyprather.github.io/static/amoeba-rl
```

then commit and push the site repo; its deploy workflow does the rest. All
asset references in `index.html` are relative, so the bundle works from any
subpath. Optionally add a `content/amoeba-rl-page.md` (or a blog post) on the
site linking to `/amoeba-rl/` — the game itself needs no Hugo page to run.

The game renders at any viewport size and supports touch, so linking it
directly on mobile is fine.
