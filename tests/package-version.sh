#!/usr/bin/env bash
set -euo pipefail

script="$(dirname -- "${BASH_SOURCE[0]}")/../scripts/package-version.sh"
for row in '47 1.00.47 1' '48 1.00.48 1' '49 1.00.48 2' '50 1.00.49 1' '101 1.01.00 1'; do
  read -r count version release <<< "$row"
  [[ "$(VERSION_MAJOR=1 VERSION_COMMIT_COUNT="$count" "$script")" == "$version" ]]
  [[ "$(VERSION_MAJOR=1 VERSION_COMMIT_COUNT="$count" "$script" --rpm-release)" == "$release" ]]
done
printf 'package-version tests: ok\n'
