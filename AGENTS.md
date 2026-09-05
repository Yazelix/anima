# Agent Guidelines

Shared Yazelix agent workflow and release policy live in the main repo:

- https://github.com/Yazelix/nova/blob/main/AGENTS.md
- In sibling local checkouts, read `../yazelix/AGENTS.md` first

Only Anima-specific guidance belongs here.

## Local Scope

- This repo owns Anima's standalone `anima` terminal animation engine and Rust crate.
- Nova owns integrated welcome/session policy and the `yzx anima` command surface.
- Keep renderer behavior usable from a plain terminal without a Yazelix session.

## Local Commands

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo check --examples`
- `cargo run --bin anima -- --help`
- `cargo run --example render_once`
- `nix build .#anima --no-link`

## Integration Notes

Main Yazelix consumes the package and Rust crate through pinned child revisions. Publish child changes before updating main locks for coupled runtime work.
