#!/bin/bash
# Print the CHANGELOG.md section for one version, e.g. ci/release-notes.sh v0.3.2
# Fails when the section is missing or empty, so no release goes out without notes.
set -euo pipefail
version="${1#v}"
notes=$(awk -v v="$version" '
  /^## / { if (found) exit; if (index($0, "## " v " ") == 1 || $0 == "## " v) { found=1; next } }
  found { print }
' "${2:-CHANGELOG.md}")
if [ -z "$(printf '%s' "$notes" | tr -d '[:space:]')" ]; then
  echo "CHANGELOG.md has no section for $version" >&2
  exit 1
fi
printf '%s\n' "$notes"
