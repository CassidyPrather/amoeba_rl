# Amoeba Roguelike Remastered

Play as a giant, constantly evolving amoeba and fight off intensifying waves
of humans trying to protect their cities. Engulf and digest them, craft new
organelles and cores from their remains, and destroy the city gates to escape
to the surface.

This is a Rust remaster of the original 7DRL entry, built on
[game-template](https://github.com/CassidyPrather/game-template)
(macroquad + wasm): the whole game is a pure, deterministic, unit-tested
library (`src/sim/`), and the frontend is a thin shell that draws it with the
original's CP437 tileset. It runs natively and in the browser — the intended
home is <https://wirenook.net/amoeba-rl/>.

The game explains itself. It opens with its own controls, relabels them for
whatever you are holding, describes anything you point the examine cursor at,
and draws every interaction in the cell it happens in — so none of that is
written down here. What is written down here is the part you cannot find out
by playing.

The original C# version (RogueSharp + RLNET) lives on in this repo's git
history (`main`, pre-port); [`docs/PORT_SPEC.md`](docs/PORT_SPEC.md) is the
complete spec the remaster was built from, including which of the original's
quirks were kept on purpose. Where the remaster departs from it, the code says
so at the point of departure — search for `DELIBERATE CHANGE`.

The 7DRL entry and the patches after it shipped as *Amoeba RL*. Enough has
changed since — another language, animation, a layout that fits a phone, a
rebalanced core, a cavern with streets in it and humans with somewhere to be —
that this version takes a name of its own and starts its own numbering at
1.0.0 rather than continuing the C# line. The rename is a cover and not a
move, though: the crate, the binary, the wasm file and the published URL are
all still `amoeba-rl`, so every link that already points at the game keeps
working.

## Development

Requires [Rust](https://rustup.rs/). Native builds on Linux also need ALSA's
development files (`libasound2-dev` on Debian/Ubuntu).

```bash
cargo run                                                     # play natively
cargo test                                                    # sim + frontend tests
cargo test -- --ignored                                       # whole-playthrough tests
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
