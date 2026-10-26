# Cartoon experiment

Where Stinky's authored cut-out puppet began. His parts now live in `worlds/kerfuffle/puppets/` and the drawing in `fables::art::puppet`, so this crate only arranges him (through the same `FigureState` the books use) for review sheets, one park setting, a composed page and a short film.

```bash
devenv shell -- cargo run --release --manifest-path experiments/cartoon/Cargo.toml
```

Output goes to `out/experiments/cartoon/`:

| File | What it shows |
| --- | --- |
| `compare-before-after.png` | The parametric puppet (tv style) beside the authored one |
| `compare-page.png` | The book's tv proof of "This is Stanley." beside the experiment's page |
| `model-sheet.png` | Happy, defiant (NOPE), disgusted and grin, front and three-quarter |
| `bogey-model-sheet.png`, `grubby-model-sheet.png`, `shouty-model-sheet.png`, `fibby-model-sheet.png`, `kids-lineup.png` | The redrawn kids' model sheets, drawn from the cast as the books draw them, and a lineup of all five |
| `mum-model-sheet.png`, `dad-model-sheet.png`, `parcel-model-sheet.png` | The grown-ups' model sheets, drawn from the cast as the books draw them; the lineup shows them beside the kids at stage sizes |
| `props-and-extras.png` | Every prop and extra from the kit, front and three-quarter |
| `turnaround.png` | The turn from three-quarter left to three-quarter right |
| `mouth-chart.png` | The nine lip-sync shapes and the mood mouths |
| `rig-check.png` | Stances and a walk cycle with the skeleton drawn over the parts |
| `park.png`, `book-page.png` | The setting, and one composed page with the text card |
| `wave-and-walk.mp4`, `film-strip.png` | Seven seconds of walk, turn, wave and "Hello!" |

Pass piece names to render only some: `sheet`, `preview`, `extras`, `turnaround`, `mouths`, `rig`, `compare`, `park`, `page`, `film`. `compare-page.png` needs the book's tv proofs rendered first.
