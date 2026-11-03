#!/usr/bin/env bash
#
# Renders the OpenSpec changes under openspec/changes/ into a single EPUB and a
# single PDF, so the whole proposal can be read in Apple Books on a tablet.
#
#   ./scripts/build-openspec-ebook.sh
#
# It always produces one book. Every change that has a proposal is included, the
# primary one first and the rest alphabetically, so a new change joins the book
# without editing this file. There is deliberately no way to render a subset:
# a second book with the same cover and a different scope is a trap, not a
# feature.
#
# The book opens with a cover page built from docs/images/wordclock-front.jpg,
# then the OpenSpec artifacts, then the project documentation (README, hardware
# guide, ESP32-C3 audit, OTA spike, strip colour fault, metrics contract,
# handoff) as reference material.
#
# Mermaid diagrams are rendered with mermaid-cli, fetched through npx unless
# `mmdc` is already on PATH.
#
# Outputs land in dist/ and are git-ignored build artifacts.

set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# Typst resolves a relative image path against the working directory, so build
# from the repository root even when the script is called from elsewhere.
cd "$root"
primary=replace-fadecandy-with-rust
out="$root/dist"
title="Wordclock without Fadecandy"
subtitle="Replacing Fadecandy with a Rust implementation"
cover="$root/docs/images/wordclock-front.jpg"
cover_rel="docs/images/wordclock-front.jpg"

# Every change with a proposal, primary first and the rest alphabetically. The
# primary leads because the book is named after it and it carries the capability
# specs the others build on. The primary is resolved from the active changes
# first and then from the archive, so archiving it does not silently drop it
# from the book and leave a stale title.
change_dir_for() {
  local slug=$1 dir
  for dir in "$root/openspec/changes/$slug" "$root"/openspec/changes/archive/*-"$slug"; do
    if [[ -f $dir/proposal.md ]]; then
      printf '%s\n' "$dir"
      return 0
    fi
  done
  return 1
}

changes=()
if change_dir_for "$primary" >/dev/null; then
  changes+=("$primary")
fi
for dir in "$root"/openspec/changes/*/; do
  slug=$(basename "${dir%/}")
  [[ $slug == "$primary" || $slug == archive ]] && continue
  [[ -f $dir/proposal.md ]] || continue
  changes+=("$slug")
done

if [[ ${#changes[@]} -eq 0 ]]; then
  echo "no OpenSpec change with a proposal.md under openspec/changes" >&2
  exit 1
fi

if [[ ! -f $cover ]]; then
  echo "missing cover image: $cover" >&2
  exit 1
fi

# Reading order for the capability specs, mirroring proposal.md. Any spec
# directory not listed here is appended alphabetically at the end.
preferred_specs=(clock-rendering color-pipeline led-output display-simulator clock-daemon deployment)

# `add-power-budget-and-current-sensing` becomes
# `Add Power Budget And Current Sensing`, for use as a chapter prefix.
change_title() {
  tr '-' ' ' <<<"$1" | awk '{ for (i = 1; i <= NF; i++) $i = toupper(substr($i, 1, 1)) substr($i, 2); print }'
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

combined="$tmp/combined.md"
: >"$combined"

# Append a document as a chapter: use our own H1, drop the file's own H1.
emit_chapter() {
  local heading=$1 file=$2
  if [[ ! -f $file ]]; then
    echo "missing chapter source: $file" >&2
    exit 1
  fi
  printf '\n```{=typst}\n#pagebreak(weak: true)\n```\n\n# %s\n\n' "$heading" >>"$combined"
  sed '1{/^# /d;}' "$file" >>"$combined"
  printf '\n' >>"$combined"
}

emit_chapter "Overview" "$root/README.md"
emit_chapter "Decisions" "$root/docs/decisions.md"

# Append one change: proposal, design, tasks, then its capability specs.
# `prefix` is empty for the first change, whose chapters keep their plain
# headings, and names the change for the rest so the table of contents cannot
# show two unrelated "Proposal" entries.
emit_change() {
  local change=$1 prefix=$2
  local change_dir
  change_dir=$(change_dir_for "$change") || return 1
  local doc spec name
  local -a specs=()

  for doc in proposal design tasks; do
    emit_chapter "${prefix}$(tr '[:lower:]' '[:upper:]' <<<"${doc:0:1}")${doc:1}" "$change_dir/$doc.md"
  done

  for spec in "${preferred_specs[@]}"; do
    [[ -f "$change_dir/specs/$spec/spec.md" ]] && specs+=("$spec")
  done
  for spec in "$change_dir"/specs/*/spec.md; do
    [[ -f $spec ]] || continue
    name=$(basename "$(dirname "$spec")")
    [[ " ${specs[*]-} " == *" $name "* ]] || specs+=("$name")
  done

  for spec in "${specs[@]}"; do
    emit_chapter "${prefix}Spec: $spec" "$change_dir/specs/$spec/spec.md"
  done
}

for index in "${!changes[@]}"; do
  if [[ $index -eq 0 ]]; then
    emit_change "${changes[$index]}" ""
  else
    emit_change "${changes[$index]}" "$(change_title "${changes[$index]}"): "
  fi
done

# Reference material that is not part of an OpenSpec change.
emit_chapter "Hardware Guide" "$root/docs/hardware.md"
emit_chapter "ESP32-C3 Host Spike" "$root/docs/esp32-c3-spike.md"
emit_chapter "OTA Spike" "$root/spike/esp32c3-ota/README.md"
emit_chapter "Strip Colour Fault" "$root/docs/strip-colour-fault-handover.md"
emit_chapter "Runtime Metrics" "$root/docs/metrics.md"
emit_chapter "Handoff" "$root/HANDOFF.md"

if [[ ${#changes[@]} -eq 1 ]]; then
  changes_label="OpenSpec change ${changes[0]}"
else
  joined=$(printf '%s, ' "${changes[@]}")
  changes_label="OpenSpec changes: ${joined%, }"
fi

mkdir -p "$out"
# The book keeps the primary change's name even when it carries more than one,
# so the documented dist/replace-fadecandy-with-rust.* paths stay valid.
name=${changes[0]}

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
  --from=gfm+raw_attribute --to=epub3 \
  --lua-filter="$root/scripts/mermaid.lua" \
  --toc --toc-depth=3 --split-level=1 \
  --css="$root/scripts/openspec-ebook.css" \
  --epub-cover-image="$cover" \
  --metadata title="$title" --metadata subtitle="$subtitle" --metadata lang=en \
  -o "$out/$name.epub"

# The PDF cover is raw Typst emitted before the body, so it becomes page 1 and
# the table of contents starts on page 2. The title metadata is deliberately
# empty: pandoc would otherwise print its own title block above the cover.
cover_typst="$tmp/cover.typ"
cat >"$cover_typst" <<EOF
#set document(title: "$title")
#page(
  fill: rgb("#0b0b0f"),
  margin: (x: 0pt, y: 0pt),
  header: none,
  footer: none,
)[
  #align(center)[
    #v(1.4cm)
    #image("$cover_rel", width: 78%)
    #v(1.1cm)
    #text(fill: rgb("#ffffff"), size: 21pt, weight: "bold")[$title]
    #v(0.2cm)
    #line(length: 40%, stroke: 0.6pt + rgb("#4f6bff"))
    #v(0.4cm)
    #text(fill: rgb("#8fb3ff"), size: 12pt)[$subtitle]
    #v(0.35cm)
    #text(fill: rgb("#9a9a9a"), size: 9.5pt)[$changes_label]
  ]
]
#pagebreak()
EOF

# A5 is within a few millimetres of the iPad mini display, so the PDF reads at
# roughly 100% zoom in portrait.
pandoc "$combined" \
  --from=gfm+raw_attribute --to=pdf \
  --lua-filter="$root/scripts/mermaid.lua" \
  --pdf-engine=typst \
  --toc --toc-depth=3 \
  -V papersize=a5 -V fontsize=10pt -V margin.x=1.5cm -V margin.y=1.5cm \
  --include-before-body="$cover_typst" \
  --metadata title="" --metadata lang=en \
  -o "$out/$name.pdf"

echo "wrote:"
ls -lh "$out/$name.epub" "$out/$name.pdf"
