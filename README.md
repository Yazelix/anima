# Yazelix Screen

Standalone terminal screen animations from Yazelix

The user-facing command is `yzs`

```bash
nix run github:luccahuguet/yazelix-screen#yzs
nix run github:luccahuguet/yazelix-screen#yzs -- static
nix run github:luccahuguet/yazelix-screen#yzs -- mandelbrot
nix run github:luccahuguet/yazelix-screen#yzs -- game_of_life_bloom --cell-style dotted
nix run github:luccahuguet/yazelix-screen#yzs -- random --duration-seconds 3
```

## What It Contains

- Animation engines for Boids, Mandelbrot, and Game of Life
- Static and logo-style Yazelix welcome screens
- File-backed Kitty PNG frame sequence rendering
- Frame production through `ScreenFrameProducer`
- Terminal sizing helpers and alternate-screen rendering helpers
- A standalone `yzs` binary with interactive and timed playback
- Small examples for library consumers

## User Command

Installed standalone command:

```bash
yzs --help
yzs
yzs static
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
cargo run --bin yzs -- mandelbrot
cargo run --bin yzs -- game_of_life_bloom --cell-style dotted
cargo run --bin yzs -- random --duration-seconds 3
```

With Nix:

```bash
nix build .#yzs
nix run .#yzs -- --help
nix run .#yzs -- static
nix run .#yzs -- mandelbrot
nix run .#yzs -- random --duration-seconds 3
```

Supported styles:

- `static`
- `logo`
- `boids`
- `boids_predator`
- `boids_schools`
- `mandelbrot`
- `game_of_life_gliders`
- `game_of_life_oscillators`
- `game_of_life_bloom`
- `random`

No style means `random`

Random chooses from all supported styles except `static`; use `static`
explicitly for the static welcome card

## Library Examples

Render one frame without alternate-screen mode:

```bash
cargo run --example render_once
```

Play a style for a bounded number of frames:

```bash
cargo run --example play_style -- mandelbrot 90
cargo run --example play_style -- boids_schools 120
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

- Product/repository: `yazelix-screen`
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
