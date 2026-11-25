#!/usr/bin/env bash

set -euo pipefail

readonly TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "$TEST_DIR/.." && pwd)"
readonly TEST_ROOT="$(mktemp -d)"
trap 'rm -rf -- "$TEST_ROOT"' EXIT

# shellcheck source=../scripts/configure-browser-camera.sh
source "$REPO_ROOT/scripts/configure-browser-camera.sh"

# Function-focused tests below acquire only the private/RPM fixtures. The
# production main() sources and acquires the global lifecycle lock first.
validate_kmod_lifecycle_lock() { :; }

test_browser_helper_acquires_global_before_private_and_rpm_locks() {
  local helper="$REPO_ROOT/scripts/configure-browser-camera.sh"
  local global private rpm activate

  global="$(grep -n '^[[:space:]]*acquire_kmod_lifecycle_lock \\' "$helper" | tail -1 | cut -d: -f1)"
  private="$(grep -n '^[[:space:]]*acquire_action_lock \\' "$helper" | tail -1 | cut -d: -f1)"
  rpm="$(grep -n '^acquire_rpm_read_lock() {' "$helper" | cut -d: -f1)"
  activate="$(grep -n '^  activate_browser_relay$' "$helper" | cut -d: -f1)"
  [[ -n "$global" && -n "$private" && -n "$rpm" && -n "$activate" ]]
  (( rpm < global && global < private && private < activate ))
}

test_browser_helper_acquires_or_reuses_global_before_private_work() {
  local test_root lock_parent lock_path lifecycle_fixture trace

  test_root="$(mktemp -d /tmp/galaxybook-browser-global-test.XXXXXX)"
  lock_parent="$test_root/lock"
  lock_path="$lock_parent/galaxybook-kmod-lifecycle.lock"
  lifecycle_fixture="$test_root/lifecycle-helper"
  trace="$test_root/trace"
  mkdir -m 0755 -- "$lock_parent"
  chmod 0755 -- "$test_root"
  printf '%s\n' \
    'source "$BROWSER_GLOBAL_CANONICAL_HELPER"' \
    'kmod_lifecycle_lock_path="$BROWSER_GLOBAL_LOCK_PATH"' \
    'kmod_lifecycle_lock_ancestry=("$BROWSER_GLOBAL_TEST_ROOT" "$BROWSER_GLOBAL_LOCK_PARENT")' \
    'kmod_lifecycle_lock_expected_uid="$EUID"' \
    'kmod_lifecycle_lock_expected_gid="$(id -g)"' \
    'kmod_lifecycle_lock_timeout_seconds=1' \
    > "$lifecycle_fixture"

  for inherited in 0 1; do
    (
      export BROWSER_GLOBAL_CANONICAL_HELPER="$REPO_ROOT/scripts/kmod-lifecycle-lock.sh"
      export BROWSER_GLOBAL_LOCK_PATH="$lock_path"
      export BROWSER_GLOBAL_TEST_ROOT="$test_root"
      export BROWSER_GLOBAL_LOCK_PARENT="$lock_parent"
      if [[ "$inherited" == 1 ]]; then
        source "$lifecycle_fixture"
        acquire_kmod_lifecycle_lock
      fi
      caller_is_root() { return 0; }
      require_command() { :; }
      lifecycle_helper="$lifecycle_fixture"
      acquire_action_lock() {
        validate_kmod_lifecycle_lock
        if flock --exclusive --nonblock "$lock_path" true; then
          return 91
        fi
        printf 'PRIVATE_AFTER_GLOBAL:%s\n' "$inherited" >> "$trace"
        return 76
      }
      main
    ) >/dev/null 2>&1 || [[ "$?" -eq 1 ]]
  done
  grep -Fxq 'PRIVATE_AFTER_GLOBAL:0' "$trace"
  grep -Fxq 'PRIVATE_AFTER_GLOBAL:1' "$trace"
  rm -rf -- "$test_root"
}

test_browser_helper_stops_before_private_work_when_global_fails() {
  local test_root lifecycle_fixture trace status

  test_root="$(mktemp -d)"
  lifecycle_fixture="$test_root/lifecycle-helper"
  trace="$test_root/trace"
  printf '%s\n' \
    'acquire_kmod_lifecycle_lock() { return 75; }' \
    'validate_kmod_lifecycle_lock() { return 1; }' \
    > "$lifecycle_fixture"
  if (
    caller_is_root() { return 0; }
    require_command() { :; }
    lifecycle_helper="$lifecycle_fixture"
    acquire_action_lock() { printf 'PRIVATE\n' >> "$trace"; }
    main
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  [[ "$status" -ne 0 && ! -e "$trace" ]]
  rm -rf -- "$test_root"
}

MANAGED_FILE_EXPECTED_OWNER="$(id -u):$(id -g)"
MANAGED_PATH_ANCHOR="$TEST_ROOT"
ACTION_LOCK_EXPECTED_OWNER="$MANAGED_FILE_EXPECTED_OWNER"
ACTION_LOCK_PATH="$TEST_ROOT/action-lock/galaxybook-browser-camera.lock"
ACTION_LOCK_PATH_ANCHOR="$TEST_ROOT"
ACTION_LOCK_TIMEOUT_SECONDS="1"
RPM_LOCK_EXPECTED_OWNER="$MANAGED_FILE_EXPECTED_OWNER"
RPM_LOCK_PATH="$TEST_ROOT/rpmdb/.rpm.lock"
RPM_LOCK_PATH_ANCHOR="$TEST_ROOT"
RPM_LOCK_TIMEOUT_SECONDS="1"
mkdir -m 0700 -- "$TEST_ROOT/action-lock" "$TEST_ROOT/rpmdb"
touch -- "$RPM_LOCK_PATH"
chmod 0600 -- "$RPM_LOCK_PATH"
"$FLOCK_COMMAND" --help | grep -Fq -- '--fcntl'
acquire_action_lock
[[ "$(stat -c '%u:%g:%a:%h' "$ACTION_LOCK_PATH")" \
  == "${ACTION_LOCK_EXPECTED_OWNER}:600:1" ]]
if "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$ACTION_LOCK_PATH" true; then
  printf 'the private action lock was not held exclusively\n' >&2
  exit 1
fi

rpm() {
  local target="${*: -1}"

  case "${RPM_TEST_MODE:-unowned}" in
    unowned)
      printf 'file %s is not owned by any package\n' "$target" >&2
      return 1
      ;;
    owned)
      printf '%s\n' test-owner
      return 0
      ;;
    owned_multiple)
      printf '%s\n' test-owner second-owner
      return 0
      ;;
    owned_extra_lf)
      printf 'test-owner\n\n'
      return 0
      ;;
    owned_nul)
      printf 'test-owner\000\n'
      return 0
      ;;
    owned_invalid_byte)
      printf 'test-owner\377\n'
      return 0
      ;;
    owned_missing_lf)
      printf '%s' test-owner
      return 0
      ;;
    owned_warning)
      printf '%s\n' test-owner
      printf '%s\n' 'rpm warning' >&2
      return 0
      ;;
    owned_failed_with_output)
      printf '%s\n' test-owner
      return 2
      ;;
    unavailable)
      printf '%s\n' 'rpm database unavailable' >&2
      return 2
      ;;
    assert_locked)
      if "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$RPM_LOCK_PATH" true; then
        printf '%s\n' 'RPM shared lock was not held during ownership query' >&2
        return 2
      fi
      printf 'file %s is not owned by any package\n' "$target" >&2
      return 1
      ;;
    replace_during_query)
      mv -fT -- "$RPM_RACE_REPLACEMENT" "$RPM_RACE_TARGET"
      printf 'file %s is not owned by any package\n' "$target" >&2
      return 1
      ;;
    chmod_ancestry_during_query)
      chmod 0777 -- "$RPM_RACE_DIRECTORY"
      printf 'file %s is not owned by any package\n' "$target" >&2
      return 1
      ;;
    fedora_bridge)
      if "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$RPM_LOCK_PATH" true; then
        printf '%s\n' 'RPM shared lock was not held during Fedora bridge ownership query' >&2
        return 2
      fi
      case "$target" in
        "$FEDORA_BRIDGE_LINK") printf '%s\n' v4l2-relayd ;;
        "$FEDORA_BRIDGE_TARGET") printf '%s\n' ipu6-camera-hal ;;
        *)
          printf 'file %s is not owned by any package\n' "$target" >&2
          return 1
          ;;
      esac
      ;;
  esac
}

mkdir -p "$TEST_ROOT/existing-directory"
chmod 0750 "$TEST_ROOT/existing-directory"
ensure_managed_directory "$TEST_ROOT/existing-directory"
[[ "$(stat -c '%a' "$TEST_ROOT/existing-directory")" == 750 ]]
ensure_managed_directory "$TEST_ROOT/new-directory"
[[ "$(stat -c '%a' "$TEST_ROOT/new-directory")" == 755 ]]
mkdir -m 0777 -- "$TEST_ROOT/writable-directory"
if (ensure_managed_directory "$TEST_ROOT/writable-directory") >/dev/null 2>&1; then
  printf 'ensure_managed_directory accepted a group/world-writable directory\n' >&2
  exit 1
fi
[[ "$(stat -c '%a' "$TEST_ROOT/writable-directory")" == 777 ]]
mkdir -m 0777 -- "$TEST_ROOT/writable-parent"
if (ensure_managed_directory "$TEST_ROOT/writable-parent/child") >/dev/null 2>&1; then
  printf 'ensure_managed_directory accepted a writable intermediate parent\n' >&2
  exit 1
fi
[[ ! -e "$TEST_ROOT/writable-parent/child" ]]
mkdir -p "$TEST_ROOT/redirected-directory"
ln -s "$TEST_ROOT/redirected-directory" "$TEST_ROOT/directory-link"
if (ensure_managed_directory "$TEST_ROOT/directory-link") >/dev/null 2>&1; then
  printf 'ensure_managed_directory followed an ancestry symlink\n' >&2
  exit 1
fi
mkdir -p "$TEST_ROOT/real-parent"
ln -s "$TEST_ROOT/real-parent" "$TEST_ROOT/parent-link"
if (ensure_managed_directory "$TEST_ROOT/parent-link/nested") >/dev/null 2>&1; then
  printf 'ensure_managed_directory followed an intermediate ancestry symlink\n' >&2
  exit 1
fi

printf '%s\n' "conteúdo administrado" > "$TEST_ROOT/admin.conf"
printf '%s\n' "conteúdo esperado" > "$TEST_ROOT/expected.conf"
ln -s "$TEST_ROOT/admin.conf" "$TEST_ROOT/managed.conf"

legacy_udev_fixture="$TEST_ROOT/90-hide-ipu6-v4l2.rules"
legacy_udev_expected="$TEST_ROOT/legacy-udev-expected.rules"
runtime_side_effects="$TEST_ROOT/runtime-side-effects"
printf '%s\n' \
  'SUBSYSTEM=="video4linux", KERNEL=="video*", ATTR{name}=="Intel IPU6 ISYS Capture*", TAG-="uaccess"' \
  'SUBSYSTEM=="video4linux", KERNEL=="video*", ATTR{name}=="Intel IPU6 CSI2*", TAG-="uaccess"' \
  > "$legacy_udev_expected"
cp -- "$legacy_udev_expected" "$legacy_udev_fixture"
if (
  udevadm() {
    printf '%s\n' udevadm >> "$runtime_side_effects"
  }
  systemctl() {
    printf '%s\n' systemctl >> "$runtime_side_effects"
  }
  refuse_legacy_udev_configuration \
    "$legacy_udev_fixture" \
    "$legacy_udev_expected"
  udevadm control --reload-rules
  systemctl restart v4l2-relayd@icamerasrc.service
) >/dev/null 2>&1; then
  printf 'legacy udev preflight accepted an obsolete rule\n' >&2
  exit 1
fi
cmp -s -- "$legacy_udev_fixture" "$legacy_udev_expected"
[[ ! -e "$runtime_side_effects" ]]

(
  WORKDIR="$TEST_ROOT/v4l2-probe"
  mkdir -m 0700 -- "$WORKDIR"
  v4l2-ctl() {
    case "$V4L2_TEST_MODE" in
      clean)
        printf 'Driver Info:\n\tVideo Capture\n'
        return 0
        ;;
      warning)
        printf 'Driver Info:\n\tVideo Capture\n'
        printf '%s\n' 'v4l2 warning' >&2
        return 0
        ;;
      failed)
        printf 'Driver Info:\n\tVideo Capture\n'
        return 1
        ;;
      near)
        printf '%s\n' 'Not Video Capture'
        return 0
        ;;
      empty) return 0 ;;
      missing-newline)
        printf '%s' 'Video Capture'
        return 0
        ;;
      carriage-return)
        printf 'Video Capture\r\n'
        return 0
        ;;
      nul)
        printf 'Video Capture\000\n'
        return 0
        ;;
      invalid-byte)
        printf '\377\n\tVideo Capture\n'
        return 0
        ;;
      stderr-only)
        printf '%s\n' 'Video Capture' >&2
        return 0
        ;;
    esac
    return 64
  }

  V4L2_TEST_MODE=clean
  virtual_camera_has_capture_capability /dev/video-test
  for V4L2_TEST_MODE in \
    warning \
    failed \
    near \
    empty \
    missing-newline \
    carriage-return \
    nul \
    invalid-byte \
    stderr-only; do
    if virtual_camera_has_capture_capability /dev/video-test; then
      printf 'virtual camera validation accepted inconclusive v4l2-ctl mode %s\n' \
        "$V4L2_TEST_MODE" >&2
      exit 1
    fi
  done
)

write_managed_file "$TEST_ROOT/created.conf" "$TEST_ROOT/expected.conf"
cmp -s -- "$TEST_ROOT/created.conf" "$TEST_ROOT/expected.conf"
[[ "$(stat -c '%u:%g:%a:%h' "$TEST_ROOT/created.conf")" \
  == "${MANAGED_FILE_EXPECTED_OWNER}:644:1" ]]
write_managed_file "$TEST_ROOT/created.conf" "$TEST_ROOT/expected.conf"
if compgen -G "$TEST_ROOT/.galaxybook-setup.*" >/dev/null; then
  printf 'write_managed_file left a temporary file behind\n' >&2
  exit 1
fi

printf '%s\n' "conteúdo local" > "$TEST_ROOT/existing.conf"
if (write_managed_file "$TEST_ROOT/existing.conf" "$TEST_ROOT/expected.conf") >/dev/null 2>&1; then
  printf 'write_managed_file overwrote an existing administrative file\n' >&2
  exit 1
fi
grep -Fxq "conteúdo local" "$TEST_ROOT/existing.conf"

for unsafe_mode in 0666 0777; do
  unsafe_file="$TEST_ROOT/existing-${unsafe_mode}.conf"
  cp -- "$TEST_ROOT/expected.conf" "$unsafe_file"
  chmod "$unsafe_mode" -- "$unsafe_file"
  if (write_managed_file "$unsafe_file" "$TEST_ROOT/expected.conf") >/dev/null 2>&1; then
    printf 'write_managed_file accepted mode %s\n' "$unsafe_mode" >&2
    exit 1
  fi
  [[ "$(stat -c '%a' "$unsafe_file")" == "${unsafe_mode#0}" ]]
done

saved_expected_owner="$MANAGED_FILE_EXPECTED_OWNER"
if [[ "$saved_expected_owner" == "0:0" ]]; then
  MANAGED_FILE_EXPECTED_OWNER="1:0"
else
  MANAGED_FILE_EXPECTED_OWNER="0:0"
fi
cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/wrong-owner.conf"
if managed_file_matches_exactly \
  "$TEST_ROOT/wrong-owner.conf" \
  "$TEST_ROOT/expected.conf"; then
  MANAGED_FILE_EXPECTED_OWNER="$saved_expected_owner"
  printf 'managed-file validation accepted an unexpected owner\n' >&2
  exit 1
fi
MANAGED_FILE_EXPECTED_OWNER="$saved_expected_owner"
cmp -s -- "$TEST_ROOT/wrong-owner.conf" "$TEST_ROOT/expected.conf"

cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/hardlinked.conf"
ln -- "$TEST_ROOT/hardlinked.conf" "$TEST_ROOT/hardlinked-peer.conf"
if (write_managed_file "$TEST_ROOT/hardlinked.conf" "$TEST_ROOT/expected.conf") >/dev/null 2>&1; then
  printf 'write_managed_file accepted a multiply linked file\n' >&2
  exit 1
fi
[[ "$(stat -c '%h' "$TEST_ROOT/hardlinked.conf")" == 2 ]]

if (write_managed_file "$TEST_ROOT/managed.conf" "$TEST_ROOT/expected.conf") >/dev/null 2>&1; then
  printf 'write_managed_file followed an unexpected symlink\n' >&2
  exit 1
fi
grep -Fxq "conteúdo administrado" "$TEST_ROOT/admin.conf"

mkdir -p "$TEST_ROOT/real-existing-parent"
cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/real-existing-parent/exact.conf"
ln -s "$TEST_ROOT/real-existing-parent" "$TEST_ROOT/existing-parent-link"
if (write_managed_file \
  "$TEST_ROOT/existing-parent-link/exact.conf" \
  "$TEST_ROOT/expected.conf") >/dev/null 2>&1; then
  printf 'write_managed_file accepted an existing file through an ancestry symlink\n' >&2
  exit 1
fi
cmp -s -- "$TEST_ROOT/real-existing-parent/exact.conf" "$TEST_ROOT/expected.conf"

diagnose_existing_file "$TEST_ROOT/managed.conf" "$TEST_ROOT/expected.conf"
[[ -L "$TEST_ROOT/managed.conf" ]]
grep -Fxq "conteúdo administrado" "$TEST_ROOT/admin.conf"

printf '%s\n' "personalização local" > "$TEST_ROOT/unknown.conf"
diagnose_existing_file "$TEST_ROOT/unknown.conf" "$TEST_ROOT/expected.conf"
grep -Fxq "personalização local" "$TEST_ROOT/unknown.conf"

cp "$TEST_ROOT/expected.conf" "$TEST_ROOT/known.conf"
RPM_TEST_MODE=assert_locked diagnose_existing_file \
  "$TEST_ROOT/known.conf" \
  "$TEST_ROOT/expected.conf"
cmp -s -- "$TEST_ROOT/known.conf" "$TEST_ROOT/expected.conf"
"$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$RPM_LOCK_PATH" true

cp "$TEST_ROOT/expected.conf" "$TEST_ROOT/rpm-owned.conf"
RPM_TEST_MODE=owned diagnose_existing_file \
  "$TEST_ROOT/rpm-owned.conf" \
  "$TEST_ROOT/expected.conf"
cmp -s -- "$TEST_ROOT/rpm-owned.conf" "$TEST_ROOT/expected.conf"

(
  local_owners=""
  acquire_rpm_read_lock
  RPM_TEST_MODE=owned rpm_file_owners "$TEST_ROOT/rpm-owned.conf" local_owners
  [[ "$local_owners" == test-owner ]]
  RPM_TEST_MODE=owned_multiple \
    rpm_file_owners "$TEST_ROOT/rpm-owned.conf" local_owners
  [[ "$local_owners" == $'test-owner\nsecond-owner' ]]
  for RPM_TEST_MODE in \
    owned_extra_lf \
    owned_nul \
    owned_invalid_byte \
    owned_missing_lf \
    owned_warning \
    owned_failed_with_output; do
    if rpm_file_owners "$TEST_ROOT/rpm-owned.conf" local_owners; then
      printf 'RPM owner query accepted inconclusive byte shape %s\n' \
        "$RPM_TEST_MODE" >&2
      exit 1
    fi
  done
)

cp "$TEST_ROOT/expected.conf" "$TEST_ROOT/rpm-unknown.conf"
RPM_TEST_MODE=unavailable diagnose_existing_file \
  "$TEST_ROOT/rpm-unknown.conf" \
  "$TEST_ROOT/expected.conf"
cmp -s -- "$TEST_ROOT/rpm-unknown.conf" "$TEST_ROOT/expected.conf"

for unsafe_mode in 0666 0777; do
  unsafe_file="$TEST_ROOT/diagnostic-${unsafe_mode}.conf"
  cp -- "$TEST_ROOT/expected.conf" "$unsafe_file"
  chmod "$unsafe_mode" -- "$unsafe_file"
  diagnose_existing_file "$unsafe_file" "$TEST_ROOT/expected.conf"
  [[ -e "$unsafe_file" ]]
  [[ "$(stat -c '%a' "$unsafe_file")" == "${unsafe_mode#0}" ]]
done

cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/diagnostic-hardlink.conf"
ln -- "$TEST_ROOT/diagnostic-hardlink.conf" "$TEST_ROOT/diagnostic-hardlink-peer.conf"
diagnose_existing_file \
  "$TEST_ROOT/diagnostic-hardlink.conf" \
  "$TEST_ROOT/expected.conf"
[[ -e "$TEST_ROOT/diagnostic-hardlink.conf" ]]
[[ "$(stat -c '%h' "$TEST_ROOT/diagnostic-hardlink.conf")" == 2 ]]

mkdir -m 0777 -- "$TEST_ROOT/diagnostic-writable-directory"
cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/diagnostic-writable-directory/known.conf"
diagnose_existing_file \
  "$TEST_ROOT/diagnostic-writable-directory/known.conf" \
  "$TEST_ROOT/expected.conf"
cmp -s -- \
  "$TEST_ROOT/diagnostic-writable-directory/known.conf" \
  "$TEST_ROOT/expected.conf"

cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/race-target.conf"
printf '%s\n' "substituição administrativa concorrente" > "$TEST_ROOT/race-replacement.conf"
RPM_RACE_TARGET="$TEST_ROOT/race-target.conf" \
RPM_RACE_REPLACEMENT="$TEST_ROOT/race-replacement.conf" \
RPM_TEST_MODE=replace_during_query \
  diagnose_existing_file \
    "$TEST_ROOT/race-target.conf" \
    "$TEST_ROOT/expected.conf"
grep -Fxq "substituição administrativa concorrente" "$TEST_ROOT/race-target.conf"
[[ ! -e "$TEST_ROOT/race-replacement.conf" ]]

mkdir -m 0700 -- "$TEST_ROOT/race-directory"
cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/race-directory/known.conf"
RPM_RACE_DIRECTORY="$TEST_ROOT/race-directory" \
RPM_TEST_MODE=chmod_ancestry_during_query \
  diagnose_existing_file \
    "$TEST_ROOT/race-directory/known.conf" \
    "$TEST_ROOT/expected.conf"
cmp -s -- "$TEST_ROOT/race-directory/known.conf" "$TEST_ROOT/expected.conf"
[[ "$(stat -c '%a' "$TEST_ROOT/race-directory")" == 777 ]]
chmod 0700 -- "$TEST_ROOT/race-directory"

cp -- "$TEST_ROOT/expected.conf" "$TEST_ROOT/post-identity-target.conf"
printf '%s\n' "substituição posterior à identidade" \
  > "$TEST_ROOT/post-identity-replacement.conf"
(
  eval "$(
    declare -f opened_managed_file_is_current \
      | sed '1s/^opened_managed_file_is_current/original_opened_managed_file_is_current/'
  )"
  opened_current_calls=0
  opened_managed_file_is_current() {
    original_opened_managed_file_is_current "$@" || return 1
    (( opened_current_calls += 1 ))
    if (( opened_current_calls == 2 )); then
      mv -fT -- \
        "$TEST_ROOT/post-identity-replacement.conf" \
        "$TEST_ROOT/post-identity-target.conf"
    fi
    return 0
  }
  RPM_TEST_MODE=unowned diagnose_existing_file \
    "$TEST_ROOT/post-identity-target.conf" \
    "$TEST_ROOT/expected.conf"
)
grep -Fxq \
  "substituição posterior à identidade" \
  "$TEST_ROOT/post-identity-target.conf"

mkdir -m 0777 -- "$TEST_ROOT/unsafe-rpm-ancestor"
mkdir -m 0700 -- "$TEST_ROOT/unsafe-rpm-ancestor/rpmdb"
touch -- "$TEST_ROOT/unsafe-rpm-ancestor/rpmdb/.rpm.lock"
chmod 0600 -- "$TEST_ROOT/unsafe-rpm-ancestor/rpmdb/.rpm.lock"
if (
  RPM_LOCK_PATH="$TEST_ROOT/unsafe-rpm-ancestor/rpmdb/.rpm.lock"
  RPM_LOCK_PATH_ANCHOR="$TEST_ROOT"
  acquire_rpm_read_lock
); then
  printf 'acquire_rpm_read_lock accepted a writable intermediate ancestor\n' >&2
  exit 1
fi

mkdir -m 0777 -- "$TEST_ROOT/unsafe-action-ancestor"
mkdir -m 0700 -- "$TEST_ROOT/unsafe-action-ancestor/locks"
if (
  ACTION_LOCK_PATH="$TEST_ROOT/unsafe-action-ancestor/locks/browser.lock"
  ACTION_LOCK_PATH_ANCHOR="$TEST_ROOT"
  ACTION_LOCK_FD=""
  acquire_action_lock
); then
  printf 'acquire_action_lock accepted a writable intermediate ancestor\n' >&2
  exit 1
fi
[[ ! -e "$TEST_ROOT/unsafe-action-ancestor/locks/browser.lock" ]]

printf '%s\n' "configuração administrativa preservada" > "$TEST_ROOT/legacy.conf"
write_managed_file_if_legacy_absent \
  "$TEST_ROOT/legacy.conf" \
  "$TEST_ROOT/replacement.conf" \
  "$TEST_ROOT/expected.conf"
[[ ! -e "$TEST_ROOT/replacement.conf" ]]

cp "$TEST_ROOT/expected.conf" "$TEST_ROOT/replacement.conf"
if (write_managed_file_if_legacy_absent \
  "$TEST_ROOT/legacy.conf" \
  "$TEST_ROOT/replacement.conf" \
  "$TEST_ROOT/expected.conf") >/dev/null 2>&1; then
  printf 'write_managed_file_if_legacy_absent accepted duplicate administrative files\n' >&2
  exit 1
fi
grep -Fxq "configuração administrativa preservada" "$TEST_ROOT/legacy.conf"
cmp -s -- "$TEST_ROOT/replacement.conf" "$TEST_ROOT/expected.conf"

rm -f -- "$TEST_ROOT/legacy.conf"
rm -f -- "$TEST_ROOT/replacement.conf"
write_managed_file_if_legacy_absent \
  "$TEST_ROOT/legacy.conf" \
  "$TEST_ROOT/replacement.conf" \
  "$TEST_ROOT/expected.conf"
cmp -s -- "$TEST_ROOT/replacement.conf" "$TEST_ROOT/expected.conf"

transaction_legacy="$TEST_ROOT/transaction-legacy.conf"
transaction_target="$TEST_ROOT/transaction-managed.conf"
transaction_lock_marker="$TEST_ROOT/transaction-lock-held"
(
  ln() {
    local destination="${*: -1}"

    if [[ "$destination" == "$transaction_target" ]]; then
      if "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$RPM_LOCK_PATH" true; then
        printf 'the RPM read lock was not held across create-only publication\n' >&2
        return 1
      fi
      if "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$ACTION_LOCK_PATH" true; then
        printf 'the private action lock was not held across publication\n' >&2
        return 1
      fi
      printf '%s\n' locked > "$transaction_lock_marker"
    fi
    /usr/bin/ln "$@"
  }
  write_managed_file_if_legacy_absent \
    "$transaction_legacy" \
    "$transaction_target" \
    "$TEST_ROOT/expected.conf"
)
grep -Fxq locked "$transaction_lock_marker"
cmp -s -- "$transaction_target" "$TEST_ROOT/expected.conf"
[[ ! -e "$transaction_legacy" ]]

concurrent_legacy="$TEST_ROOT/concurrent-legacy.conf"
concurrent_target="$TEST_ROOT/concurrent-managed.conf"
if (
  ln() {
    local destination="${*: -1}"

    if [[ "$destination" == "$concurrent_target" ]]; then
      printf '%s\n' "legado administrativo concorrente" > "$concurrent_legacy"
    fi
    /usr/bin/ln "$@"
  }
  write_managed_file_if_legacy_absent \
    "$concurrent_legacy" \
    "$concurrent_target" \
    "$TEST_ROOT/expected.conf"
) >/dev/null 2>&1; then
  printf 'concurrent legacy creation was not reported as a conflict\n' >&2
  exit 1
fi
grep -Fxq "legado administrativo concorrente" "$concurrent_legacy"
cmp -s -- "$concurrent_target" "$TEST_ROOT/expected.conf"

mkdir -p "$TEST_ROOT/modprobe-a" "$TEST_ROOT/modprobe-b"
printf '%s\n' 'options v4l2loopback exclusive_caps=1' > "$TEST_ROOT/modprobe-a/vendor.conf"
(
  acquire_rpm_read_lock
  v4l2loopback_option_exists_outside_managed \
    "$TEST_ROOT/modprobe-b/managed.conf" \
    "$TEST_ROOT/modprobe-a" \
    "$TEST_ROOT/modprobe-b"
)
printf '%s\n' 'options v4l2loopback exclusive_caps=1' > "$TEST_ROOT/modprobe-b/managed.conf"
if (
  acquire_rpm_read_lock
  v4l2loopback_option_exists_outside_managed \
    "$TEST_ROOT/modprobe-b/managed.conf" \
    "$TEST_ROOT/modprobe-b"
); then
  printf 'managed v4l2loopback file was treated as a duplicate of itself\n' >&2
  exit 1
fi

rm -f -- "$TEST_ROOT/modprobe-a/vendor.conf" "$TEST_ROOT/modprobe-b/managed.conf"
v4l2_transaction_target="$TEST_ROOT/modprobe-b/managed.conf"
v4l2_transaction_vendor="$TEST_ROOT/modprobe-a/concurrent-vendor.conf"
v4l2_transaction_lock_marker="$TEST_ROOT/v4l2-transaction-lock-held"
(
  ln() {
    local destination="${*: -1}"

    if [[ "$destination" == "$v4l2_transaction_target" ]]; then
      if "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$RPM_LOCK_PATH" true; then
        printf 'the RPM read lock was not held across the v4l2loopback scan and publication\n' >&2
        return 1
      fi
      printf '%s\n' locked > "$v4l2_transaction_lock_marker"
    fi
    /usr/bin/ln "$@"
  }
  reconcile_v4l2loopback_option \
    "$v4l2_transaction_target" \
    "$TEST_ROOT/expected.conf" \
    "$TEST_ROOT/modprobe-a" \
    "$TEST_ROOT/modprobe-b"
)
grep -Fxq locked "$v4l2_transaction_lock_marker"
cmp -s -- "$v4l2_transaction_target" "$TEST_ROOT/expected.conf"
[[ ! -e "$v4l2_transaction_vendor" ]]

rm -f -- "$v4l2_transaction_target"
if (
  ln() {
    local destination="${*: -1}"

    if [[ "$destination" == "$v4l2_transaction_target" ]]; then
      printf '%s\n' 'options v4l2loopback devices=2' \
        > "$v4l2_transaction_vendor"
    fi
    /usr/bin/ln "$@"
  }
  reconcile_v4l2loopback_option \
    "$v4l2_transaction_target" \
    "$TEST_ROOT/expected.conf" \
    "$TEST_ROOT/modprobe-a" \
    "$TEST_ROOT/modprobe-b"
) >/dev/null 2>&1; then
  printf 'a concurrent v4l2loopback option was not reported as a conflict\n' >&2
  exit 1
fi
grep -Fxq 'options v4l2loopback devices=2' "$v4l2_transaction_vendor"
cmp -s -- "$v4l2_transaction_target" "$TEST_ROOT/expected.conf"

rm -f -- "$v4l2_transaction_vendor" "$v4l2_transaction_target"
mkdir -m 0777 -- "$TEST_ROOT/unsafe-modprobe-scan"
if (reconcile_v4l2loopback_option \
  "$v4l2_transaction_target" \
  "$TEST_ROOT/expected.conf" \
  "$TEST_ROOT/modprobe-a" \
  "$TEST_ROOT/unsafe-modprobe-scan") >/dev/null 2>&1; then
  printf 'an unsafe v4l2loopback scan directory was accepted\n' >&2
  exit 1
fi
[[ ! -e "$v4l2_transaction_target" ]]

rm -f -- "$v4l2_transaction_target"
printf '%s\n' 'options v4l2loopback exclusive_caps=1' \
  > "$TEST_ROOT/modprobe-a/00-safe.conf"
ln -s "$TEST_ROOT/expected.conf" "$TEST_ROOT/modprobe-a/zz-symlink.conf"
if (reconcile_v4l2loopback_option \
  "$v4l2_transaction_target" \
  "$TEST_ROOT/expected.conf" \
  "$TEST_ROOT/modprobe-a" \
  "$TEST_ROOT/modprobe-b") >/dev/null 2>&1; then
  printf 'a symlinked external v4l2loopback configuration was accepted\n' >&2
  exit 1
fi
[[ ! -e "$v4l2_transaction_target" ]]
rm -f -- \
  "$TEST_ROOT/modprobe-a/00-safe.conf" \
  "$TEST_ROOT/modprobe-a/zz-symlink.conf"

printf '%s\n' 'options v4l2loopback devices=2' \
  > "$TEST_ROOT/modprobe-a/writable.conf"
chmod 0666 -- "$TEST_ROOT/modprobe-a/writable.conf"
if (reconcile_v4l2loopback_option \
  "$v4l2_transaction_target" \
  "$TEST_ROOT/expected.conf" \
  "$TEST_ROOT/modprobe-a" \
  "$TEST_ROOT/modprobe-b") >/dev/null 2>&1; then
  printf 'a writable external v4l2loopback configuration was accepted\n' >&2
  exit 1
fi
[[ ! -e "$v4l2_transaction_target" ]]
rm -f -- "$TEST_ROOT/modprobe-a/writable.conf"

bridge_directory="$TEST_ROOT/v4l2-relayd.d"
bridge_configuration="$bridge_directory/icamerasrc.conf"
bridge_source="$TEST_ROOT/bridge-source.conf"
bridge_side_effects="$TEST_ROOT/bridge-side-effects"
mkdir -m 0755 -- "$bridge_directory"
printf '%s\n' 'safe local bridge configuration' > "$bridge_source"
printf '%s\n' 'private administrative bridge configuration' \
  > "$bridge_configuration"
chmod 0640 -- "$bridge_configuration"
prepare_bridge_configuration "$bridge_configuration" "$bridge_source"
grep -Fxq 'private administrative bridge configuration' "$bridge_configuration"

chmod 0666 -- "$bridge_configuration"
if (
  udevadm() { printf '%s\n' udevadm >> "$bridge_side_effects"; }
  systemctl() { printf '%s\n' systemctl >> "$bridge_side_effects"; }
  prepare_bridge_configuration "$bridge_configuration" "$bridge_source"
  udevadm control --reload-rules
  systemctl restart v4l2-relayd@icamerasrc.service
) >/dev/null 2>&1; then
  printf 'a writable bridge configuration was accepted\n' >&2
  exit 1
fi
[[ ! -e "$bridge_side_effects" ]]
chmod 0640 -- "$bridge_configuration"

mkdir -m 0755 -- "$TEST_ROOT/arbitrary-bridge-targets"
printf '%s\n' arbitrary > "$TEST_ROOT/arbitrary-bridge-targets/config"
rm -f -- "$bridge_configuration"
ln -s "$TEST_ROOT/arbitrary-bridge-targets/config" "$bridge_configuration"
if (
  udevadm() { printf '%s\n' udevadm >> "$bridge_side_effects"; }
  systemctl() { printf '%s\n' systemctl >> "$bridge_side_effects"; }
  prepare_bridge_configuration "$bridge_configuration" "$bridge_source"
  udevadm control --reload-rules
  systemctl restart v4l2-relayd@icamerasrc.service
) >/dev/null 2>&1; then
  printf 'an arbitrary bridge symlink was accepted\n' >&2
  exit 1
fi
[[ ! -e "$bridge_side_effects" ]]

rm -f -- "$bridge_configuration"
printf '%s\n' regular > "$bridge_configuration"
chmod 0640 -- "$bridge_configuration"
chmod 0777 -- "$bridge_directory"
if (
  udevadm() { printf '%s\n' udevadm >> "$bridge_side_effects"; }
  systemctl() { printf '%s\n' systemctl >> "$bridge_side_effects"; }
  prepare_bridge_configuration "$bridge_configuration" "$bridge_source"
  udevadm control --reload-rules
  systemctl restart v4l2-relayd@icamerasrc.service
) >/dev/null 2>&1; then
  printf 'a bridge configuration under a writable directory was accepted\n' >&2
  exit 1
fi
[[ ! -e "$bridge_side_effects" ]]
chmod 0755 -- "$bridge_directory"

fedora_runtime_directory="$TEST_ROOT/run"
fedora_defaults_root="$TEST_ROOT/defaults/etc/camera"
fedora_target_directory="$fedora_defaults_root/ipu6epmtl"
fedora_target="$fedora_target_directory/v4l2-relayd"
mkdir -m 0755 -- \
  "$fedora_runtime_directory" \
  "$TEST_ROOT/defaults" \
  "$TEST_ROOT/defaults/etc" \
  "$TEST_ROOT/defaults/etc/camera" \
  "$fedora_target_directory"
printf '%s\n' 'Fedora packaged bridge configuration' > "$fedora_target"
chmod 0644 -- "$fedora_target"
ln -- "$fedora_target" "$fedora_target_directory/v4l2-relayd-hardlink"
BRIDGE_RUNTIME_LINK="$fedora_runtime_directory/v4l2-relayd"
BRIDGE_DEFAULTS_ROOT="$fedora_defaults_root"
ln -s "$fedora_target" "$BRIDGE_RUNTIME_LINK"
rm -f -- "$bridge_configuration"
ln -s "$BRIDGE_RUNTIME_LINK" "$bridge_configuration"
if (
  udevadm() { printf '%s\n' udevadm >> "$bridge_side_effects"; }
  systemctl() { printf '%s\n' systemctl >> "$bridge_side_effects"; }
  FEDORA_BRIDGE_LINK="$bridge_configuration" \
  FEDORA_BRIDGE_TARGET="$fedora_target" \
  RPM_TEST_MODE=unavailable \
    prepare_bridge_configuration "$bridge_configuration" "$bridge_source"
  udevadm control --reload-rules
  systemctl restart v4l2-relayd@icamerasrc.service
) >/dev/null 2>&1; then
  printf 'a Fedora bridge chain without conclusive RPM ownership was accepted\n' >&2
  exit 1
fi
[[ ! -e "$bridge_side_effects" ]]

FEDORA_BRIDGE_LINK="$bridge_configuration" \
FEDORA_BRIDGE_TARGET="$fedora_target" \
RPM_TEST_MODE=fedora_bridge \
  prepare_bridge_configuration "$bridge_configuration" "$bridge_source"
[[ -L "$bridge_configuration" && -L "$BRIDGE_RUNTIME_LINK" ]]
grep -Fxq 'Fedora packaged bridge configuration' "$fedora_target"

test_browser_helper_acquires_global_before_private_and_rpm_locks
test_browser_helper_acquires_or_reuses_global_before_private_work
test_browser_helper_stops_before_private_work_when_global_fails

# Use only a synthetic sysfs tree and command fakes; never activate host units.
(
  devices="$TEST_ROOT/i2c-devices"
  mkdir -p "$devices" "$TEST_ROOT/ov-driver"
  require_bound_ov_sensor_if_present "$devices"
  mkdir "$devices/i2c-OVTI02C1:00"
  if (require_bound_ov_sensor_if_present "$devices") 2>"$TEST_ROOT/unbound-error"; then
    exit 1
  fi
  grep -Fq 'sem driver' "$TEST_ROOT/unbound-error"
  # Exercise main's preflight with the real selector before any config or
  # service operation. Poison the first write and all runtime commands.
  fixture_sys="$TEST_ROOT/relay-sys"
  mkdir -p "$fixture_sys/bus/i2c"
  ln -s "$devices" "$fixture_sys/bus/i2c/devices"
  printf '%s\n' 'acquire_kmod_lifecycle_lock() { :; }' 'validate_kmod_lifecycle_lock() { :; }' > "$TEST_ROOT/relay-lock"
  status=0
  GALAXYBOOK_SYS_ROOT="$fixture_sys" bash -c '
    source "$1"
    lifecycle_helper="$2"
    caller_is_root() { :; }
    require_command() { :; }
    acquire_action_lock() { :; }
    mktemp() { exit 91; }
    udevadm() { exit 92; }
    systemctl() { exit 93; }
    main
  ' _ "$REPO_ROOT/scripts/configure-browser-camera.sh" "$TEST_ROOT/relay-lock" 2>"$TEST_ROOT/relay-main.err" || status=$?
  [[ "$status" == 1 ]]
  grep -Fq 'sem driver' "$TEST_ROOT/relay-main.err"
  # A second bound sensor must win over an earlier, unbound match.
  mkdir "$devices/i2c-OVTI02C1:01"
  ln -s "$TEST_ROOT/ov-driver" "$devices/i2c-OVTI02C1:01/driver"
  require_bound_ov_sensor_if_present "$devices"
)

for scenario in fresh healthy stuck fresh_no_capture failed_recovery lost_lock; do
  (
    trace="$TEST_ROOT/relay-$scenario"
    : > "$trace"
    instance=v4l2-relayd@icamerasrc.service
    systemctl() {
      case "$*" in
        "is-active --quiet $instance")
          if [[ "$scenario" == fresh* ]]; then grep -Fxq enabled "$trace"; fi ;;
        "enable --now v4l2-relayd.service $instance") echo enabled >> "$trace" ;;
        "restart $instance") echo restarted >> "$trace" ;;
        *) exit 90 ;;
      esac
    }
    wait_for_virtual_capture() {
      echo probed >> "$trace"
      case "$scenario" in
        stuck) grep -Fxq restarted "$trace" || return 1 ;;
        fresh_no_capture|failed_recovery) return 1 ;;
      esac
      echo /dev/test-camera
    }
    validate_kmod_lifecycle_lock() {
      echo validated >> "$trace"
      [[ "$scenario" != lost_lock ]]
    }
    status=0
    (activate_browser_relay) >"$trace.out" 2>"$trace.err" || status=$?
    [[ "$(grep -c '^enabled$' "$trace")" == 1 ]]
    case "$scenario" in
      stuck|failed_recovery) restarts=1; probes=2 ;;
      *) restarts=0; probes=1 ;;
    esac
    [[ "$(grep -c '^restarted$' "$trace" || :)" == "$restarts" ]]
    [[ "$(grep -c '^probed$' "$trace")" == "$probes" ]]
    case "$scenario" in
      healthy|fresh|stuck)
        [[ "$status" == 0 ]]
        grep -Fxq validated "$trace"
        grep -Fq '/dev/test-camera' "$trace.out" ;;
      *) [[ "$status" != 0 && ! -s "$trace.out" ]] ;;
    esac
  )
done
