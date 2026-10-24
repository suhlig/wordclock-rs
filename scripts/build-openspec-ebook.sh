#!/usr/bin/env bash
#
# Renders the OpenSpec change under openspec/changes/ into a single EPUB and a
# single PDF, so the whole proposal can be read in Apple Books on a tablet.
#
#   ./scripts/build-openspec-ebook.sh [change-name]
#
# Outputs land in dist/ and are git-ignored build artifacts.

set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
change=${1:-replace-fadecandy-with-rust}
change_dir="$root/openspec/changes/$change"
out="$root/dist"
title="Wordclock without Fadecandy"

# Reading order for the capability specs, mirroring proposal.md. Any spec
# directory not listed here is appended alphabetically at the end.
preferred_specs=(clock-rendering color-pipeline led-output display-simulator clock-daemon deployment)

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

combined="$tmp/combined.md"
: >"$combined"

# Append a document as a chapter: use our own H1, drop the file's own H1.
emit_chapter() {
  local heading=$1 file=$2
  printf '\n# %s\n\n' "$heading" >>"$combined"
  sed '1{/^# /d;}' "$file" >>"$combined"
  printf '\n' >>"$combined"
}

for doc in proposal design tasks; do
  emit_chapter "$(tr '[:lower:]' '[:upper:]' <<<"${doc:0:1}")${doc:1}" "$change_dir/$doc.md"
done

declare -a specs=()
for spec in "${preferred_specs[@]}"; do
  [[ -f "$change_dir/specs/$spec/spec.md" ]] && specs+=("$spec")
done
for spec in "$change_dir"/specs/*/spec.md; do
  [[ -f "$spec" ]] || continue
  name=$(basename "$(dirname "$spec")")
  [[ " ${specs[*]-} " == *" $name "* ]] || specs+=("$name")
done

for spec in "${specs[@]}"; do
  emit_chapter "Spec: $spec" "$change_dir/specs/$spec/spec.md"
done

mkdir -p "$out"
name=$(tr '[:upper:]' '[:lower:]' <<<"$change")

pandoc "$combined" \
  --from=gfm --to=epub3 \
  --toc --toc-depth=3 --split-level=1 \
  --css="$root/scripts/openspec-ebook.css" \
  --metadata title="$title" --metadata lang=en \
  -o "$out/$name.epub"

# A5 is within a few millimetres of the iPad mini display, so the PDF reads at
# roughly 100% zoom in portrait.
pandoc "$combined" \
  --from=gfm --to=pdf \
  --pdf-engine=typst \
  --toc --toc-depth=3 \
  -V papersize=a5 -V fontsize=10pt -V margin.x=1.5cm -V margin.y=1.5cm \
  --metadata title="$title" --metadata lang=en \
  -o "$out/$name.pdf"

echo "wrote:"
ls -lh "$out/$name.epub" "$out/$name.pdf"
