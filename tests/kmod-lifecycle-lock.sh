#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=../scripts/kmod-lifecycle-lock.sh
source "${repo_root}/scripts/kmod-lifecycle-lock.sh"

test_root="$(mktemp -d /tmp/galaxybook-kmod-lifecycle-test.XXXXXX)"
holder_pid=""
trap '
	if [[ -n "$holder_pid" ]]; then
		kill "$holder_pid" 2>/dev/null || true
		wait "$holder_pid" 2>/dev/null || true
	fi
	rm -rf -- "$test_root"
' EXIT

lock_parent="${test_root}/lock"
mkdir -m 0755 -- "$lock_parent"
chmod 0755 -- "$test_root"
kmod_lifecycle_lock_path="${lock_parent}/galaxybook-kmod-lifecycle.lock"
kmod_lifecycle_lock_ancestry=("$test_root" "$lock_parent")
kmod_lifecycle_lock_expected_uid="$EUID"
kmod_lifecycle_lock_expected_gid="$(id -g)"
kmod_lifecycle_lock_timeout_seconds="2"

close_test_lock() {
	local lock_fd="${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}"

	if [[ "$lock_fd" =~ ^[0-9]+$ ]]; then
		exec {lock_fd}>&-
	fi
	unset GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD
}

assert_no_local_fd_for_lock() {
	local lock_identity candidate candidate_identity

	lock_identity="$(stat -Lc '%d:%i' -- "$kmod_lifecycle_lock_path")"
	for candidate in /proc/${BASHPID}/fd/*; do
		candidate_identity="$(stat -Lc '%d:%i' -- "$candidate" 2>/dev/null || true)"
		[[ "$candidate_identity" != "$lock_identity" ]]
	done
}

acquire_kmod_lifecycle_lock
[[ "${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}" =~ ^[0-9]+$ ]]
[[ "$(stat -Lc '%u:%g:%a:%h' -- "$kmod_lifecycle_lock_path")" == \
	"${EUID}:$(id -g):600:1" ]]
validate_kmod_lifecycle_lock
if flock --exclusive --nonblock "$kmod_lifecycle_lock_path" true; then
	printf 'global lifecycle lock did not exclude an independent holder\n' >&2
	exit 1
fi

# A child must reuse the inherited open file description instead of waiting
# for its parent, because nested lifecycle helpers execute as child processes.
bash -c '
	set -euo pipefail
	source "$1"
	kmod_lifecycle_lock_path="$2"
	kmod_lifecycle_lock_ancestry=("$3" "$4")
	kmod_lifecycle_lock_expected_uid="$5"
	kmod_lifecycle_lock_expected_gid="$6"
	kmod_lifecycle_lock_timeout_seconds=1
	validate_kmod_lifecycle_lock
	acquire_kmod_lifecycle_lock
' bash "${repo_root}/scripts/kmod-lifecycle-lock.sh" \
	"$kmod_lifecycle_lock_path" "$test_root" "$lock_parent" \
	"$EUID" "$(id -g)"

close_test_lock
flock --exclusive --nonblock "$kmod_lifecycle_lock_path" true

# `exec {fd}` must remain valid even when callers enable varredir_close.
shopt -s varredir_close
acquire_kmod_lifecycle_lock
validate_kmod_lifecycle_lock
bash -c '
	set -euo pipefail
	source "$1"
	kmod_lifecycle_lock_path="$2"
	kmod_lifecycle_lock_ancestry=("$3" "$4")
	kmod_lifecycle_lock_expected_uid="$5"
	kmod_lifecycle_lock_expected_gid="$6"
	validate_kmod_lifecycle_lock
' bash "${repo_root}/scripts/kmod-lifecycle-lock.sh" \
	"$kmod_lifecycle_lock_path" "$test_root" "$lock_parent" \
	"$EUID" "$(id -g)"
close_test_lock
shopt -u varredir_close

# A read-only empty environment slot must fail before opening the file and
# leave an independent holder unblocked.
readonly_ready="${test_root}/readonly.ready"
readonly_release="${test_root}/readonly.release"
(
	readonly GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD=""
	if acquire_kmod_lifecycle_lock >/dev/null 2>&1; then
		exit 1
	fi
	assert_no_local_fd_for_lock
	: > "$readonly_ready"
	while [[ ! -e "$readonly_release" ]]; do /usr/bin/sleep 0.01; done
) &
readonly_pid=$!
for ((attempt = 1; attempt <= 200; attempt++)); do
	[[ -e "$readonly_ready" ]] && break
	/usr/bin/sleep 0.01
done
[[ -e "$readonly_ready" ]]
flock --exclusive --nonblock "$kmod_lifecycle_lock_path" true
: > "$readonly_release"
wait "$readonly_pid"

# Two independent helper instances must never enter the lifecycle section at
# the same time. The second instance uses the same bounded acquisition path.
instance_ready="${test_root}/instance.ready"
instance_release="${test_root}/instance.release"
bash -c '
	set -euo pipefail
	source "$1"
	kmod_lifecycle_lock_path="$2"
	kmod_lifecycle_lock_ancestry=("$3" "$4")
	kmod_lifecycle_lock_expected_uid="$5"
	kmod_lifecycle_lock_expected_gid="$6"
	acquire_kmod_lifecycle_lock
	: > "$7"
	while [[ ! -e "$8" ]]; do /usr/bin/sleep 0.01; done
' bash "${repo_root}/scripts/kmod-lifecycle-lock.sh" \
	"$kmod_lifecycle_lock_path" "$test_root" "$lock_parent" \
	"$EUID" "$(id -g)" "$instance_ready" "$instance_release" &
holder_pid=$!
for ((attempt = 1; attempt <= 200; attempt++)); do
	[[ -e "$instance_ready" ]] && break
	/usr/bin/sleep 0.01
done
[[ -e "$instance_ready" ]]
kmod_lifecycle_lock_timeout_seconds="1"
if acquire_kmod_lifecycle_lock >/dev/null 2>&1; then
	printf 'two independent lifecycle helpers entered concurrently\n' >&2
	exit 1
fi
: > "$instance_release"
wait "$holder_pid"
holder_pid=""

# A forged inherited descriptor must fail closed instead of falling back to a
# second open, which would hide a broken lock-order contract.
unrelated_file="${test_root}/unrelated.lock"
(umask 077; : > "$unrelated_file")
exec {unrelated_fd}<>"$unrelated_file"
export GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD="$unrelated_fd"
if acquire_kmod_lifecycle_lock >/dev/null 2>&1; then
	printf 'global lifecycle lock accepted an unrelated inherited descriptor\n' >&2
	exit 1
fi
exec {unrelated_fd}>&-
unset GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD

# An independent process holding the same path must force a bounded timeout.
holder_ready="${test_root}/holder.ready"
holder_release="${test_root}/holder.release"
(
	flock --exclusive "$kmod_lifecycle_lock_path" bash -c '
		: > "$1"
		while [[ ! -e "$2" ]]; do /usr/bin/sleep 0.01; done
	' bash "$holder_ready" "$holder_release"
) &
holder_pid=$!
for ((attempt = 1; attempt <= 200; attempt++)); do
	[[ -e "$holder_ready" ]] && break
	/usr/bin/sleep 0.01
done
[[ -e "$holder_ready" ]]
kmod_lifecycle_lock_timeout_seconds="1"
start_seconds="$SECONDS"
if acquire_kmod_lifecycle_lock >/dev/null 2>&1; then
	printf 'global lifecycle lock ignored an external holder\n' >&2
	exit 1
fi
elapsed_seconds=$((SECONDS - start_seconds))
(( elapsed_seconds >= 1 && elapsed_seconds <= 3 ))
[[ -z "${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}" ]]
assert_no_local_fd_for_lock
: > "$holder_release"
wait "$holder_pid"
holder_pid=""

assert_unsafe_lock_fails() {
	local description="$1"
	if acquire_kmod_lifecycle_lock >/dev/null 2>&1; then
		printf 'global lifecycle lock accepted %s\n' "$description" >&2
		exit 1
	fi
	[[ -z "${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}" ]]
}

chmod 0660 -- "$kmod_lifecycle_lock_path"
assert_unsafe_lock_fails 'a group-writable lock file'
chmod 0600 -- "$kmod_lifecycle_lock_path"

hardlink="${test_root}/global-lock-hardlink"
ln -- "$kmod_lifecycle_lock_path" "$hardlink"
assert_unsafe_lock_fails 'a multiply linked lock file'
unlink -- "$hardlink"

mv -- "$kmod_lifecycle_lock_path" "${kmod_lifecycle_lock_path}.regular"
ln -s -- "${kmod_lifecycle_lock_path}.regular" "$kmod_lifecycle_lock_path"
assert_unsafe_lock_fails 'a lock symlink'
unlink -- "$kmod_lifecycle_lock_path"
mv -- "${kmod_lifecycle_lock_path}.regular" "$kmod_lifecycle_lock_path"

rm -f -- "$kmod_lifecycle_lock_path"
mkdir -- "$kmod_lifecycle_lock_path"
assert_unsafe_lock_fails 'a directory at the lock path'
rmdir -- "$kmod_lifecycle_lock_path"

mkfifo -- "$kmod_lifecycle_lock_path"
assert_unsafe_lock_fails 'a FIFO at the lock path'
unlink -- "$kmod_lifecycle_lock_path"

if mknod -- "$kmod_lifecycle_lock_path" c 1 3 2>/dev/null; then
	assert_unsafe_lock_fails 'a device at the lock path'
	unlink -- "$kmod_lifecycle_lock_path"
fi

(umask 077; : > "$kmod_lifecycle_lock_path")

chmod 0775 -- "$lock_parent"
assert_unsafe_lock_fails 'a group-writable ancestry directory'
chmod 0755 -- "$lock_parent"

mv -- "$lock_parent" "${lock_parent}.regular"
ln -s -- "${lock_parent}.regular" "$lock_parent"
assert_unsafe_lock_fails 'an ancestry symlink'
unlink -- "$lock_parent"
mv -- "${lock_parent}.regular" "$lock_parent"

# Unsafe ancestry must be rejected before a missing lock can be created.
rm -f -- "$kmod_lifecycle_lock_path"
chmod 0777 -- "$lock_parent"
assert_unsafe_lock_fails 'unsafe ancestry for a missing lock'
[[ ! -e "$kmod_lifecycle_lock_path" ]]
chmod 0755 -- "$lock_parent"

acquire_kmod_lifecycle_lock
close_test_lock

printf 'kmod-lifecycle-lock tests: ok\n'
