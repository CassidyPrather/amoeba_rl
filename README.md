# Amoeba RL

Play as a giant, constantly evolving amoeba and fight off intensifying waves
of humans trying to protect their cities. Engulf and digest them, craft new
organelles and cores from their remains, and destroy the city gates to escape
to the surface.

This is a Rust rewrite of the original 7DRL entry, built on
[game-template](https://github.com/CassidyPrather/game-template)
(macroquad + wasm): the whole game is a pure, deterministic, unit-tested
library (`src/sim/`), and the frontend is a thin shell that draws it with the
original's CP437 tileset. It runs natively and in the browser — the intended
home is <https://wirenook.net/amoeba-rl/>.

The original C# version (RogueSharp + RLNET) lives on in this repo's git
history (`main`, pre-port); [`docs/PORT_SPEC.md`](docs/PORT_SPEC.md) is the
complete spec the port was built from, including which of the original's
quirks were kept on purpose.

## How to play

Destroy the required number of city gates (`C`) by walking into them with
enough mass. You lose when your last nucleus dies.

When you move (not swap), you drag a path of organelles behind you — the
highlighted slime shows which tiles will be dragged. Enemies sealed in with no
walkable escape are engulfed and digested. Crafting materials upgrade the
organelles (or the nucleus you swap with) next to them.

Every interaction is drawn where it happens, so you can watch a turn instead of
reconstructing it from the log: a militia's blow swings at the organelle it
hits, a nucleus dodging into a neighbour leaves an arrow behind it, a catalyst
sparks across to whichever cytoplasm was spent growing it. A turn spends at
most about two thirds of a second showing itself however much happened in it,
and pressing anything cuts it short — so nothing ever waits on an animation.

### Controls

| Key | Action |
|---|---|
| Arrows | Move / steer the cursor or sidebar |
| Space or `.` | Wait |
| `A` / `D` | Previous / next nucleus |
| `Z` | Organelle browser |
| `X` | Examine mode |
| `Q` / `E` | Page the sidebar |
| `S` or `F2` | Settings |
| `F1` | Help |
| `M` | Mute |
| `R` | Restart (after the run ends) |

### Settings

`S` opens a panel over whatever is on screen. It holds the message log
(hidden gives its rows to the map), the animations, how fast they run — *slow*
for a first run, *fast* once you know what a maw does — and the sound. Arrows
choose a row, enter or left/right changes it, `S` or `Esc` closes.

Difficulty (Normal / Easy / GJ) is chosen on the title screen — the original's
`--easy` and `--gj` command-line flags, made playable in a browser.

On a phone: an on-screen pad and buttons appear, a tap next to your active
nucleus moves, and a tap in examine mode sends the cursor there.

## Development

Requires [Rust](https://rustup.rs/). Native builds on Linux also need ALSA's
development files (`libasound2-dev` on Debian/Ubuntu).

```bash
cargo run                                                     # play natively
cargo test                                                    # sim + frontend tests
cargo clippy --all-targets --all-features -- -D warnings      # lint
cargo clippy --target wasm32-unknown-unknown -- -D warnings   # lint, wasm
./scripts/build-web.sh                                        # wasm -> dist/web/
python3 -m http.server --directory dist/web 8080              # serve it
cargo bench --bench sim_bench -- --quick                      # bench the sim
```

The web build needs `rustup target add wasm32-unknown-unknown`, and uses
`wasm-opt` from [binaryen](https://github.com/WebAssembly/binaryen) when
installed. Deployment (GitHub Pages and wirenook.net) is described in
[`docs/DEPLOYING.md`](docs/DEPLOYING.md).

## Credits

Original game, design, and this port's blueprint: Cassidy Prather.
Extensive playtesting, design, and support on the original from JackNine;
further playtesting from Qu and Decinym.
Font: [libtcod](https://github.com/libtcod/libtcod)'s `terminal12x12_gs_ro.png`
(see [CREDITS.md](CREDITS.md) for all vendored assets).
Code is AGPL-3.0-or-later; see [LICENSE](LICENSE).
