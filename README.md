# Anima

Anima is a standalone terminal animation toolkit from Yazelix. It works in any
capable terminal; no Yazelix installation is required.

The user-facing command is `yzs`

```bash
nix run github:Yazelix/anima#yzs
nix run github:Yazelix/anima#yzs -- static
nix run github:Yazelix/anima#yzs -- asciiquarium --duration-seconds 3
nix run github:Yazelix/anima#yzs -- mandelbrot
nix run github:Yazelix/anima#yzs -- friends_and_enemies
nix run github:Yazelix/anima#yzs -- game_of_life_bloom --cell-style dotted
nix run github:Yazelix/anima#yzs -- random --duration-seconds 3
```

## What It Contains

- Animation engines for Boids, friends and enemies, Mandelbrot, Matrix rain, and Game of Life
- The separately packaged `asciiquarium-rs` terminal aquarium
- Static and logo-style Yazelix welcome screens
- File-backed Kitty PNG frame sequence rendering
- Frame production through `ScreenFrameProducer`
- Terminal sizing helpers and alternate-screen rendering helpers
- A standalone `yzs` binary with interactive and timed playback
- Small examples for library consumers

## Special Thanks

Special thanks to:

- [Craig Reynolds](https://www.red3d.com/cwr/), who created
  [Boids](https://www.red3d.com/cwr/boids/) in 1986. Its separation, alignment,
  and cohesion rules inspire Anima's Boids animations.
- [John Horton Conway](https://mathshistory.st-andrews.ac.uk/Biographies/Conway/),
  who invented the Game of Life in 1970. His cellular automaton inspires
  Anima's Game of Life animations.
- [Simon Woods](https://community.wolfram.com/groups/-/m/t/122095), who
  published the friends-and-enemies particle dance. Its update rule inspires
  Anima's dense particle animation.
- [Benoît Mandelbrot](https://news.yale.edu/2010/10/18/memoriam-benoit-mandelbrot),
  whose pioneering fractal work led to the Mandelbrot set. It inspires Anima's
  Mandelbrot animation.
- [Simon Whiteley](https://www.wired.com/story/the-matrix-code-sushi-recipe/),
  who designed the digital rain for *The Matrix*. It inspires Anima's Matrix
  animation.

## User Command

Installed standalone command:

```bash
yzs --help
yzs
yzs static
yzs asciiquarium --duration-seconds 3
yzs friends_and_enemies --duration-seconds 3
yzs mandelbrot
yzs game_of_life_bloom --cell-style dotted
yzs random --duration-seconds 3
```

Yazelix users get the integrated screen surface through the main command:

```bash
yzx screen
yzx screen mandelbrot
```

## Repository Usage

From this repository:

```bash
cargo run --bin yzs -- --help
cargo run --bin yzs -- static
cargo run --bin yzs -- asciiquarium --duration-seconds 3
cargo run --bin yzs -- friends_and_enemies --duration-seconds 3
cargo run --bin yzs -- mandelbrot
cargo run --bin yzs -- game_of_life_bloom --cell-style dotted
cargo run --bin yzs -- random --duration-seconds 3
```

Source-only Cargo runs resolve `asciiquarium-rs` from `PATH`; Nix runs use the
pinned upstream executable

With Nix:

```bash
nix build .#yzs
nix run .#yzs -- --help
nix run .#yzs -- static
nix run .#yzs -- asciiquarium --duration-seconds 3
nix run .#yzs -- friends_and_enemies --duration-seconds 3
nix run .#yzs -- mandelbrot
nix run .#yzs -- random --duration-seconds 3
```

Supported styles:

- `static`
- `logo`
- `asciiquarium`
- `boids`
- `boids_predator`
- `boids_schools`
- `friends_and_enemies`
- `mandelbrot`
- `matrix`
- `game_of_life_gliders`
- `game_of_life_oscillators`
- `game_of_life_bloom`
- `random`

No style means `random`

In native animations, `Left`/`h`/`p` selects the previous style and
`Right`/`l`/`n` selects the next; any other key exits

Random chooses from the dogfooded animated styles. `static`, `logo`, and
`friends_and_enemies` remain explicitly selectable but outside that pool

The aquarium runs as a separate
[`asciiquarium-rs`](https://github.com/cablehead/asciiquarium-rs) process under
its GPL-2.0-or-later license. Its upstream [credit and
lineage](https://github.com/cablehead/asciiquarium-rs#credit-and-lineage) section
traces it to Kirk Baucom's original Perl program, Joan Stark's ASCII art,
Claudio Matsuoka's additions, and `cablehead`'s Rust port. `yzs` supplies the same
any-key exit and optional duration contract used by its native styles without
copying or linking the aquarium implementation. The packaged upstream revision
exits cleanly when its terminal disappears, so closing the containing terminal
cannot orphan it

## Library Examples

Render one frame without alternate-screen mode:

```bash
cargo run --example render_once
```

Play a style for a bounded number of frames:

```bash
cargo run --example play_style -- mandelbrot 90
cargo run --example play_style -- matrix 90
cargo run --example play_style -- boids_schools 120
cargo run --example play_style -- friends_and_enemies 90
cargo run --example play_style -- game_of_life_gliders 80
```

The second argument is the frame count. The examples use only `yazelix_screen` APIs and standard Rust APIs

## Boundary With Yazelix

`yazelix_screen` owns reusable animation and terminal-rendering primitives, including standalone Yazelix-branded screen styles. Integrated welcome/session policy stays outside the crate

The crate must not depend on:

- `yazelix_core`
- `settings.jsonc`
- generated Yazelix config or state
- Zellij session state
- Home Manager install state
- Yazelix command palette or workspace orchestration

Yazelix consumes this crate for integrated rendering. `yzx screen` is the integrated Yazelix command; `yzs` is the standalone command for terminal users who want only the screen animations

## Surfaces

- Product/repository: `anima`
- Command: `yzs`
- Rust crate: `yazelix_screen`
- Integrated Yazelix command: `yzx screen`

## Release Policy

External releases use SemVer. Breaking changes to frame producer traits, style names, terminal-mode helpers, or cell-style parsing require a major version bump

Component tags should use:

```text
v0.1.0
```

## Verification

From this repository:

```bash
cargo fmt --all -- --check
cargo check --examples
cargo test
cargo run --bin yzs -- --help
cargo run --example render_once
nix build .#yzs
nix run .#yzs -- --help
```
