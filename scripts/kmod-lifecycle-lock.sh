#!/usr/bin/env bash

# This helper is intentionally driver-neutral. Every Galaxy Book component
# that can mutate kernel-module indexes or initramfs images ships the same
# source and participates in this one BSD flock.
kmod_lifecycle_lock_path="/run/lock/galaxybook-kmod-lifecycle.lock"
kmod_lifecycle_lock_timeout_seconds="600"
kmod_lifecycle_lock_flock_command="/usr/bin/flock"
kmod_lifecycle_lock_stat_command="/usr/bin/stat"
kmod_lifecycle_lock_dirname_command="/usr/bin/dirname"
kmod_lifecycle_lock_id_command="/usr/bin/id"
kmod_lifecycle_lock_expected_uid="0"
kmod_lifecycle_lock_expected_gid="0"
kmod_lifecycle_lock_ancestry=(/run /run/lock)

kmod_lifecycle_lock_configuration_is_safe() {
	local lock_parent

	[[ "$kmod_lifecycle_lock_path" == /* && \
		"$kmod_lifecycle_lock_path" != *$'\n'* && \
		"$kmod_lifecycle_lock_timeout_seconds" =~ ^[1-9][0-9]*$ && \
		"$kmod_lifecycle_lock_expected_uid" =~ ^[0-9]+$ && \
		"$kmod_lifecycle_lock_expected_gid" =~ ^[0-9]+$ && \
		"${#kmod_lifecycle_lock_ancestry[@]}" -gt 0 ]] || return 1
	lock_parent="$("$kmod_lifecycle_lock_dirname_command" -- \
		"$kmod_lifecycle_lock_path")" || return 1
	[[ "$lock_parent" == \
		"${kmod_lifecycle_lock_ancestry[${#kmod_lifecycle_lock_ancestry[@]} - 1]}" ]]
}

kmod_lifecycle_lock_ancestry_is_safe() {
	local directory metadata mode

	kmod_lifecycle_lock_configuration_is_safe || return 1
	for directory in "${kmod_lifecycle_lock_ancestry[@]}"; do
		[[ "$directory" == /* && "$directory" != *$'\n'* && \
			-d "$directory" && ! -L "$directory" ]] || return 1
		metadata="$("$kmod_lifecycle_lock_stat_command" -Lc '%u:%g:%a' -- \
			"$directory")" || return 1
		[[ "$metadata" =~ ^([0-9]+):([0-9]+):([0-7]{3,4})$ ]] || return 1
		[[ "${BASH_REMATCH[1]}" == "$kmod_lifecycle_lock_expected_uid" && \
			"${BASH_REMATCH[2]}" == "$kmod_lifecycle_lock_expected_gid" ]] || return 1
		mode="${BASH_REMATCH[3]}"
		(( (8#${mode} & 8#022) == 0 )) || return 1
	done
}

kmod_lifecycle_lock_file_is_safe() {
	local path="$1"
	local metadata

	[[ -f "$path" ]] || return 1
	metadata="$("$kmod_lifecycle_lock_stat_command" -Lc '%u:%g:%a:%h' -- \
		"$path")" || return 1
	[[ "$metadata" == \
		"${kmod_lifecycle_lock_expected_uid}:${kmod_lifecycle_lock_expected_gid}:600:1" ]]
}

kmod_lifecycle_lock_fd_matches_path() {
	local lock_fd="$1"
	local fd_path fd_identity path_identity fd_metadata path_metadata

	[[ "$lock_fd" =~ ^[0-9]+$ && "$lock_fd" -ge 3 ]] || return 1
	fd_path="/proc/${BASHPID}/fd/${lock_fd}"
	[[ -r "$fd_path" && -w "$fd_path" && -f "$fd_path" && \
		-f "$kmod_lifecycle_lock_path" && \
		! -L "$kmod_lifecycle_lock_path" ]] || return 1
	fd_identity="$("$kmod_lifecycle_lock_stat_command" -Lc '%d:%i' -- \
		"$fd_path")" || return 1
	path_identity="$("$kmod_lifecycle_lock_stat_command" -Lc '%d:%i' -- \
		"$kmod_lifecycle_lock_path")" || return 1
	[[ "$fd_identity" == "$path_identity" ]] || return 1
	fd_metadata="$("$kmod_lifecycle_lock_stat_command" -Lc '%u:%g:%a:%h' -- \
		"$fd_path")" || return 1
	path_metadata="$("$kmod_lifecycle_lock_stat_command" -Lc '%u:%g:%a:%h' -- \
		"$kmod_lifecycle_lock_path")" || return 1
	[[ "$fd_metadata" == "$path_metadata" && "$path_metadata" == \
		"${kmod_lifecycle_lock_expected_uid}:${kmod_lifecycle_lock_expected_gid}:600:1" ]]
}

validate_kmod_lifecycle_lock() {
	local lock_fd="${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}"

	kmod_lifecycle_lock_ancestry_is_safe || return 1
	kmod_lifecycle_lock_fd_matches_path "$lock_fd" || return 1
	# Reapplying BSD flock on the inherited open file description is
	# idempotent. Nonblocking mode proves that the descriptor still owns the
	# lock without silently waiting in a child that should have inherited it.
	"$kmod_lifecycle_lock_flock_command" --exclusive --nonblock "$lock_fd" \
		|| return 1
	kmod_lifecycle_lock_ancestry_is_safe || return 1
	kmod_lifecycle_lock_fd_matches_path "$lock_fd"
}

kmod_lifecycle_lock_descriptor_is_assignable() {
	local declaration

	if ! declaration="$(declare -p GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD \
		2>/dev/null)"
	then
		return 0
	fi
	[[ ! "$declaration" =~ ^declare\ -[^[:space:]]*r[^[:space:]]*\  ]]
}

acquire_kmod_lifecycle_lock() {
	local lock_fd created_lock=0
	local current_gid

	if [[ -n "${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}" ]]; then
		if validate_kmod_lifecycle_lock; then
			return 0
		fi
		printf 'galaxybook-kmod-lifecycle: inherited lock descriptor is unsafe\n' >&2
		return 1
	fi
	if ! kmod_lifecycle_lock_descriptor_is_assignable; then
		printf 'galaxybook-kmod-lifecycle: lock descriptor variable is read-only\n' >&2
		return 1
	fi

	kmod_lifecycle_lock_ancestry_is_safe || {
		printf 'galaxybook-kmod-lifecycle: lock ancestry is missing or unsafe\n' >&2
		return 1
	}
	current_gid="$("$kmod_lifecycle_lock_id_command" -g)" || return 1
	if [[ "$EUID" != "$kmod_lifecycle_lock_expected_uid" || \
		"$current_gid" != "$kmod_lifecycle_lock_expected_gid" ]]
	then
		printf 'galaxybook-kmod-lifecycle: caller does not own the lock hierarchy\n' >&2
		return 1
	fi

	if [[ -L "$kmod_lifecycle_lock_path" ]]; then
		printf 'galaxybook-kmod-lifecycle: lock path is a symlink\n' >&2
		return 1
	fi
	if [[ ! -e "$kmod_lifecycle_lock_path" ]]; then
		if (umask 077; set -o noclobber; : > "$kmod_lifecycle_lock_path") \
			2>/dev/null
		then
			created_lock=1
		elif [[ ! -e "$kmod_lifecycle_lock_path" || \
			-L "$kmod_lifecycle_lock_path" ]]
		then
			printf 'galaxybook-kmod-lifecycle: could not create the lock safely\n' >&2
			return 1
		fi
	fi
	if ! kmod_lifecycle_lock_file_is_safe "$kmod_lifecycle_lock_path" || \
		[[ -L "$kmod_lifecycle_lock_path" ]]
	then
		if [[ "$created_lock" -eq 1 ]]; then
			printf 'galaxybook-kmod-lifecycle: newly created lock has unsafe metadata\n' >&2
		else
			printf 'galaxybook-kmod-lifecycle: existing lock has unsafe metadata\n' >&2
		fi
		return 1
	fi

	if ! exec {lock_fd}<>"$kmod_lifecycle_lock_path"; then
		printf 'galaxybook-kmod-lifecycle: could not open the lock\n' >&2
		return 1
	fi
	if ! kmod_lifecycle_lock_ancestry_is_safe || \
		! kmod_lifecycle_lock_fd_matches_path "$lock_fd"
	then
		exec {lock_fd}>&-
		printf 'galaxybook-kmod-lifecycle: lock changed while it was opened\n' >&2
		return 1
	fi
	if ! "$kmod_lifecycle_lock_flock_command" --exclusive \
		--timeout "$kmod_lifecycle_lock_timeout_seconds" "$lock_fd"
	then
		exec {lock_fd}>&-
		printf 'galaxybook-kmod-lifecycle: timed out waiting for the lock\n' >&2
		return 1
	fi
	if ! kmod_lifecycle_lock_ancestry_is_safe || \
		! kmod_lifecycle_lock_fd_matches_path "$lock_fd"
	then
		exec {lock_fd}>&-
		printf 'galaxybook-kmod-lifecycle: lock changed after acquisition\n' >&2
		return 1
	fi

	if ! builtin export GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD="$lock_fd"; then
		exec {lock_fd}>&-
		unset GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD 2>/dev/null || true
		printf 'galaxybook-kmod-lifecycle: could not publish the lock descriptor\n' >&2
		return 1
	fi
}
