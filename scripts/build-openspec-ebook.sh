#!/usr/bin/env bash
#
# Renders the OpenSpec change under openspec/changes/ into a single EPUB and a
# single PDF, so the whole proposal can be read in Apple Books on a tablet.
#
#   ./scripts/build-openspec-ebook.sh [change-name]
#
# Mermaid diagrams are rendered with mermaid-cli, fetched through npx unless
# `mmdc` is already on PATH.
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

# Mermaid diagrams are rendered to PNG so the book shows the flowchart
# instead of its source. mermaid-cli drives a headless browser; reuse a
# locally installed Chrome when there is one so Puppeteer does not download
# its own.
export MERMAID_CACHE_DIR="$out/.mermaid-cache"
if [[ -n ${MERMAID_CLI:-} ]]; then
  export MERMAID_CLI
elif command -v mmdc >/dev/null 2>&1; then
  export MERMAID_CLI=mmdc
else
  export MERMAID_CLI="npx --yes @mermaid-js/mermaid-cli@11.17.0"
fi

if [[ -z ${PUPPETEER_EXECUTABLE_PATH:-} ]]; then
  for candidate in \
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
    "/Applications/Chromium.app/Contents/MacOS/Chromium"
  do
    if [[ -x $candidate ]]; then
      export PUPPETEER_EXECUTABLE_PATH=$candidate
      break
    fi
  done
fi
if [[ -z ${PUPPETEER_EXECUTABLE_PATH:-} ]]; then
  for candidate in google-chrome chromium chromium-browser; do
    if command -v "$candidate" >/dev/null 2>&1; then
      candidate_path=$(command -v "$candidate")
      export PUPPETEER_EXECUTABLE_PATH=$candidate_path
      break
    fi
  done
fi
if [[ -n ${PUPPETEER_EXECUTABLE_PATH:-} ]]; then
  export PUPPETEER_SKIP_DOWNLOAD=1
fi

pandoc "$combined" \
  --from=gfm --to=epub3 \
  --lua-filter="$root/scripts/mermaid.lua" \
  --toc --toc-depth=3 --split-level=1 \
  --css="$root/scripts/openspec-ebook.css" \
  --metadata title="$title" --metadata lang=en \
  -o "$out/$name.epub"

# A5 is within a few millimetres of the iPad mini display, so the PDF reads at
# roughly 100% zoom in portrait.
pandoc "$combined" \
  --from=gfm --to=pdf \
  --lua-filter="$root/scripts/mermaid.lua" \
  --pdf-engine=typst \
  --toc --toc-depth=3 \
  -V papersize=a5 -V fontsize=10pt -V margin.x=1.5cm -V margin.y=1.5cm \
  --metadata title="$title" --metadata lang=en \
  -o "$out/$name.pdf"

echo "wrote:"
ls -lh "$out/$name.epub" "$out/$name.pdf"
