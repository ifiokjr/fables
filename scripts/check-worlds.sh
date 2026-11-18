#!/usr/bin/env bash
# Check everything the repository ships: every book's script, every drawing style and every
# puppet. `devenv shell -- check:worlds` runs it locally; the CI worlds job runs the same script.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# CI builds the release binary once and passes it in; locally every run goes through cargo so
# the check always uses the working tree's code.
if [[ -n "${FABLES:-}" ]]; then
	fables=("$FABLES")
else
	fables=(cargo run --release --quiet --)
fi

echo "books"
while IFS= read -r story; do
	"${fables[@]}" check "$story"
done < <(find worlds -name story.md | sort)

echo "styles"
while IFS= read -r style; do
	"${fables[@]}" style check "${style%% *}"
done < <("${fables[@]}" style list)

echo "props"
# Every prop a world can use (the built-in ones and its own) must read; `style check` above
# draws every built-in prop in a kid's and a grown-up's hand in each style.
while IFS= read -r world; do
	"${fables[@]}" --world "$world" prop list >/dev/null
done < <(find worlds -mindepth 1 -maxdepth 1 -type d | sort)

echo "puppets"
while IFS= read -r world; do
	"${fables[@]}" --world "$world" puppet check
done < <(find worlds -mindepth 1 -maxdepth 1 -type d | sort)
