#!/bin/sh
set -eu

mkdir -p for_embedding

for file in ./*.md; do
  [ -e "$file" ] || continue

  base="$(basename "$file")"

  if [ "$base" = "README.md" ]; then
    continue
  fi

  pandoc \
    "$file" \
    -f gfm \
    -t markdown-auto_identifiers+header_attributes \
    --wrap=preserve \
    -o "for_embedding/$base"

  echo "Wrote for_embedding/$base"
done
