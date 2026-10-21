# Cartoon experiment

Stinky redrawn as an authored cut-out puppet: every part is a hand-placed Bézier drawing (`src/stinky.rs`) hung on the unchanged fables rig (`fables::art::rig`), with lip-sync from `fables::video::lipsync`. Nothing in the main crate was changed.

```bash
devenv shell -- cargo run --release --manifest-path experiments/cartoon/Cargo.toml
```

Output goes to `out/experiments/cartoon/`:

| File | What it shows |
| --- | --- |
| `compare-now-vs-experiment.png` | The current puppet (tv style) beside the new one |
| `compare-page.png` | The current "This is Stanley." proof beside the new page |
| `model-sheet.png` | Happy, defiant (NOPE), disgusted and grin, front and three-quarter |
| `turnaround.png` | The turn from three-quarter left to three-quarter right |
| `mouth-chart.png` | The nine lip-sync shapes and the mood mouths |
| `rig-check.png` | Stances and a walk cycle with the skeleton drawn over the parts |
| `park.png`, `book-page.png` | The setting, and one composed page with the text card |
| `wave-and-walk.mp4`, `film-strip.png` | Seven seconds of walk, turn, wave and "Hello!" |

Pass piece names to render only some: `sheet`, `turnaround`, `mouths`, `rig`, `compare`, `park`, `page`, `film`. `compare-page.png` needs the book's tv proofs rendered first.
