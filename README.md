# Anima

Anima is a standalone terminal animation toolkit from Yazelix. It works in any
capable terminal; no Yazelix installation is required.

![Primordial particles, a Mandelbrot dive, Matrix rain, and Game of Life tumblers in Anima](assets/anima.gif)

[See every animation](#animation-gallery).

The user-facing command is `yzs`

```bash
nix run github:Yazelix/anima#yzs
nix run github:Yazelix/anima#yzs -- static
nix run github:Yazelix/anima#yzs -- asciiquarium --duration-seconds 3
nix run github:Yazelix/anima#yzs -- mandelbrot
nix run github:Yazelix/anima#yzs -- friends_and_enemies
nix run github:Yazelix/anima#yzs -- primordial
nix run github:Yazelix/anima#yzs -- physarum
nix run github:Yazelix/anima#yzs -- chladni
nix run github:Yazelix/anima#yzs -- game_of_life_tumblers --cell-style dotted
nix run github:Yazelix/anima#yzs -- random --duration-seconds 3
```

## What It Contains

- Animation engines for Boids, friends and enemies, Primordial Particle Systems, Physarum trail networks, Chladni nodal patterns, Mandelbrot, Matrix rain, and Game of Life
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
- [Thomas Schmickl, Martin Stefanec, and Karl Crailsheim](https://www.nature.com/articles/srep37969),
  who introduced the Primordial Particle System motion law. It inspires Anima's
  Primordial animation.
- [Jeff Jones](https://doi.org/10.1162/artl.2010.16.2.16202), whose particle model
  of Physarum transport networks inspires Anima's trail-network animation.
- [Paul Bourke](https://www.paulbourke.net/geometry/chladni/), who documents the
  Chladni nodal equations used in Anima's geometric animation.

## User Command

Installed standalone command:

```bash
yzs --help
yzs
yzs static
yzs asciiquarium --duration-seconds 3
yzs friends_and_enemies --duration-seconds 3
yzs primordial --duration-seconds 3
yzs physarum --duration-seconds 3
yzs chladni --duration-seconds 3
yzs mandelbrot
yzs game_of_life_tumblers --cell-style dotted
yzs random --duration-seconds 3
```

Nova users get the integrated animation surface through the main command:

```bash
yzx anima
yzx anima chladni
```

## Repository Usage

From this repository:

```bash
cargo run --bin yzs -- --help
cargo run --bin yzs -- static
cargo run --bin yzs -- asciiquarium --duration-seconds 3
cargo run --bin yzs -- friends_and_enemies --duration-seconds 3
cargo run --bin yzs -- primordial --duration-seconds 3
cargo run --bin yzs -- physarum --duration-seconds 3
cargo run --bin yzs -- chladni --duration-seconds 3
cargo run --bin yzs -- mandelbrot
cargo run --bin yzs -- game_of_life_tumblers --cell-style dotted
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
nix run .#yzs -- primordial --duration-seconds 3
nix run .#yzs -- physarum --duration-seconds 3
nix run .#yzs -- chladni --duration-seconds 3
nix run .#yzs -- mandelbrot
nix run .#yzs -- random --duration-seconds 3
```

The [gallery](#animation-gallery) shows every distinct animation. `boids` is an
alias of `boids_predator`. `static` shows a still welcome card; `random` chooses
an animation. No style means `random`

In native animations, `Left`/`h`/`p` selects the previous style and
`Right`/`l`/`n` selects the next; any other key exits

Random chooses from the dogfooded animated styles. `static`, `logo`, and
`friends_and_enemies` remain explicitly selectable but outside that pool.
`physarum` and `chladni` are available by name and through native browsing;
they stay outside random selection pending integrated Nova dogfooding

In `physarum`, agents follow and deposit trails that diffuse into branching
networks. The animation uses truecolor half blocks, a 6,000-agent ceiling, and
the shared duration, input, and resize handling. It adds no dependency. This is
a visual approximation of Jones's model, without biological-fidelity claims

In `chladni`, five standing-wave mode pairs blend through a 900-frame loop
(36 seconds of frame delays, plus rendering and terminal I/O time).
Warm nodal lines separate blue and violet regions on an opaque background.
The animation uses square half-block samples under the 2:1 terminal-cell
convention, cached cosine terms, and a fixed palette of at most 64 colors.
It is a visual approximation of plate patterns, not an acoustic simulation

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

## Animation Gallery

Each preview is a short looping excerpt, not a complete simulation cycle.
Run the command above a GIF to watch that style in your terminal.

### Logo

`yzs logo`

![Yazelix welcome card cycling through its title and colored text](assets/animations/logo.gif)

### Aquarium

`yzs asciiquarium`

![ASCII fish swimming past seaweed and rising bubbles](assets/animations/asciiquarium.gif)

### Boids: Predator

`yzs boids_predator` (alias: `yzs boids`)

![Colored flocks scattering around a pursuing predator](assets/animations/boids_predator.gif)

### Boids: Schools

`yzs boids_schools`

![Schools of colored particles turning and swimming together](assets/animations/boids_schools.gif)

### Friends and Enemies

`yzs friends_and_enemies`

![Dense colored particles chasing friends and avoiding enemies](assets/animations/friends_and_enemies.gif)

### Primordial

`yzs primordial`

![Bright green particles moving across a dark field](assets/animations/primordial.gif)

### Physarum

`yzs physarum`

![Glowing trails joining into a branching transport network](assets/animations/physarum.gif)

### Chladni

`yzs chladni`

![Warm nodal lines shifting between blue and violet standing-wave patterns](assets/animations/chladni.gif)

### Mandelbrot

`yzs mandelbrot`

![A colorful zoom into the branching edge of the Mandelbrot set](assets/animations/mandelbrot.gif)

### Matrix

`yzs matrix`

![Green columns of glowing characters falling across the terminal](assets/animations/matrix.gif)

### Game of Life: Gliders

`yzs game_of_life_gliders`

![Small colored gliders traveling diagonally across a dark field](assets/animations/game_of_life_gliders.gif)

### Game of Life: Tumblers

`yzs game_of_life_tumblers`

![Rows of colored block patterns oscillating in Conway's Game of Life](assets/animations/game_of_life_tumblers.gif)

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
cargo run --example play_style -- primordial 180
cargo run --example play_style -- physarum 180
cargo run --example play_style -- chladni 180
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

Nova consumes this crate for integrated rendering. `yzx anima` is the integrated Nova command; `yzs` is the standalone command for terminal users who want only the screen animations

## Surfaces

- Product/repository: `anima`
- Command: `yzs`
- Rust crate: `yazelix_screen`
- Integrated Nova command: `yzx anima`

## Release Policy

External releases use SemVer. Breaking changes to frame producer traits, style names, terminal-mode helpers, or cell-style parsing require a major version bump

Component tags should use:

```text
v0.1.0
```

## Verification

The gallery and hero montage are recorded from the local `yzs` package through
[Kinestra](https://github.com/Yazelix/kinestra) and a pinned Mars terminal:

```sh
nix run .#record-demo
```

Run this from the repository root on x86_64 Linux. The recipe exports a
two-second excerpt per style after a four-second warm-up (15 seconds for
Physarum). Each `assets/animations/STYLE.gif` is 640 × 360 at 10 FPS with a
64-color palette. Capture headroom keeps shutdown frames out of the excerpts.
Live recordings vary between takes.
The eight-second hero montage reuses two seconds each of Primordial, Mandelbrot,
Matrix, and tumblers. [A still preview](assets/anima-poster.png) is also available.

The Rust recipe is [demo/record.rs](demo/record.rs); appearance settings live in
`demo/mars/`. It checks its inventory against packaged CLI help and prints each
GIF's byte size and the gallery total. When adding an animation, update the
recipe and gallery together, inspect the motion and framing, and review asset
sizes before committing. `boids` shares the predator GIF; `static` and `random`
do not need separate previews.

Nix compiles the recipe against the pinned Kinestra library. Intermediate MP4s
stay in ignored `demo/.work/`. Recording tools and gallery assets stay outside
the installed `yzs` closure. Kinestra and Mars revisions are pinned in `flake.lock`.

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
