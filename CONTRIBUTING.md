# Contributing

fables is a Rust project (edition 2024) built with the nightly pinned in
[`rust-toolchain.toml`](rust-toolchain.toml); `rustup` installs it on the first build.

```bash
cargo build --release
cargo run --release -- proofs worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md
```

## Checks

Run all four before you send a change:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
./scripts/check-worlds.sh
```

`check-worlds.sh` checks every book, style and puppet the repository ships. The devenv shell
provides `check:all` and `check:worlds` for the same commands, and CI runs them on Linux and
macOS.

## What not to commit

Rendered pages, videos, screenshots and other binaries never go into git: every committed byte
stays in the history. Attach that evidence to the pull request, or host it and link it from the
description.

## Pull requests

Open a pull request against `main` rather than pushing to it. Titles use the history's
conventional-commit prefixes (`feat(world): …`, `fix(video): …`, `chore(ci): …`), and one pull
request carries one coherent change. Say what the change does and how you checked it.

## Proposing a style, setting or world

Send a pull request that adds the folder, with a model sheet or a rendered page attached to the
description — not committed. A world starts from `fables world new "Name" --style cartoon`;
characters come from `fables character new <id> --name "…"`; a setting starts from
`fables setting new <name> --from park` into a world's own `settings/`, or a new built-in SVG
under [`settings/`](settings). A style is a folder in [`styles/`](styles): copy the closest one,
read what it offers with `fables style describe <name>`, and make sure `fables style check <name>`
passes. [`worlds/mumbleton/`](worlds/mumbleton) is the smallest complete example world, and the
example worlds double as tests, so any new world has to pass `fables check` and `check:worlds`.
