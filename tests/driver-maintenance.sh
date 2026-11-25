#!/usr/bin/env bash

set -euo pipefail

readonly TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "$TEST_DIR/.." && pwd)"
readonly HELPER="$REPO_ROOT/scripts/driver-maintenance.sh"

# shellcheck source=../scripts/driver-maintenance.sh
source "$HELPER"

test_production_paths_ignore_the_environment() {
  env \
    maintenance_script=/tmp/unsafe-maintenance \
    lifecycle_helper=/tmp/unsafe-lifecycle \
    reconcile_helper=/tmp/unsafe-reconcile \
    browser_camera_helper=/tmp/unsafe-browser \
    systemd_inhibit_command=/tmp/unsafe-inhibit \
    maintenance_inhibitor_parent_fixture=1 \
    bash --noprofile --norc -c '
      set -euo pipefail
      source "$1"
      [[ "$maintenance_script" == /usr/libexec/galaxybook-setup/driver-maintenance ]]
      [[ "$lifecycle_helper" == /usr/libexec/galaxybook-setup/kmod-lifecycle-lock.sh ]]
      [[ "$reconcile_helper" == /usr/libexec/galaxybook-setup/reconcile-kmods ]]
      [[ "$browser_camera_helper" == /usr/libexec/galaxybook-setup/configure-browser-camera ]]
      [[ "$systemd_inhibit_command" == /usr/bin/systemd-inhibit ]]
      [[ -z "$maintenance_inhibitor_parent_fixture" ]]
    ' bash "$HELPER"
}

test_real_systemd_inhibitor_parent_identity_when_available() {
  if [[ ! -x /usr/bin/systemd-inhibit ]] \
    || ! /usr/bin/systemd-inhibit --list --no-pager >/dev/null 2>&1; then
    printf 'driver-maintenance real inhibitor identity: SKIP (logind unavailable)\n'
    return 0
  fi

  if (inhibitor_parent_is_trusted) >/dev/null 2>&1; then
    printf 'accepted a direct process as the systemd-inhibit parent\n' >&2
    return 1
  fi
  /usr/bin/systemd-inhibit \
    --mode=block \
    --what=idle \
    --who='Galaxy Book Setup tests' \
    --why='Validating the maintenance parent without taking production locks' \
    /usr/bin/bash --noprofile --norc -c '
      set -euo pipefail
      source "$1"
      inhibitor_parent_is_trusted
      exec /usr/bin/bash --noprofile --norc -c '\''
        set -euo pipefail
        source "$1"
        inhibitor_parent_is_trusted
      '\'' bash "$2"
    ' bash "$HELPER" "$REPO_ROOT/scripts/reconcile-kmods.sh"
}

test_all_public_modes_delegate_to_one_coordinator() {
  local rust_source="$REPO_ROOT/src/lib.rs"
  local runtime_source="$REPO_ROOT/src/actions/runtime.rs"

  grep -Fq 'exec /usr/libexec/galaxybook-setup/driver-maintenance install-main' \
    "$rust_source"
  grep -Fq 'exec /usr/libexec/galaxybook-setup/driver-maintenance install-camera' \
    "$rust_source"
  grep -Fq 'exec /usr/libexec/galaxybook-setup/driver-maintenance enable-browser-camera' \
    "$rust_source"
  grep -Fq 'exec /usr/libexec/galaxybook-setup/driver-maintenance enable-speakers' \
    "$rust_source"
  grep -Fq 'exec /usr/libexec/galaxybook-setup/driver-maintenance upgrade' \
    "$runtime_source"

  for mapping in \
    'install-main:MAIN' \
    'install-camera:CAMERA' \
    'enable-browser-camera:BROWSER' \
    'enable-speakers:SPEAKERS' \
    'upgrade:UPGRADE'; do
    mode="${mapping%%:*}"
    expected="${mapping#*:}"
    output="$({
      caller_is_root() { return 0; }
      inhibitor_parent_is_trusted() { return 0; }
      lifecycle_helper=/dev/null
      acquire_kmod_lifecycle_lock() { printf 'GLOBAL\n'; }
      validate_kmod_lifecycle_lock() { return 0; }
      install_main_support() { printf 'MAIN\n'; }
      install_camera_support() { printf 'CAMERA\n'; }
      enable_browser_camera() { printf 'BROWSER\n'; }
      enable_speakers() { printf 'SPEAKERS\n'; }
      upgrade_installed_packages() { printf 'UPGRADE\n'; }
      main --inhibited "$mode"
    })"
    [[ "$output" == $'GLOBAL\n'"$expected" ]]
  done
}

test_package_upgrade_has_no_direct_rust_bypass() {
  local -a mutating_paths

  mapfile -t mutating_paths < <(
    grep -RIl -- 'dnf --refresh upgrade -y' \
      "$REPO_ROOT/src" "$REPO_ROOT/scripts"
  )
  [[ "${#mutating_paths[@]}" -eq 1 \
    && "${mutating_paths[0]}" == "$REPO_ROOT/scripts/driver-maintenance.sh" ]]
  ! grep -R -Eq -- 'install_package_updates|PKEXEC_COMMAND' "$REPO_ROOT/src"
}

run_upgrade_fixture() (
  local installed_packages="$1"
  local post_dnf_packages="${2:-}"
  local rpm_status="${3:-0}"
  local raw_inventory="${4:-0}"
  local rpm_stderr="${5:-}"

  rpm() {
    [[ "$1" == -qa ]] || return 64
    printf '%s' "$rpm_stderr" >&2
    if [[ "$raw_inventory" == 1 ]]; then
      printf '%s' "$installed_packages"
    else
      local package
      for package in $installed_packages; do
        printf '%s\n' "$package"
      done
    fi
    return "$rpm_status"
  }
  dnf() {
    printf 'DNF:%s\n' "$*"
    if [[ -n "$post_dnf_packages" ]]; then
      installed_packages="$post_dnf_packages"
    fi
  }
  validate_kmod_lifecycle_lock() { printf 'GLOBAL_VALIDATE\n'; }
  exec_reconcile() { printf 'RECONCILE:%s\n' "$1"; }
  upgrade_installed_packages
)

test_upgrade_inventory_and_family_dispatch_are_fail_closed() {
  local output

  output="$(run_upgrade_fixture galaxybook-camera)"
  [[ "$output" == *'DNF:--refresh upgrade -y galaxybook-camera'* ]]
  [[ "$output" == *'nenhuma família completa'* ]]
  [[ "$output" != *'RECONCILE:'* ]]

  output="$(run_upgrade_fixture \
    'galaxybook-camera galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 kmod-galaxybook-ov02c10')"
  [[ "$output" == *'RECONCILE:ov02c10'* ]]

  output="$(run_upgrade_fixture \
    galaxybook-setup \
    'galaxybook-setup galaxybook-ov02c10-kmod-common akmod-galaxybook-ov02c10 kmod-galaxybook-ov02c10')"
  [[ "$output" == *'RECONCILE:ov02c10'* ]]

  if run_upgrade_fixture \
    'galaxybook-camera galaxybook-ov02c10-kmod-common' \
    >/dev/null 2>&1; then
    printf 'accepted a partially installed driver family before DNF\n' >&2
    return 1
  fi

  for shape in \
    'empty:0:0:' \
    'truncated:0:1:galaxybook-camera' \
    'malformed:0:1:invalid package
' \
    'warning:0:1:galaxybook-camera
'; do
    label="${shape%%:*}"
    remainder="${shape#*:}"
    status="${remainder%%:*}"
    remainder="${remainder#*:}"
    raw="${remainder%%:*}"
    inventory="${remainder#*:}"
    warning=""
    [[ "$label" == warning ]] && warning=$'rpm warning\n'
    if run_upgrade_fixture "$inventory" "" "$status" "$raw" "$warning" \
      >/dev/null 2>&1; then
      printf 'accepted inconclusive RPM inventory: %s\n' "$label" >&2
      return 1
    fi
  done
}

test_global_failure_stops_before_dnf() {
  local test_root trace status

  test_root="$(mktemp -d)"
  trace="$test_root/trace"
  if (
    caller_is_root() { return 0; }
    maintenance_inhibitor_parent_fixture=1
    lifecycle_helper=/dev/null
    acquire_kmod_lifecycle_lock() { return 75; }
    validate_kmod_lifecycle_lock() { return 1; }
    dnf() { printf 'DNF\n' >> "$trace"; }
    main --inhibited install-camera
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  [[ "$status" -ne 0 && ! -e "$trace" ]]
  rm -rf -- "$test_root"
}

test_inhibitor_closes_old_fds_before_global_then_preserves_global_to_reconcile() {
  local test_root lock_parent lock_path lifecycle_fixture
  local rpm_root rpm_lock_path
  local fixture_kernel="7.0.0-1.fc44.x86_64"
  local inhibitor_mock worker reconcile_mock trace sentinel_file

  test_root="$(mktemp -d /tmp/galaxybook-driver-maintenance-test.XXXXXX)"
  lock_parent="$test_root/lock"
  lock_path="$lock_parent/galaxybook-kmod-lifecycle.lock"
  lifecycle_fixture="$test_root/lifecycle-helper"
  inhibitor_mock="$test_root/systemd-inhibit"
  worker="$test_root/maintenance-worker"
  reconcile_mock="$test_root/reconcile"
  trace="$test_root/trace"
  sentinel_file="$test_root/pre-inhibitor-fd"
  rpm_root="$test_root/rpm-root"
  rpm_lock_path="$rpm_root/usr/lib/sysimage/rpm/.rpm.lock"
  mkdir -m 0755 -- "$lock_parent"
  mkdir -p -- "$rpm_root/usr/lib/sysimage/rpm"
  mkdir -p -- "$test_root/modules/$fixture_kernel"
  chmod 0755 -- "$test_root" "$rpm_root" "$rpm_root/usr" \
    "$rpm_root/usr/lib" "$rpm_root/usr/lib/sysimage" \
    "$rpm_root/usr/lib/sysimage/rpm"
  (umask 022; : > "$rpm_lock_path")
  : > "$sentinel_file"

  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'source "$FIXTURE_CANONICAL_HELPER"' \
    'kmod_lifecycle_lock_path="$FIXTURE_LOCK_PATH"' \
    'kmod_lifecycle_lock_ancestry=("$FIXTURE_TEST_ROOT" "$FIXTURE_LOCK_PARENT")' \
    'kmod_lifecycle_lock_expected_uid="$EUID"' \
    'kmod_lifecycle_lock_expected_gid="$(id -g)"' \
    'kmod_lifecycle_lock_timeout_seconds=2' \
    > "$lifecycle_fixture"

  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    'while (( $# > 0 )) && [[ "$1" == --* ]]; do shift; done' \
    'printf "INHIBITOR_NO_GLOBAL\n" >> "$FIXTURE_TRACE"' \
    '[[ -z "${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}" ]]' \
    '/usr/bin/bash --noprofile --norc -c '\''for fd_path in /proc/${BASHPID}/fd/*; do fd="${fd_path##*/}"; if [[ "$fd" =~ ^[0-9]+$ ]] && (( fd > 2 )); then eval "exec ${fd}>&-" 2>/dev/null || true; fi; done; exec "$@"'\'' bash "$@"' \
    > "$inhibitor_mock"

  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    '[[ ! -e "/proc/${BASHPID}/fd/${FIXTURE_SENTINEL_FD}" ]]' \
    'printf "AFTER_INHIBITOR_FD_CLOSED\n" >> "$FIXTURE_TRACE"' \
    'source "$FIXTURE_DRIVER_HELPER"' \
    'caller_is_root() { return 0; }' \
    'maintenance_inhibitor_parent_fixture=1' \
    'lifecycle_helper="$FIXTURE_LIFECYCLE_HELPER"' \
    'reconcile_helper="$FIXTURE_RECONCILE_HELPER"' \
    'dnf() {' \
    '  validate_kmod_lifecycle_lock' \
    '  if flock --exclusive --nonblock "$FIXTURE_LOCK_PATH" true; then exit 91; fi' \
    '  printf "DNF_LOCKED:%s\n" "$*" >> "$FIXTURE_TRACE"' \
    '}' \
    'main "$@"' \
    > "$worker"

  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'set -euo pipefail' \
    '[[ "$#" -eq 2 && "$1" == --coordinated && "$2" == ov02c10 ]]' \
    'source "$FIXTURE_RECONCILE_SOURCE"' \
    'module_tree_root="$FIXTURE_TEST_ROOT/modules"' \
    'caller_is_root() { return 0; }' \
    'reconcile_inhibitor_parent_fixture=1' \
    'lifecycle_helper="$FIXTURE_LIFECYCLE_HELPER"' \
    'rpm_lock_path="$FIXTURE_RPM_LOCK_PATH"' \
    'rpm_lock_ancestry=("$FIXTURE_RPM_ROOT" "$FIXTURE_RPM_ROOT/usr" "$FIXTURE_RPM_ROOT/usr/lib" "$FIXTURE_RPM_ROOT/usr/lib/sysimage" "$FIXTURE_RPM_ROOT/usr/lib/sysimage/rpm")' \
    'rpm_lock_expected_uid="$EUID"' \
    'rpm_lock_expected_gid="$(id -g)"' \
    'rpm_lock_timeout_seconds=2' \
    'require_command() { :; }' \
    'detect_secure_boot_state() { SECURE_BOOT_STATE=disabled; }' \
    'installed_kernels() { printf "%s\n" "$FIXTURE_KERNEL"; }' \
    'ensure_signing_key_available() { :; }' \
    'detect_akmods_signing_identity() { :; }' \
    'assert_global_lock_held() {' \
    '  validate_kmod_lifecycle_lock' \
    '  if flock --exclusive --nonblock "$FIXTURE_LOCK_PATH" true; then exit 92; fi' \
    '}' \
    'build_driver_for_kernel() {' \
    '  assert_global_lock_held' \
    '  printf "RECONCILE_BUILD_LOCKED:%s:%s\n" "$1" "$2" >> "$FIXTURE_TRACE"' \
    '}' \
    'assert_post_build_locks_held() {' \
    '  assert_global_lock_held' \
    '  validate_rpm_read_lock' \
    '  if flock --fcntl --exclusive --nonblock "$FIXTURE_RPM_LOCK_PATH" true; then exit 93; fi' \
    '}' \
    'validate_packaged_module() { assert_post_build_locks_held; }' \
    'finalize_ov_kernel() {' \
    '  assert_global_lock_held' \
    '  [[ -z "$reconcile_rpm_lock_fd" ]]' \
    '  flock --fcntl --exclusive --nonblock "$FIXTURE_RPM_LOCK_PATH" true' \
    '  printf "RECONCILE_FINALIZER_ORDERED:%s\n" "$1" >> "$FIXTURE_TRACE"' \
    '}' \
    'validate_resolved_module() { assert_post_build_locks_held; }' \
    'mok_reboot_required() { return 1; }' \
    'main "$@"' \
    > "$reconcile_mock"
  chmod 0755 -- "$inhibitor_mock" "$worker" "$reconcile_mock" "$lifecycle_fixture"

  (
    export FIXTURE_CANONICAL_HELPER="$REPO_ROOT/scripts/kmod-lifecycle-lock.sh"
    export FIXTURE_DRIVER_HELPER="$HELPER"
    export FIXTURE_LIFECYCLE_HELPER="$lifecycle_fixture"
    export FIXTURE_RECONCILE_HELPER="$reconcile_mock"
    export FIXTURE_RECONCILE_SOURCE="$REPO_ROOT/scripts/reconcile-kmods.sh"
    export FIXTURE_KERNEL="$fixture_kernel"
    export FIXTURE_LOCK_PATH="$lock_path"
    export FIXTURE_LOCK_PARENT="$lock_parent"
    export FIXTURE_RPM_ROOT="$rpm_root"
    export FIXTURE_RPM_LOCK_PATH="$rpm_lock_path"
    export FIXTURE_TEST_ROOT="$test_root"
    export FIXTURE_TRACE="$trace"
    caller_is_root() { return 0; }
    systemd_inhibit_command="$inhibitor_mock"
    maintenance_script="$worker"
    exec {sentinel_fd}<>"$sentinel_file"
    export FIXTURE_SENTINEL_FD="$sentinel_fd"
    main install-camera
  )

  [[ "$(sed -n '1p' "$trace")" == INHIBITOR_NO_GLOBAL ]]
  [[ "$(sed -n '2p' "$trace")" == AFTER_INHIBITOR_FD_CLOSED ]]
  grep -Fq 'DNF_LOCKED:--refresh install -y' "$trace"
  grep -Fxq "RECONCILE_BUILD_LOCKED:galaxybook-ov02c10:$fixture_kernel" "$trace"
  grep -Fxq "RECONCILE_FINALIZER_ORDERED:$fixture_kernel" "$trace"
  [[ "$(grep -c '^DNF_LOCKED:' "$trace")" -eq 1 ]]
  [[ "$(grep -c '^RECONCILE_BUILD_LOCKED:' "$trace")" -eq 1 ]]
  [[ "$(grep -c '^RECONCILE_FINALIZER_ORDERED:' "$trace")" -eq 1 ]]
  rm -rf -- "$test_root"
}

test_production_paths_ignore_the_environment
test_real_systemd_inhibitor_parent_identity_when_available
test_all_public_modes_delegate_to_one_coordinator
test_package_upgrade_has_no_direct_rust_bypass
test_upgrade_inventory_and_family_dispatch_are_fail_closed
test_global_failure_stops_before_dnf
test_inhibitor_closes_old_fds_before_global_then_preserves_global_to_reconcile

printf 'driver-maintenance tests: ok\n'
