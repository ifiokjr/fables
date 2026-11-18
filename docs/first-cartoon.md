# Your first cartoon in five minutes

From an empty folder to a rendered book, a storyboard and a preview video, with nothing but
the command line. Install fables once with `cargo install --path .`.

```bash
mkdir my-cartoon
cd my-cartoon
fables world new "Teacup Lane" --style chibi --author "Your Name"
```

That writes `worlds/teacup-lane/` with a `world.toml`, an empty `cast.toml` holding a
commented example, and `stories/`. The world's **drawing style** is `chibi`: every character in
it is built the way that style builds figures. Its page skin is `riso` until a book says
otherwise.

```bash
fables character new pip --name "Pip Pocket" --skin tangerine --hair-colour plum --top teal --trim cream --bottom night --shoes ink --eyes cocoa
fables character new mum --name "Mum" --kind grown-up --skin cocoa --hair-colour ink --top tomato --trim cream --bottom night --shoes cocoa --eyes cocoa --draw
```

Pip has no puppet file, so they are drawn straight from the chibi template in their own
colours. `--draw` wrote Mum's parts to `worlds/teacup-lane/puppets/mum.svg` so they can be
redrawn: `fables puppet parts mum` prints them, and `fables puppet set mum <part> --d "M…"`
writes one back, checked before it lands. Both are drawn in the world's style either way.

```bash
mkdir -p worlds/teacup-lane/stories/pip-and-the-puddle
cat > worlds/teacup-lane/stories/pip-and-the-puddle/story.md <<'EOF'
+++
title = "Pip and the Puddle"
star = "pip"
moral = "A puddle is only deep until you step in it."
+++

# garden golden
@pip x=0.4 worried
Pip stopped at the puddle. It looked very deep.

# garden
@pip x=0.5 grin jump
Pip jumped. **SPLASH!** The puddle was only a puddle.

# garden
@mum x=0.72 happy
@pip x=0.35 proud hold holds=bucket
"Again!" said Pip, and held up a bucket of puddle.
@scene dusk
The sun went down on a very wet, very happy Pip.
EOF
```

The story has no `author`; the world's `author = "Your Name"` credits it. Bare `golden` and
`dusk` are lights: `# garden golden` is `# garden light=golden`, and `@scene dusk` ends the day.
`holds=bucket` puts a prop in Pip's hand; `fables prop list` names every one a world can use.

```bash
fables check worlds/teacup-lane/stories/pip-and-the-puddle/story.md
fables proofs worlds/teacup-lane/stories/pip-and-the-puddle/story.md
fables storyboard worlds/teacup-lane/stories/pip-and-the-puddle/story.md --format landscape
fables video worlds/teacup-lane/stories/pip-and-the-puddle/story.md --preview --format landscape
```

Open `out/pip-and-the-puddle/riso/proofs/contact-sheet.png` for every page at once, and
`out/pip-and-the-puddle/riso/storyboard/storyboard.png` for the video's camera plan. The
preview video is in `out/pip-and-the-puddle/riso/video/`: half size at 15 fps.

The storyboard also prints its shot list (and writes it beside the board as `storyboard.md`):
read it for each shot's page, script line, size and timing. To change a page's camera, pin it
with `--pin page=2`, edit the `@camera … // pinned` lines it writes into the script, and run
`fables animatic` to judge the timing in seconds; `--unpin` takes the pinned lines out again.

Next: `fables style describe chibi` lists every part a figure is drawn with; `fables setting
new harbour --from park` gives the world a place of its own; and `fables book <story>` makes
the ePub and print files when the book looks right.
