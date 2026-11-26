#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd)"

major="${VERSION_MAJOR:-1}"
commit_count="${VERSION_COMMIT_COUNT:-}"

if [[ -z "$commit_count" ]]; then
  if git -C "$REPO_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    commit_count="$(git -C "$REPO_ROOT" rev-list --count HEAD)"
  else
    commit_count=0
  fi
fi

# Stable-version revisions: full-history commit count => RPM release.
# Normal commits need no entry and advance the application version as before.
declare -Ar rpm_rebuilds=([49]=2 [51]=2)
[[ "$commit_count" =~ ^(0|[1-9][0-9]*)$ ]] || { echo 'Invalid commit count' >&2; exit 1; }
app_count=$commit_count
for rebuild in "${!rpm_rebuilds[@]}"; do
  if (( rebuild <= commit_count )); then
    app_count=$((app_count - 1))
  fi
done

case "${1:---version}" in
  --version) printf '%s.%02d.%02d\n' "$major" "$((app_count / 100))" "$((app_count % 100))" ;;
  --rpm-release) printf '%s\n' "${rpm_rebuilds[$commit_count]:-1}" ;;
  *) echo 'Usage: package-version.sh [--version|--rpm-release]' >&2; exit 2 ;;
esac
