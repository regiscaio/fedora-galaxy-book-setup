#!/usr/bin/env bash

set -euo pipefail

readonly TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "$TEST_DIR/.." && pwd)"

# shellcheck source=../scripts/repair-nvidia-kmod.sh
source "$REPO_ROOT/scripts/repair-nvidia-kmod.sh"

test_production_paths_and_kernel_ignore_the_environment() {
  env \
    nvidia_script=/tmp/unsafe-nvidia \
    lifecycle_helper=/tmp/unsafe-lifecycle \
    systemd_inhibit_command=/tmp/unsafe-inhibit \
    systemd_inhibit_stat_command=/tmp/unsafe-stat \
    nvidia_inhibitor_parent_fixture=1 \
    nvidia_kernel_fixture=unsafe \
    modules_root=/tmp/unsafe-modules \
    rpm_lock_path=/tmp/unsafe-rpm-lock \
    bash --noprofile --norc -c '
      set -euo pipefail
      source "$1"
      [[ "$nvidia_script" == /usr/libexec/galaxybook-setup/repair-nvidia-kmod ]]
      [[ "$lifecycle_helper" == /usr/libexec/galaxybook-setup/kmod-lifecycle-lock.sh ]]
      [[ "$systemd_inhibit_command" == /usr/bin/systemd-inhibit ]]
      [[ "$systemd_inhibit_stat_command" == /usr/bin/stat ]]
      [[ -z "$nvidia_inhibitor_parent_fixture" ]]
      [[ -z "$nvidia_kernel_fixture" ]]
      [[ "$modules_root" == /lib/modules ]]
      [[ "$rpm_lock_path" == /usr/lib/sysimage/rpm/.rpm.lock ]]
    ' bash "$REPO_ROOT/scripts/repair-nvidia-kmod.sh"
}

test_real_systemd_inhibitor_parent_identity_when_available() {
  if [[ ! -x /usr/bin/systemd-inhibit ]] \
    || ! /usr/bin/systemd-inhibit --list --no-pager >/dev/null 2>&1; then
    printf 'repair-nvidia real inhibitor identity: SKIP (logind unavailable)\n'
    return 0
  fi

  if (inhibitor_parent_is_trusted) >/dev/null 2>&1; then
    printf 'accepted a direct process as the NVIDIA systemd-inhibit parent\n' >&2
    return 1
  fi
  /usr/bin/systemd-inhibit \
    --mode=block \
    --what=idle \
    --who='Galaxy Book Setup tests' \
    --why='Validating the NVIDIA maintenance parent without taking production locks' \
    /usr/bin/bash --noprofile --norc -c '
      set -euo pipefail
      source "$1"
      inhibitor_parent_is_trusted
    ' bash "$REPO_ROOT/scripts/repair-nvidia-kmod.sh"
}

test_kernel_names_are_strict() {
  nvidia_kernel_fixture='7.1.8-200.fc44.x86_64'
  [[ "$(running_kernel)" == '7.1.8-200.fc44.x86_64' ]]
  for unsafe in '../kernel' 'kernel-without-leading-digit' '7.1.8-' \
    $'7.1.8\nsecond' '7.1.8/other'; do
    if (
      nvidia_kernel_fixture="$unsafe"
      running_kernel
    ) >/dev/null 2>&1; then
      printf 'accepted unsafe NVIDIA kernel release: %q\n' "$unsafe" >&2
      return 1
    fi
  done
  nvidia_kernel_fixture=""
}

test_running_kernel_probe_fails_closed() {
  nvidia_kernel_fixture=""

  [[ "$(
    uname() { printf '%s\n' '7.1.8-200.fc44.x86_64'; }
    running_kernel
  )" == '7.1.8-200.fc44.x86_64' ]]

  if (
    uname() { printf '%s\n' '7.1.8-200.fc44.x86_64'; return 1; }
    running_kernel
  ) >/dev/null 2>&1; then
    printf 'accepted failed uname output\n' >&2
    return 1
  fi
  if (
    uname() {
      printf '%s\n' '7.1.8-200.fc44.x86_64'
      printf '%s\n' warning >&2
    }
    running_kernel
  ) >/dev/null 2>&1; then
    printf 'accepted uname stderr\n' >&2
    return 1
  fi
  if (
    uname() { printf '%s' '7.1.8-200.fc44.x86_64'; }
    running_kernel
  ) >/dev/null 2>&1; then
    printf 'accepted truncated uname output\n' >&2
    return 1
  fi
  if (
    uname() { printf '%s\n' '7.1.8-200.fc44.x86_64' '7.1.7-200.fc44.x86_64'; }
    running_kernel
  ) >/dev/null 2>&1; then
    printf 'accepted multiline uname output\n' >&2
    return 1
  fi
}

prepare_rpm_lock_fixture() {
  local root="$1"

  mkdir -p -- "$root/usr/lib/sysimage/rpm"
  chmod 0755 -- "$root" "$root/usr" "$root/usr/lib" \
    "$root/usr/lib/sysimage" "$root/usr/lib/sysimage/rpm"
  rpm_lock_path="$root/usr/lib/sysimage/rpm/.rpm.lock"
  rpm_lock_ancestry=(
    "$root"
    "$root/usr"
    "$root/usr/lib"
    "$root/usr/lib/sysimage"
    "$root/usr/lib/sysimage/rpm"
  )
  rpm_lock_expected_uid="$EUID"
  rpm_lock_expected_gid="$(id -g)"
  rpm_lock_timeout_seconds="1"
  (umask 022; : > "$rpm_lock_path")
}

test_rpm_read_lock_rejects_unsafe_state() {
  local test_root hardlink

  test_root="$(mktemp -d)"
  prepare_rpm_lock_fixture "$test_root"

  chmod 0666 -- "$rpm_lock_path"
  if (acquire_rpm_read_lock) >/dev/null 2>&1; then
    printf 'accepted writable RPM lock\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  chmod 0644 -- "$rpm_lock_path"

  hardlink="$test_root/rpm-hardlink"
  ln -- "$rpm_lock_path" "$hardlink"
  if (acquire_rpm_read_lock) >/dev/null 2>&1; then
    printf 'accepted hard-linked RPM lock\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  unlink -- "$hardlink"

  chmod 0777 -- "$test_root/usr/lib/sysimage"
  if (acquire_rpm_read_lock) >/dev/null 2>&1; then
    printf 'accepted unsafe RPM lock ancestry\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  chmod 0755 -- "$test_root/usr/lib/sysimage"

  mv -- "$rpm_lock_path" "$rpm_lock_path.regular"
  ln -s -- "$rpm_lock_path.regular" "$rpm_lock_path"
  if (acquire_rpm_read_lock) >/dev/null 2>&1; then
    printf 'accepted RPM lock symlink\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi

  rm -rf -- "$test_root"
}

test_rpm_read_lock_has_bounded_contention() {
  local test_root holder_ready holder_release holder_pid status start_ns elapsed_ms

  test_root="$(mktemp -d)"
  prepare_rpm_lock_fixture "$test_root"
  holder_ready="$test_root/holder.ready"
  holder_release="$test_root/holder.release"

  (
    flock --fcntl --exclusive "$rpm_lock_path" bash -c '
      : > "$1"
      while [[ ! -e "$2" ]]; do sleep 0.01; done
    ' bash "$holder_ready" "$holder_release"
  ) &
  holder_pid=$!
  for ((attempt = 1; attempt <= 200; attempt++)); do
    [[ -e "$holder_ready" ]] && break
    sleep 0.01
  done
  [[ -e "$holder_ready" ]]

  start_ns="$(date +%s%N)"
  if (acquire_rpm_read_lock) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  elapsed_ms=$(( ($(date +%s%N) - start_ns) / 1000000 ))
  : > "$holder_release"
  wait "$holder_pid"
  [[ "$status" -ne 0 ]]
  (( elapsed_ms >= 800 && elapsed_ms <= 3000 ))

  rm -rf -- "$test_root"
}

test_inhibitor_wraps_nvidia_helper() {
  local test_root record

  test_root="$(mktemp -d)"
  record="$test_root/arguments"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf "%s\n" "$@" >> "$NVIDIA_INHIBITOR_RECORD"' \
    > "$test_root/systemd-inhibit"
  chmod 0755 "$test_root/systemd-inhibit"

  (
    PATH="$test_root:$PATH"
    export PATH NVIDIA_INHIBITOR_RECORD="$record"
    systemd_inhibit_command="$test_root/systemd-inhibit"
    nvidia_script="$REPO_ROOT/scripts/repair-nvidia-kmod.sh"
    enter_systemd_inhibitor
  )

  grep -Fxq -- '--mode=block' "$record"
  grep -Fxq -- '--what=shutdown:sleep:idle' "$record"
  grep -Fxq -- '--who=Galaxy Book Setup' "$record"
  grep -Fxq -- '--why=Gerando e finalizando o módulo NVIDIA para o kernel atual' "$record"
  grep -Fxq -- "$REPO_ROOT/scripts/repair-nvidia-kmod.sh" "$record"
  grep -Fxq -- '--inhibited' "$record"
  rm -rf -- "$test_root"
}

test_nvidia_maintenance_holds_global_then_rpm_locks() {
  local test_root trace

  test_root="$(mktemp -d)"
  trace="$test_root/trace"
  prepare_rpm_lock_fixture "$test_root"
  mkdir -p -- "$test_root/modules/7.1.8-200.fc44.x86_64"
  printf '%s\n' \
    'acquire_kmod_lifecycle_lock() {' \
    '  printf "GLOBAL_ACQUIRE\n" >> "$NVIDIA_TEST_TRACE"' \
    '  export GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD=91' \
    '}' \
    'validate_kmod_lifecycle_lock() {' \
    '  printf "GLOBAL_VALIDATE\n" >> "$NVIDIA_TEST_TRACE"' \
    '}' \
    > "$test_root/lifecycle-helper"

  (
    caller_is_root() { return 0; }
    dnf() { printf 'DNF:%s\n' "$*" >> "$NVIDIA_TEST_TRACE"; }
    akmods() { printf 'AKMODS:%s\n' "$*" >> "$NVIDIA_TEST_TRACE"; }
    depmod() {
      validate_rpm_read_lock
      validate_kmod_lifecycle_lock
      printf 'DEPMOD:%s\n' "$*" >> "$NVIDIA_TEST_TRACE"
    }
    dracut() {
      validate_rpm_read_lock
      validate_kmod_lifecycle_lock
      printf 'DRACUT:%s\n' "$*" >> "$NVIDIA_TEST_TRACE"
    }
    export NVIDIA_TEST_TRACE="$trace"
    nvidia_inhibitor_parent_fixture=1
    lifecycle_helper="$test_root/lifecycle-helper"
    modules_root="$test_root/modules"
    nvidia_kernel_fixture='7.1.8-200.fc44.x86_64'
    main --inhibited
  )

  grep -Fxq 'DNF:install -y akmod-nvidia' "$trace"
  grep -Fxq 'AKMODS:--force --akmod nvidia --kernels 7.1.8-200.fc44.x86_64' "$trace"
  grep -Fxq 'DEPMOD:-a 7.1.8-200.fc44.x86_64' "$trace"
  grep -Fxq 'DRACUT:--force --kver 7.1.8-200.fc44.x86_64' "$trace"
  [[ "$(grep -c '^GLOBAL_ACQUIRE$' "$trace")" -eq 1 ]]

  local global_line dnf_line akmods_line depmod_line dracut_line
  global_line="$(grep -n '^GLOBAL_ACQUIRE$' "$trace" | cut -d: -f1)"
  dnf_line="$(grep -n '^DNF:' "$trace" | cut -d: -f1)"
  akmods_line="$(grep -n '^AKMODS:' "$trace" | cut -d: -f1)"
  depmod_line="$(grep -n '^DEPMOD:' "$trace" | cut -d: -f1)"
  dracut_line="$(grep -n '^DRACUT:' "$trace" | cut -d: -f1)"
  (( global_line < dnf_line && dnf_line < akmods_line \
    && akmods_line < depmod_line && depmod_line < dracut_line ))

  rm -rf -- "$test_root"
}

test_nvidia_never_mutates_before_global_lock() {
  local test_root trace status

  test_root="$(mktemp -d)"
  trace="$test_root/trace"
  printf '%s\n' \
    'acquire_kmod_lifecycle_lock() { return 75; }' \
    'validate_kmod_lifecycle_lock() { return 1; }' \
    > "$test_root/lifecycle-helper"

  if (
    caller_is_root() { return 0; }
    dnf() { printf 'DNF\n' >> "$NVIDIA_TEST_TRACE"; }
    akmods() { printf 'AKMODS\n' >> "$NVIDIA_TEST_TRACE"; }
    depmod() { printf 'DEPMOD\n' >> "$NVIDIA_TEST_TRACE"; }
    dracut() { printf 'DRACUT\n' >> "$NVIDIA_TEST_TRACE"; }
    export NVIDIA_TEST_TRACE="$trace"
    nvidia_inhibitor_parent_fixture=1
    lifecycle_helper="$test_root/lifecycle-helper"
    main --inhibited
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  [[ "$status" -ne 0 && ! -e "$trace" ]]
  rm -rf -- "$test_root"
}

test_nvidia_never_runs_depmod_without_rpm_lock() {
  local test_root trace status

  test_root="$(mktemp -d)"
  trace="$test_root/trace"
  mkdir -p -- "$test_root/modules/7.1.8-200.fc44.x86_64"
  printf '%s\n' \
    'acquire_kmod_lifecycle_lock() { export GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD=91; }' \
    'validate_kmod_lifecycle_lock() { return 0; }' \
    > "$test_root/lifecycle-helper"

  if (
    caller_is_root() { return 0; }
    dnf() { printf 'DNF\n' >> "$NVIDIA_TEST_TRACE"; }
    akmods() { printf 'AKMODS\n' >> "$NVIDIA_TEST_TRACE"; }
    acquire_rpm_read_lock() { return 76; }
    depmod() { printf 'DEPMOD\n' >> "$NVIDIA_TEST_TRACE"; }
    dracut() { printf 'DRACUT\n' >> "$NVIDIA_TEST_TRACE"; }
    export NVIDIA_TEST_TRACE="$trace"
    nvidia_inhibitor_parent_fixture=1
    lifecycle_helper="$test_root/lifecycle-helper"
    modules_root="$test_root/modules"
    nvidia_kernel_fixture='7.1.8-200.fc44.x86_64'
    main --inhibited
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  [[ "$status" -ne 0 ]]
  grep -Fxq DNF "$trace"
  grep -Fxq AKMODS "$trace"
  ! grep -Eq '^(DEPMOD|DRACUT)$' "$trace"
  rm -rf -- "$test_root"
}

test_nvidia_has_no_inline_unlocked_index_mutation() {
  local rust_source="$REPO_ROOT/src/lib.rs"
  local helper="$REPO_ROOT/scripts/repair-nvidia-kmod.sh"
  local rpm_acquire depmod_line dracut_line

  grep -Fq 'exec /usr/libexec/galaxybook-setup/repair-nvidia-kmod' "$rust_source"
  rpm_acquire="$(grep -n '^[[:space:]]*acquire_rpm_read_lock$' "$helper" | tail -1 | cut -d: -f1)"
  depmod_line="$(grep -n '^[[:space:]]*depmod -a "\$kernel"$' "$helper" | cut -d: -f1)"
  dracut_line="$(grep -n '^[[:space:]]*dracut --force --kver "\$kernel"$' "$helper" | cut -d: -f1)"
  [[ -n "$rpm_acquire" && -n "$depmod_line" && -n "$dracut_line" ]]
  (( rpm_acquire < depmod_line && depmod_line < dracut_line ))
}

test_production_paths_and_kernel_ignore_the_environment
test_real_systemd_inhibitor_parent_identity_when_available
test_kernel_names_are_strict
test_running_kernel_probe_fails_closed
test_rpm_read_lock_rejects_unsafe_state
test_rpm_read_lock_has_bounded_contention
test_inhibitor_wraps_nvidia_helper
test_nvidia_maintenance_holds_global_then_rpm_locks
test_nvidia_never_mutates_before_global_lock
test_nvidia_never_runs_depmod_without_rpm_lock
test_nvidia_has_no_inline_unlocked_index_mutation

printf 'repair-nvidia-kmod tests: ok\n'
