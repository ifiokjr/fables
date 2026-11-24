#!/usr/bin/env python3
"""Measure how fast fables builds a book and how many bytes it writes.

    scripts/bench.py                      # the quick cases, three runs each
    scripts/bench.py --full               # also the full-size 1080p video
    scripts/bench.py --runs 5 --json out/bench.json

Every case runs the release binary on a copy of one example book. The copy's voices are swapped
for a stand-in that writes a tone as long as the line would take to read, so the video's timing
is realistic and the numbers do not depend on `say`, an API key or the network. Takes are
recorded once, before anything is timed, the way a second build finds them cached.

For each case it prints the median wall time, the median CPU time (user + system, ffmpeg
included), the largest peak memory of any process in the run, and the bytes the case wrote.
Only the standard library is used, so it runs anywhere Python 3.9+ does.
"""

import argparse
import json
import math
import os
import platform
import re
import resource
import shutil
import statistics
import struct
import subprocess
import sys
import tempfile
import time
import wave
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RATE = 48_000
# Characters a second of the stand-in voice: the rate fables itself estimates with.
CHARACTERS_A_SECOND = 16.0

QUICK = [
    ("check", ["check"]),
    ("proofs", ["proofs"]),
    ("storyboard", ["storyboard"]),
    ("ebook", ["ebook"]),
    ("print", ["print"]),
    ("animatic", ["animatic"]),
    ("video-preview", ["video", "--preview", "--format", "landscape"]),
]
FULL = [("video-1080p", ["video", "--format", "landscape"])]


def voice(out: str) -> None:
    """The stand-in voice: a soft 220 Hz tone as long as the text on stdin takes to read."""
    text = sys.stdin.read()
    seconds = max(len(text) / CHARACTERS_A_SECOND, 0.3)
    frames = int(seconds * RATE)
    tone = (
        int(8000 * math.sin(2 * math.pi * 220 * i / RATE)) for i in range(frames)
    )

    with wave.open(out, "wb") as file:
        file.setnchannels(1)
        file.setsampwidth(2)
        file.setframerate(RATE)
        file.writeframes(b"".join(struct.pack("<h", sample) for sample in tone))


def stand_in_voices(world: Path) -> None:
    """Points the narrator and every character voice in a world copy at [`voice`]."""
    command = json.dumps([sys.executable, str(Path(__file__).resolve()), "voice", "{out}"])
    header = re.compile(r"^\[(narrator|[^\]]+\.voice)\]\s*$")

    for name in ("world.toml", "cast.toml"):
        path = world / name

        if not path.is_file():
            continue

        lines = []

        for line in path.read_text().splitlines():
            if line.strip().startswith("provider") and lines and lines[-1].startswith("command ="):
                continue  # the stand-in already names its provider

            lines.append(line)

            if header.match(line):
                lines.append('provider = "command"')
                lines.append(f"command = {command}")

        if name == "world.toml" and not any(header.match(line) for line in lines):
            # A world without a narrator reads with the default voice; give it the stand-in.
            lines += ["", "[narrator]", 'provider = "command"', f"command = {command}"]

        path.write_text("\n".join(lines) + "\n")


def size_of(folder: Path) -> int:
    """Bytes of every file under `folder`, leaving out the recorded takes."""
    return sum(
        path.stat().st_size
        for path in folder.rglob("*")
        if path.is_file() and "cache" not in path.parts
    )


def measure(command: list) -> None:
    """Runs `command` and prints its wall seconds, CPU seconds (with its children) and peak
    memory as JSON. It runs in a process of its own, because a process's record of its
    children's peak memory only ever grows."""
    started = time.perf_counter()
    result = subprocess.run(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    wall = time.perf_counter() - started
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)

    if result.returncode != 0:
        sys.exit(f"{' '.join(command)} failed:\n{result.stderr.decode()[-3000:]}")

    # ru_maxrss is the largest single child, in KiB on Linux and bytes on macOS.
    peak = usage.ru_maxrss * (1 if platform.system() == "Darwin" else 1024)
    print(json.dumps({"wall": wall, "cpu": usage.ru_utime + usage.ru_stime, "peak": peak}))


def run(command: list) -> dict:
    """Runs `command` once under [`measure`]."""
    output = subprocess.run(
        [sys.executable, str(Path(__file__).resolve()), "measure", *map(str, command)],
        stdout=subprocess.PIPE,
        check=True,
    )

    return json.loads(output.stdout)


def main() -> None:
    if len(sys.argv) == 3 and sys.argv[1] == "voice":
        voice(sys.argv[2])
        return

    if len(sys.argv) > 2 and sys.argv[1] == "measure":
        measure(sys.argv[2:])
        return

    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--story", default="worlds/kerfuffle/stories/stinky-mcstinkbreath/story.md")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--full", action="store_true", help="also render the 1080p video")
    parser.add_argument("--only", help="comma-separated case names")
    parser.add_argument("--json", help="also write the results here")
    parser.add_argument("--fables", default=os.environ.get("FABLES"), help="binary to measure")
    args = parser.parse_args()

    fables = args.fables

    if not fables:
        subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=ROOT, check=True)
        fables = str(ROOT / "target" / "release" / "fables")

    story = (ROOT / args.story).resolve()
    world = next(p for p in story.parents if (p / "world.toml").is_file())
    cases = QUICK + (FULL if args.full else [])

    if args.only:
        wanted = args.only.split(",")
        cases = [case for case in cases if case[0] in wanted]

    results = {
        "binary_bytes": Path(fables).stat().st_size,
        "machine": f"{platform.system()} {platform.machine()}, {os.cpu_count()} cores",
        "story": args.story,
        "cases": {},
    }

    with tempfile.TemporaryDirectory(prefix="fables-bench-") as scratch:
        scratch = Path(scratch)
        copy = scratch / "worlds" / world.name
        shutil.copytree(world, copy)
        stand_in_voices(copy)
        script = copy / story.relative_to(world)

        print(f"{'case':<15} {'wall s':>8} {'cpu s':>8} {'peak MB':>8} {'out MB':>9}")

        for name, command in cases:
            out = scratch / "out" / name
            base = [fables, *command, "--out", str(out), str(script)]

            if name == "check":
                base = [fables, "check", str(script)]
            elif command[0] in ("video", "animatic"):
                run(base)  # records every take once, untimed

            runs = [run(base) for _ in range(args.runs)]
            case = {
                "wall": statistics.median(r["wall"] for r in runs),
                "cpu": statistics.median(r["cpu"] for r in runs),
                "peak": max(r["peak"] for r in runs),
                "bytes": size_of(out) if out.exists() else 0,
            }
            results["cases"][name] = case
            print(
                f"{name:<15} {case['wall']:>8.2f} {case['cpu']:>8.2f} "
                f"{case['peak'] / 1e6:>8.0f} {case['bytes'] / 1e6:>9.2f}",
                flush=True,
            )

    print(f"binary: {results['binary_bytes'] / 1e6:.1f} MB")

    if args.json:
        Path(args.json).parent.mkdir(parents=True, exist_ok=True)
        Path(args.json).write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
