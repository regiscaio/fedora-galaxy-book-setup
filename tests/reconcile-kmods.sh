#!/usr/bin/env bash

set -euo pipefail

readonly TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "$TEST_DIR/.." && pwd)"

# shellcheck source=../scripts/reconcile-kmods.sh
source "$REPO_ROOT/scripts/reconcile-kmods.sh"

SECURE_BOOT_STATE=disabled

# Focused function tests below do not execute main(), which is responsible for
# sourcing and acquiring the production lifecycle helper. Tests that exercise
# the lock itself live in tests/kmod-lifecycle-lock.sh.
validate_kmod_lifecycle_lock() { :; }

test_reconciliation_acquires_global_before_driver_work() {
  local helper="$REPO_ROOT/scripts/reconcile-kmods.sh"
  local global_acquire require initial_rpm inventory build complete_call
  local first_snapshot release_before_finalizers finalizers final_snapshot
  local -a rpm_acquisitions

  global_acquire="$(grep -n '^[[:space:]]*acquire_kmod_lifecycle_lock \\' "$helper" | tail -1 | cut -d: -f1)"
  require="$(grep -n '^[[:space:]]*require_command akmods$' "$helper" | cut -d: -f1)"
  inventory="$(grep -n '^[[:space:]]*if ! kernel_output="\$(installed_kernels)"; then$' "$helper" | cut -d: -f1)"
  build="$(grep -n '^[[:space:]]*build_driver_for_kernel "\$OV_AKMOD" "\$kernel"$' "$helper" | cut -d: -f1)"
  complete_call="$(grep -n '^[[:space:]]*complete_reconciliation "\$mode" "\$kernel_output" "\${kernels\[@\]}"$' "$helper" | cut -d: -f1)"
  mapfile -t rpm_acquisitions < <(
    grep -n '^[[:space:]]*acquire_rpm_read_lock$' "$helper" | cut -d: -f1
  )
  [[ "${#rpm_acquisitions[@]}" -eq 3 ]]
  first_snapshot="${rpm_acquisitions[0]}"
  final_snapshot="${rpm_acquisitions[1]}"
  initial_rpm="${rpm_acquisitions[2]}"
  release_before_finalizers="$(grep -n '^[[:space:]]*|| fail "Não foi possível liberar o lock RPM antes dos finalizadores\."$' "$helper" | cut -d: -f1)"
  finalizers="$(grep -n '^[[:space:]]*run_driver_finalizers "\$mode" "\${kernels\[@\]}"$' "$helper" | cut -d: -f1)"
  [[ -n "$global_acquire" && -n "$require" && -n "$inventory" \
    && -n "$build" && -n "$complete_call" \
    && -n "$initial_rpm" && -n "$release_before_finalizers" \
    && -n "$finalizers" ]]
  (( global_acquire < require && require < initial_rpm \
    && initial_rpm < inventory && inventory < build && build < complete_call \
    && first_snapshot < release_before_finalizers \
    && release_before_finalizers < finalizers \
    && finalizers < final_snapshot ))
}

prepare_reconcile_rpm_lock_fixture() {
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
  reconcile_rpm_lock_fd=""
  (umask 022; : > "$rpm_lock_path")
}

test_production_rpm_lock_contract_ignores_environment() {
  env \
    rpm_lock_path=/tmp/unsafe-rpm-lock \
    rpm_lock_timeout_seconds=1 \
    rpm_lock_expected_uid=123 \
    rpm_lock_expected_gid=456 \
    rpm_lock_ancestry=/tmp/unsafe-rpm-ancestry \
    reconcile_rpm_lock_fd=99 \
    bash --noprofile --norc -c '
      set -euo pipefail
      source "$1"
      [[ "$rpm_lock_path" == /usr/lib/sysimage/rpm/.rpm.lock ]]
      [[ "$rpm_lock_timeout_seconds" == 90 ]]
      [[ "$rpm_lock_expected_uid" == 0 ]]
      [[ "$rpm_lock_expected_gid" == 0 ]]
      [[ "${rpm_lock_ancestry[*]}" == "/usr /usr/lib /usr/lib/sysimage /usr/lib/sysimage/rpm" ]]
      [[ -z "$reconcile_rpm_lock_fd" ]]
    ' bash "$REPO_ROOT/scripts/reconcile-kmods.sh"
}

test_inhibitor_wraps_the_entire_reconciliation() {
  local test_root record

  test_root="$(mktemp -d)"
  record="$test_root/arguments"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf "%s\n" "$@" >> "$INHIBITOR_RECORD"' \
    > "$test_root/systemd-inhibit"
  chmod 0755 "$test_root/systemd-inhibit"

  (
    PATH="$test_root:$PATH"
    export PATH INHIBITOR_RECORD="$record"
    systemd_inhibit_command="$test_root/systemd-inhibit"
    reconcile_script="$REPO_ROOT/scripts/reconcile-kmods.sh"
    enter_systemd_inhibitor ov02c10
  )

  grep -Fxq -- '--mode=block' "$record"
  grep -Fxq -- '--what=shutdown:sleep:idle' "$record"
  grep -Fxq -- '--who=Galaxy Book Setup' "$record"
  grep -Fxq -- '--why=Gerando e validando módulos externos para todos os kernels' "$record"
  grep -Fxq -- "$REPO_ROOT/scripts/reconcile-kmods.sh" "$record"
  grep -Fxq -- '--inhibited' "$record"
  grep -Fxq -- 'ov02c10' "$record"

  rm -rf -- "$test_root"
}

test_inhibitor_failure_stops_reconciliation() {
  local test_root

  test_root="$(mktemp -d)"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'exit 73' \
    > "$test_root/systemd-inhibit"
  chmod 0755 "$test_root/systemd-inhibit"

  if (
    PATH="$test_root:$PATH"
    export PATH
    systemd_inhibit_command="$test_root/systemd-inhibit"
    reconcile_script="$REPO_ROOT/scripts/reconcile-kmods.sh"
    enter_systemd_inhibitor max98390
  ) >/dev/null 2>&1; then
    printf 'expected inhibitor acquisition failure to stop reconciliation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi

  rm -rf -- "$test_root"
}

test_coordinated_reconciliation_requires_inhibitor_parent_and_inherited_global() {
  if (
    caller_is_root() { return 0; }
    reconcile_inhibitor_parent_fixture=0
    main --coordinated ov02c10
  ) >/dev/null 2>&1; then
    printf 'coordinated reconciliation accepted an unauthenticated parent\n' >&2
    return 1
  fi
  if (
    caller_is_root() { return 0; }
    reconcile_inhibitor_parent_fixture=1
    lifecycle_helper=/dev/null
    validate_kmod_lifecycle_lock() { return 1; }
    unset GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD
    main --coordinated ov02c10
  ) >/dev/null 2>&1; then
    printf 'coordinated reconciliation accepted a missing inherited global lock\n' >&2
    return 1
  fi
}

test_rpm_read_lock_rejects_unsafe_state() {
  local test_root hardlink

  test_root="$(mktemp -d)"
  prepare_reconcile_rpm_lock_fixture "$test_root"

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

test_rpm_read_lock_detects_post_acquisition_tampering() {
  local test_root original

  test_root="$(mktemp -d)"
  prepare_reconcile_rpm_lock_fixture "$test_root"
  acquire_rpm_read_lock
  validate_rpm_read_lock

  original="$rpm_lock_path.original"
  mv -- "$rpm_lock_path" "$original"
  (umask 022; : > "$rpm_lock_path")
  if validate_rpm_read_lock >/dev/null 2>&1; then
    printf 'accepted a replaced RPM lock pathname\n' >&2
    exec {reconcile_rpm_lock_fd}>&-
    rm -rf -- "$test_root"
    return 1
  fi
  exec {reconcile_rpm_lock_fd}>&-
  reconcile_rpm_lock_fd=""
  rm -f -- "$rpm_lock_path"
  mv -- "$original" "$rpm_lock_path"

  acquire_rpm_read_lock
  chmod 0777 -- "$test_root/usr/lib/sysimage"
  if validate_rpm_read_lock >/dev/null 2>&1; then
    printf 'accepted RPM ancestry changed after acquisition\n' >&2
    exec {reconcile_rpm_lock_fd}>&-
    rm -rf -- "$test_root"
    return 1
  fi
  exec {reconcile_rpm_lock_fd}>&-
  reconcile_rpm_lock_fd=""
  chmod 0755 -- "$test_root/usr/lib/sysimage"

  acquire_rpm_read_lock
  ln -- "$rpm_lock_path" "$test_root/rpm-hardlink"
  if validate_rpm_read_lock >/dev/null 2>&1; then
    printf 'accepted RPM hardlink added after acquisition\n' >&2
    exec {reconcile_rpm_lock_fd}>&-
    rm -rf -- "$test_root"
    return 1
  fi
  exec {reconcile_rpm_lock_fd}>&-
  reconcile_rpm_lock_fd=""

  rm -rf -- "$test_root"
}

test_rpm_read_lock_has_bounded_contention() {
  local test_root holder_ready holder_release holder_pid status start_ns elapsed_ms

  test_root="$(mktemp -d)"
  prepare_reconcile_rpm_lock_fixture "$test_root"
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
  if (
    installed_kernels() { printf '%s\n' test-kernel; }
    validate_packaged_module() { : > "$RECONCILE_UNEXPECTED_SIDE_EFFECT"; }
    export RECONCILE_UNEXPECTED_SIDE_EFFECT="$test_root/unexpected-side-effect"
    complete_reconciliation ov02c10 test-kernel test-kernel
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  elapsed_ms=$(( ($(date +%s%N) - start_ns) / 1000000 ))
  : > "$holder_release"
  wait "$holder_pid"
  [[ "$status" -ne 0 ]]
  [[ ! -e "$test_root/unexpected-side-effect" ]]
  (( elapsed_ms >= 800 && elapsed_ms <= 3000 ))

  rm -rf -- "$test_root"
}

test_reconciliation_uses_safe_rpm_snapshots_around_private_finalizers() {
  local test_root trace

  test_root="$(mktemp -d)"
  trace="$test_root/trace"
  prepare_reconcile_rpm_lock_fixture "$test_root"

  (
    installed_kernels() {
      assert_external_writer_is_blocked INVENTORY
      printf '%s\n' test-kernel
    }
    assert_external_writer_is_blocked() {
      local phase="$1"

      validate_rpm_read_lock
      validate_kmod_lifecycle_lock
      if flock --fcntl --exclusive --nonblock "$rpm_lock_path" true; then
        printf 'external RPM writer entered during %s\n' "$phase" >&2
        return 1
      fi
      printf '%s\n' "$phase" >> "$RECONCILE_LOCK_TRACE"
    }
    assert_private_finalizer_phase() {
      local phase="$1"

      [[ -z "$reconcile_rpm_lock_fd" ]]
      validate_kmod_lifecycle_lock
      # Between stable RPM snapshots an external writer may proceed. The
      # package finalizer then takes its private lock followed by RPM shared;
      # model the latter in a real child and prove it excludes a writer.
      flock --fcntl --exclusive --nonblock "$rpm_lock_path" true
      flock --fcntl --shared --timeout 1 "$rpm_lock_path" bash -c '
        if flock --fcntl --exclusive --nonblock "$1" true; then
          exit 91
        fi
      ' bash "$rpm_lock_path"
      flock --fcntl --exclusive --nonblock "$rpm_lock_path" true
      printf '%s\n' "$phase" >> "$RECONCILE_LOCK_TRACE"
    }
    validate_packaged_module() {
      assert_external_writer_is_blocked "PACKAGE:$3"
    }
    finalize_max_kernel() {
      assert_private_finalizer_phase "FINALIZE_MAX:$1"
    }
    finalize_ov_kernel() {
      assert_private_finalizer_phase "FINALIZE_OV:$1"
    }
    validate_resolved_module() {
      assert_external_writer_is_blocked "RESOLVED:$3"
    }
    mok_reboot_required() { return 1; }
    enable_max_services() {
      assert_external_writer_is_blocked ENABLE_MAX
    }
    validate_max_service() {
      assert_external_writer_is_blocked VALIDATE_MAX_LIVE
    }
    log() {
      assert_external_writer_is_blocked ACKNOWLEDGEMENT
    }
    export RECONCILE_LOCK_TRACE="$trace"
    complete_reconciliation all test-kernel test-kernel
    [[ -z "$reconcile_rpm_lock_fd" ]]
  )

  [[ "$(grep -c '^INVENTORY$' "$trace")" -eq 2 ]]
  [[ "$(grep -c '^PACKAGE:ov02c10$' "$trace")" -eq 2 ]]
  [[ "$(grep -c '^PACKAGE:snd-hda-scodec-max98390$' "$trace")" -eq 2 ]]
  [[ "$(grep -c '^PACKAGE:snd-hda-scodec-max98390-i2c$' "$trace")" -eq 2 ]]
  grep -Fxq 'FINALIZE_MAX:test-kernel' "$trace"
  grep -Fxq 'FINALIZE_OV:test-kernel' "$trace"
  grep -Fxq 'RESOLVED:ov02c10' "$trace"
  grep -Fxq 'RESOLVED:snd-hda-scodec-max98390' "$trace"
  grep -Fxq 'RESOLVED:snd-hda-scodec-max98390-i2c' "$trace"
  grep -Fxq 'ENABLE_MAX' "$trace"
  grep -Fxq 'VALIDATE_MAX_LIVE' "$trace"
  grep -Fxq 'ACKNOWLEDGEMENT' "$trace"
  local first_package finalizer last_package resolved ack
  first_package="$(grep -n '^PACKAGE:ov02c10$' "$trace" | head -1 | cut -d: -f1)"
  finalizer="$(grep -n '^FINALIZE_MAX:' "$trace" | cut -d: -f1)"
  last_package="$(grep -n '^PACKAGE:ov02c10$' "$trace" | tail -1 | cut -d: -f1)"
  resolved="$(grep -n '^RESOLVED:ov02c10$' "$trace" | cut -d: -f1)"
  ack="$(grep -n '^ACKNOWLEDGEMENT$' "$trace" | cut -d: -f1)"
  (( first_package < finalizer && finalizer < last_package \
    && last_package < resolved && resolved < ack ))
  flock --fcntl --exclusive --nonblock "$rpm_lock_path" true

  rm -rf -- "$test_root"
}

test_rpm_read_lock_releases_on_failure_and_signal() {
  local test_root ready block_fifo child_pid

  test_root="$(mktemp -d)"
  prepare_reconcile_rpm_lock_fixture "$test_root"

  if (
    installed_kernels() { printf '%s\n' test-kernel; }
    validate_packaged_module() {
      fail "falha sintética durante a validação"
    }
    complete_reconciliation ov02c10 test-kernel test-kernel
  ) >/dev/null 2>&1; then
    printf 'expected the synthetic finalization failure\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  flock --fcntl --exclusive --nonblock "$rpm_lock_path" true

  if (
    installed_kernels() { printf '%s\n' another-kernel; }
    validate_packaged_module() {
      : > "$RECONCILE_UNEXPECTED_SIDE_EFFECT"
    }
    export RECONCILE_UNEXPECTED_SIDE_EFFECT="$test_root/unexpected-side-effect"
    complete_reconciliation ov02c10 test-kernel test-kernel
  ) >/dev/null 2>&1; then
    printf 'accepted a changed kernel inventory under the RPM lock\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  [[ ! -e "$test_root/unexpected-side-effect" ]]
  flock --fcntl --exclusive --nonblock "$rpm_lock_path" true

  ready="$test_root/signal.ready"
  block_fifo="$test_root/signal.block"
  mkfifo -- "$block_fifo"
  (
    installed_kernels() { printf '%s\n' test-kernel; }
    validate_packaged_module() { :; }
    finalize_ov_kernel() { :; }
    validate_resolved_module() {
      : > "$RECONCILE_SIGNAL_READY"
      IFS= read -r _ < "$RECONCILE_SIGNAL_BLOCK"
    }
    mok_reboot_required() { return 1; }
    log() { :; }
    export RECONCILE_SIGNAL_READY="$ready"
    export RECONCILE_SIGNAL_BLOCK="$block_fifo"
    complete_reconciliation ov02c10 test-kernel test-kernel
  ) &
  child_pid=$!
  for ((attempt = 1; attempt <= 200; attempt++)); do
    [[ -e "$ready" ]] && break
    sleep 0.01
  done
  if [[ ! -e "$ready" ]]; then
    kill -TERM "$child_pid" 2>/dev/null || :
    wait "$child_pid" 2>/dev/null || :
    printf 'signal fixture did not reach the locked finalizer\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  if flock --fcntl --exclusive --nonblock "$rpm_lock_path" true; then
    kill -TERM "$child_pid" 2>/dev/null || :
    wait "$child_pid" 2>/dev/null || :
    printf 'external RPM writer entered before the signal\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  kill -TERM "$child_pid"
  if wait "$child_pid" 2>/dev/null; then
    printf 'expected the signaled finalizer to terminate unsuccessfully\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  flock --fcntl --exclusive --nonblock "$rpm_lock_path" true

  rm -rf -- "$test_root"
}

test_rejects_unowned_updates_override() {
  modinfo() {
    printf '%s\n' "/lib/modules/test-kernel/updates/sound/hda/codecs/side-codecs/snd-hda-scodec-max98390.ko"
  }
  rpm() {
    return 1
  }

  if (
    validate_resolved_module test-kernel kmod-galaxybook-max98390-test-kernel snd-hda-scodec-max98390 extra/galaxybook-max98390
  ) >/dev/null 2>&1; then
    printf 'expected an unowned updates override to be rejected\n' >&2
    return 1
  fi
}

test_accepts_owned_external_payload() {
  modinfo() {
    case "$*" in
      *'-F vermagic'*) printf '%s\n' 'test-kernel SMP' ;;
      *) printf '%s\n' "/lib/modules/test-kernel/extra/galaxybook-max98390/sound/hda/codecs/side-codecs/snd-hda-scodec-max98390.ko.xz" ;;
    esac
  }
  rpm() {
    if [[ "$1" == -qf ]]; then
      printf '%s\n' kmod-galaxybook-max98390-test-kernel
      return 0
    fi
    return 1
  }

  validate_resolved_module test-kernel kmod-galaxybook-max98390-test-kernel snd-hda-scodec-max98390 extra/galaxybook-max98390
}

test_rpm_queries_fail_closed_on_status_or_stderr() {
  if (
    rpm() {
      if [[ "$1" == -qf ]]; then
        printf '%s\n' kmod-galaxybook-max98390-test-kernel
        return 1
      fi
      return 1
    }
    rpm_file_owner /lib/modules/test-kernel/extra/galaxybook-max98390/module.ko
  ) >/dev/null 2>&1; then
    printf 'expected rpm -qf stdout with a failed status to be rejected\n' >&2
    return 1
  fi

  if (
    rpm() {
      printf '%s\n' /lib/modules/test-kernel/extra/galaxybook-max98390/module.ko
      printf '%s\n' 'rpmdb warning' >&2
      return 0
    }
    package_module_path package module
  ) >/dev/null 2>&1; then
    printf 'expected rpm -ql stderr to make the package listing inconclusive\n' >&2
    return 1
  fi

  if (
    rpm() {
      printf '%s\n' /lib/modules/test-kernel/extra/galaxybook-max98390/module.ko
      return 1
    }
    package_module_path package module
  ) >/dev/null 2>&1; then
    printf 'expected rpm -ql output with a failed status to be rejected\n' >&2
    return 1
  fi
}

test_kernel_inventory_requires_complete_canonical_queries() {
  local inventory

  inventory="$(
    rpm() {
      [[ "$*" == $'-q --qf %{VERSION}-%{RELEASE}.%{ARCH}\\n -- kernel-core' ]]
      printf '%s\n' \
        '7.1.8-200.fc44.x86_64' \
        '7.1.6-201.fc44.x86_64'
    }
    uname() {
      [[ "$1" == -r ]]
      printf '%s\n' '7.1.7-200.fc44.x86_64'
    }
    installed_kernels
  )"
  [[ "$inventory" == $'7.1.6-201.fc44.x86_64\n7.1.7-200.fc44.x86_64\n7.1.8-200.fc44.x86_64' ]]

  if (
    rpm() { return 0; }
    uname() { printf '%s\n' '7.1.7-200.fc44.x86_64'; }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected an empty kernel-core inventory to be rejected\n' >&2
    return 1
  fi

  if (
    rpm() { printf '%s' '7.1.8-200.fc44.x86_64'; }
    uname() { printf '%s\n' '7.1.7-200.fc44.x86_64'; }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected a kernel-core inventory without a final newline to be rejected\n' >&2
    return 1
  fi

  if (
    rpm() {
      printf '%s\n' '7.1.8-200.fc44.x86_64'
      printf '%s\n' 'rpmdb warning' >&2
    }
    uname() { printf '%s\n' '7.1.7-200.fc44.x86_64'; }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected kernel-core stderr to make the inventory inconclusive\n' >&2
    return 1
  fi

  if (
    rpm() {
      printf '%s\n' '7.1.8-200.fc44.x86_64'
      return 1
    }
    uname() { printf '%s\n' '7.1.7-200.fc44.x86_64'; }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected a failed kernel-core query to be rejected\n' >&2
    return 1
  fi

  if (
    rpm() { printf '%s\n' '7.1.8-200.fc44.x86_64' '../../kernel'; }
    uname() { printf '%s\n' '7.1.7-200.fc44.x86_64'; }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected an unsafe kernel release to invalidate the whole inventory\n' >&2
    return 1
  fi

  if (
    rpm() { printf '%s\n' '7.1.8-200.fc44.x86_64'; }
    uname() {
      printf '%s\n' '7.1.7-200.fc44.x86_64'
      printf '%s\n' 'uname warning' >&2
    }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected uname stderr to invalidate the whole inventory\n' >&2
    return 1
  fi

  if (
    rpm() { printf '%s\n' '7.1.8-200.fc44.x86_64'; }
    uname() { printf '%s' '7.1.7-200.fc44.x86_64'; }
    installed_kernels
  ) >/dev/null 2>&1; then
    printf 'expected uname output without a final newline to be rejected\n' >&2
    return 1
  fi
}

test_evidence_probes_fail_closed_on_status_or_stderr() {
  if (
    modinfo() {
      case "$*" in
        *'-F vermagic'*) printf '%s\n' 'test-kernel SMP' ;;
        *) printf '%s\n' /lib/modules/test-kernel/extra/galaxybook-max98390/module.ko ;;
      esac
      return 1
    }
    rpm() {
      printf '%s\n' kmod-galaxybook-max98390-test-kernel
    }
    validate_resolved_module \
      test-kernel kmod-galaxybook-max98390-test-kernel \
      module extra/galaxybook-max98390
  ) >/dev/null 2>&1; then
    printf 'expected trusted-looking modinfo stdout with failure status to be rejected\n' >&2
    return 1
  fi

  if (
    systemctl() {
      printf '%s\n' inactive
      return 1
    }
    wait_for_akmods_unit akmods.service
  ) >/dev/null 2>&1; then
    printf 'expected a failed systemctl show to stop reconciliation\n' >&2
    return 1
  fi

  if (
    systemctl() {
      return 0
    }
    wait_for_akmods_unit akmods.service
  ) >/dev/null 2>&1; then
    printf 'expected an empty ActiveState to stop reconciliation\n' >&2
    return 1
  fi

  if (
    systemctl() {
      printf '%s\n' inactive
      printf '%s\n' 'systemd warning' >&2
    }
    wait_for_akmods_unit akmods.service
  ) >/dev/null 2>&1; then
    printf 'expected systemctl show stderr to stop reconciliation\n' >&2
    return 1
  fi

  if (
    systemctl() {
      printf '%s\n' active inactive
    }
    wait_for_akmods_unit akmods.service
  ) >/dev/null 2>&1; then
    printf 'expected a non-canonical ActiveState to stop reconciliation\n' >&2
    return 1
  fi

  if (
    SECURE_BOOT_STATE=enabled
    openssl() {
      printf '%s\n' 'subject=CN=trusted-looking'
      return 1
    }
    detect_akmods_signing_identity
  ) >/dev/null 2>&1; then
    printf 'expected a failed openssl identity probe to stop reconciliation\n' >&2
    return 1
  fi

  if (
    fake_probe() {
      printf '%s\n' valid
      printf '%s\n' warning >&2
    }
    capture_clean_output fake_probe
  ) >/dev/null 2>&1; then
    printf 'expected stderr from an evidence probe to be rejected\n' >&2
    return 1
  fi

  if (
    fake_probe() {
      printf 'valid\n\n'
    }
    capture_clean_output fake_probe
  ) >/dev/null 2>&1; then
    printf 'expected an extra trailing line from an evidence probe to be rejected\n' >&2
    return 1
  fi

  if (
    fake_probe() {
      printf 'valid\000'
    }
    capture_clean_output fake_probe
  ) >/dev/null 2>&1; then
    printf 'expected binary output from an evidence probe to be rejected\n' >&2
    return 1
  fi
}

test_certificate_queries_reject_warnings_and_unexpected_grammar() {
  if (
    openssl() {
      printf '%s\n' 'sha1 Fingerprint=00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
      printf '%s\n' 'openssl warning' >&2
    }
    certificate_sha1_fingerprint /test/public.der
  ) >/dev/null 2>&1; then
    printf 'expected certificate fingerprint stderr to be rejected\n' >&2
    return 1
  fi

  if (
    openssl() {
      printf '%s\n' '00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
    }
    certificate_sha1_fingerprint /test/public.der
  ) >/dev/null 2>&1; then
    printf 'expected a fingerprint without OpenSSL grammar to be rejected\n' >&2
    return 1
  fi

  if (
    openssl() {
      printf '%s\n' 'serial=112233445566778899AABBCCDDEEFF00112233'
      printf '%s\n' 'openssl warning' >&2
    }
    certificate_serial /test/public.der
  ) >/dev/null 2>&1; then
    printf 'expected certificate serial stderr to be rejected\n' >&2
    return 1
  fi
}

test_pending_mok_requires_reboot_without_live_validation() (
  SECURE_BOOT_STATE=enabled
  akmods_key_enrolled() {
    return 1
  }
  mok_enrollment_pending() {
    return 0
  }

  mok_reboot_required
)

test_mok_enrollment_requires_a_successful_test_key_status() {
  AKMODS_CERT_FINGERPRINT="00112233445566778899aabbccddeeff00112233"
  if (
    mokutil() {
      [[ "$1" == --test-key ]] || return 1
      printf '%s\n' 'already enrolled'
      printf '%s\n' 'mokutil warning' >&2
    }
    akmods_key_enrolled
  ); then
    printf 'expected test-key stderr to reject a trusted-looking success\n' >&2
    return 1
  fi
  if (
    mokutil() {
      case "$1" in
        --test-key) printf '%s\n' 'already enrolled'; return 1 ;;
        --list-enrolled) return 1 ;;
      esac
      return 1
    }
    akmods_key_enrolled
  ); then
    printf 'expected a failed mokutil status to override untrusted text\n' >&2
    return 1
  fi
  if (
    mokutil() {
      case "$1" in
        --test-key) return 1 ;;
        --list-enrolled)
          printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
          printf '%s\n' 'warning' >&2
          ;;
      esac
    }
    akmods_key_enrolled
  ); then
    printf 'expected list-enrolled stderr to reject matching output\n' >&2
    return 1
  fi
  (
    mokutil() {
      [[ "$1" == --test-key ]]
    }
    akmods_key_enrolled
  )
  (
    mokutil() {
      case "$1" in
        --test-key) return 1 ;;
        --list-enrolled)
          printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
          ;;
      esac
    }
    akmods_key_enrolled
  )
  if (
    mokutil() {
      case "$1" in
        --test-key) return 1 ;;
        --list-enrolled)
          printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
          return 1
          ;;
      esac
    }
    akmods_key_enrolled
  ); then
    printf 'expected a failed list-enrolled status to reject matching output\n' >&2
    return 1
  fi
  if (
    mokutil() {
      [[ "$1" == --list-new ]] || return 1
      printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
      printf '%s\n' 'warning' >&2
    }
    mok_enrollment_pending
  ); then
    printf 'expected list-new stderr to reject matching output\n' >&2
    return 1
  fi
  if (
    mokutil() {
      [[ "$1" == --list-new ]] || return 1
      printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33'
      return 1
    }
    mok_enrollment_pending
  ); then
    printf 'expected a failed list-new status to reject matching output\n' >&2
    return 1
  fi
  AKMODS_CERT_FINGERPRINT=""
}

test_secure_boot_state_is_explicit_and_fails_closed() {
  [[ "$(parse_secure_boot_state 'SecureBoot enabled')" == enabled ]]
  [[ "$(parse_secure_boot_state 'SecureBoot disabled')" == disabled ]]
  if parse_secure_boot_state 'estado desconhecido' >/dev/null; then
    printf 'expected unknown Secure Boot output to be rejected\n' >&2
    return 1
  fi
  if parse_secure_boot_state $'SecureBoot disabled\nwarning' >/dev/null; then
    printf 'expected multiline Secure Boot output to be rejected\n' >&2
    return 1
  fi
  if (
    mokutil() {
      return 1
    }
    detect_secure_boot_state
  ) >/dev/null 2>&1; then
    printf 'expected unavailable mokutil state to stop reconciliation\n' >&2
    return 1
  fi
  if (
    mokutil() {
      printf '%s\n' 'SecureBoot disabled'
      printf '%s\n' 'warning' >&2
    }
    detect_secure_boot_state
  ) >/dev/null 2>&1; then
    printf 'expected Secure Boot stderr to stop reconciliation\n' >&2
    return 1
  fi
}

test_secure_boot_rejects_a_payload_signed_by_another_key() {
  SECURE_BOOT_STATE=enabled
  AKMODS_SIGNER_CN="current-akmods-key"
  AKMODS_SIGNER_SERIAL="112233445566778899aabbccddeeff00112233"
  modinfo() {
    case "$*" in
      *'-F signer'*) printf '%s\n' old-akmods-key ;;
      *'-F sig_key'*) printf '%s\n' '00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33' ;;
      *'-F vermagic'*) printf '%s\n' 'test-kernel SMP' ;;
      *) printf '%s\n' "/lib/modules/test-kernel/extra/galaxybook-max98390/snd-hda-scodec-max98390.ko" ;;
    esac
  }
  rpm() {
    if [[ "$1" == -qf ]]; then
      printf '%s\n' kmod-galaxybook-max98390-test-kernel
      return 0
    fi
    return 1
  }

  if (validate_resolved_module test-kernel kmod-galaxybook-max98390-test-kernel snd-hda-scodec-max98390 extra/galaxybook-max98390) >/dev/null 2>&1; then
    printf 'expected a module signed by another key to be rejected\n' >&2
    return 1
  fi
  SECURE_BOOT_STATE=disabled
  AKMODS_SIGNER_CN=""
  AKMODS_SIGNER_SERIAL=""
}

test_secure_boot_rejects_a_same_cn_signature_from_another_key() {
  SECURE_BOOT_STATE=enabled
  AKMODS_SIGNER_CN="current-akmods-key"
  AKMODS_SIGNER_SERIAL="112233445566778899aabbccddeeff00112233"
  modinfo() {
    case "$*" in
      *'-F signer'*) printf '%s\n' current-akmods-key ;;
      *'-F sig_key'*) printf '%s\n' 'FF:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33' ;;
      *'-F vermagic'*) printf '%s\n' 'test-kernel SMP' ;;
      *) printf '%s\n' "/lib/modules/test-kernel/extra/galaxybook-max98390/snd-hda-scodec-max98390.ko" ;;
    esac
  }
  rpm() {
    if [[ "$1" == -qf ]]; then
      printf '%s\n' kmod-galaxybook-max98390-test-kernel
      return 0
    fi
    return 1
  }

  if (validate_resolved_module test-kernel kmod-galaxybook-max98390-test-kernel snd-hda-scodec-max98390 extra/galaxybook-max98390) >/dev/null 2>&1; then
    printf 'expected a same-CN signature from another key to be rejected\n' >&2
    return 1
  fi
  SECURE_BOOT_STATE=disabled
  AKMODS_SIGNER_CN=""
  AKMODS_SIGNER_SERIAL=""
}

test_secure_boot_accepts_the_exact_current_signing_identity() {
  SECURE_BOOT_STATE=enabled
  AKMODS_SIGNER_CN="current-akmods-key"
  AKMODS_SIGNER_SERIAL="112233445566778899aabbccddeeff00112233"
  modinfo() {
    case "$*" in
      *'-F signer'*) printf '%s\n' current-akmods-key ;;
      *'-F sig_key'*) printf '%s\n' '00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33' ;;
      *'-F vermagic'*) printf '%s\n' 'test-kernel SMP' ;;
      *) printf '%s\n' "/lib/modules/test-kernel/extra/galaxybook-max98390/snd-hda-scodec-max98390.ko" ;;
    esac
  }
  rpm() {
    if [[ "$1" == -qf ]]; then
      printf '%s\n' kmod-galaxybook-max98390-test-kernel
      return 0
    fi
    return 1
  }

  validate_resolved_module test-kernel kmod-galaxybook-max98390-test-kernel snd-hda-scodec-max98390 extra/galaxybook-max98390
  SECURE_BOOT_STATE=disabled
  AKMODS_SIGNER_CN=""
  AKMODS_SIGNER_SERIAL=""
}

test_explicit_reconciliation_forces_a_complete_rebuild() {
  local arguments

  for state in enabled disabled; do
    arguments="$(
      SECURE_BOOT_STATE="$state"
      export SECURE_BOOT_STATE
      wait_for_akmods_unit() { :; }
      ensure_kernel_build_tree() { :; }
      log() { :; }
      akmods() { printf '%s\n' "$*"; }
      build_driver_for_kernel galaxybook-ov02c10 test-kernel
    )"
    [[ "$arguments" == \
      $'--force --akmod galaxybook-ov02c10 --kernels test-kernel\n--force --rebuild --akmod galaxybook-ov02c10 --kernels test-kernel' ]]
  done
}

test_mok_fingerprint_matching_is_exact() {
  printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33' \
    | mok_output_contains_fingerprint 00112233445566778899aabbccddeeff00112233
  if printf '%s\n' 'SHA1 Fingerprint: 00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:34' \
    | mok_output_contains_fingerprint 00112233445566778899aabbccddeeff00112233; then
    printf 'expected a different MOK fingerprint to be rejected\n' >&2
    return 1
  fi
}

test_certificate_identity_is_cryptographic_and_well_formed() {
  local test_root fingerprint serial

  test_root="$(mktemp -d)"
  openssl req -x509 -newkey rsa:2048 -nodes \
    -subj '/CN=galaxybook-reconcile-test' \
    -keyout "$test_root/private.pem" \
    -out "$test_root/public.pem" \
    -days 1 >/dev/null 2>&1
  openssl x509 -in "$test_root/public.pem" -outform DER \
    -out "$test_root/public.der"

  fingerprint="$(certificate_sha1_fingerprint "$test_root/public.der")"
  serial="$(certificate_serial "$test_root/public.der")"
  [[ "$fingerprint" =~ ^[0-9a-f]{40}$ ]]
  [[ "$serial" =~ ^[0-9a-f]+$ ]]
  (( ${#serial} >= 2 && ${#serial} <= 40 && ${#serial} % 2 == 0 ))

  rm -rf -- "$test_root"
}

test_certificate_serial_canonicalizes_der_integer_padding() {
  [[ "$(normalize_certificate_serial '80')" == 80 ]]
  [[ "$(normalize_certificate_serial '00:80')" == 80 ]]
  [[ "$(normalize_certificate_serial '00:01')" == 01 ]]
  if normalize_certificate_serial 0000 >/dev/null; then
    printf 'expected an all-zero certificate serial to be rejected\n' >&2
    return 1
  fi
}

test_kernel_names_are_strictly_validated() {
  kernel_name_is_safe '7.1.8-200.fc44.x86_64'
  kernel_name_is_safe '7'
  if kernel_name_is_safe '../kernel-inválido'; then
    printf 'expected unsafe kernel name to be rejected\n' >&2
    return 1
  fi
  if kernel_name_is_safe 'kernel-without-leading-digit'; then
    printf 'expected a kernel name without a leading digit to be rejected\n' >&2
    return 1
  fi
  if kernel_name_is_safe '7.1.8-'; then
    printf 'expected a kernel name without an alphanumeric suffix to be rejected\n' >&2
    return 1
  fi
}

test_max_hardware_discovery_is_tristate_and_side_effect_free() {
  local status test_root record helper

  if (
    compgen() { return 1; }
    grep() { return 1; }
    max_hardware_present
  ); then
    printf 'expected a conclusive non-match to report hardware absent\n' >&2
    return 1
  else
    status=$?
  fi
  [[ "$status" -eq 1 ]]

  if (
    compgen() { return 1; }
    grep() { return 2; }
    max_hardware_present
  ); then
    printf 'expected a grep error to make hardware discovery inconclusive\n' >&2
    return 1
  else
    status=$?
  fi
  [[ "$status" -eq 2 ]]

  test_root="$(mktemp -d)"
  record="$test_root/systemctl-calls"
  helper="$test_root/max-helper"
  printf '%s\n' '#!/usr/bin/env bash' 'exit 0' > "$helper"
  chmod 0755 "$helper"

  if (
    compgen() { return 1; }
    grep() { return 2; }
    systemctl() { printf '%s\n' "$*" >> "$MAX_SYSTEMCTL_RECORD"; }
    export MAX_SYSTEMCTL_RECORD="$record"
    validate_max_service "$helper"
  ) >/dev/null 2>&1; then
    printf 'expected inconclusive MAX discovery to stop validation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  [[ ! -e "$record" ]]

  (
    compgen() { return 1; }
    grep() { return 1; }
    systemctl() { printf '%s\n' "$*" >> "$MAX_SYSTEMCTL_RECORD"; }
    export MAX_SYSTEMCTL_RECORD="$record"
    validate_max_service "$helper"
  )
  [[ ! -e "$record" ]]

  rm -rf -- "$test_root"
}

test_max_service_state_queries_fail_closed() {
  local test_root helper

  test_root="$(mktemp -d)"
  helper="$test_root/max-helper"
  printf '%s\n' '#!/usr/bin/env bash' 'exit 0' > "$helper"
  chmod 0755 "$helper"

  if (
    max_hardware_present() { return 0; }
    systemctl() {
      case "$1:$*" in
        restart:*) return 0 ;;
        show:*ActiveState*)
          printf '%s\n' active
          printf '%s\n' 'systemd warning' >&2
          ;;
        *) return 0 ;;
      esac
    }
    validate_max_service "$helper"
  ) >/dev/null 2>&1; then
    printf 'expected ActiveState stderr to stop MAX validation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi

  if (
    max_hardware_present() { return 0; }
    systemctl() {
      case "$1:$*" in
        restart:*) return 0 ;;
        show:*ActiveState*) printf '%s\n' active inactive ;;
        *) return 0 ;;
      esac
    }
    validate_max_service "$helper"
  ) >/dev/null 2>&1; then
    printf 'expected a non-canonical MAX ActiveState to stop validation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi

  if (
    max_hardware_present() { return 0; }
    systemctl() {
      case "$1:$*" in
        restart:*) return 0 ;;
        show:*ActiveState*) printf '%s\n' active ;;
        show:*Result*)
          printf '%s\n' success
          printf '%s\n' 'systemd warning' >&2
          ;;
        *) return 1 ;;
      esac
    }
    validate_max_service "$helper"
  ) >/dev/null 2>&1; then
    printf 'expected Result stderr to stop MAX validation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi

  (
    max_hardware_present() { return 0; }
    systemctl() {
      case "$1:$*" in
        restart:*) return 0 ;;
        show:*ActiveState*) printf '%s\n' active ;;
        show:*Result*) printf '%s\n' success ;;
        *) return 1 ;;
      esac
    }
    find() { printf '....'; }
    validate_max_service "$helper"
  )

  rm -rf -- "$test_root"
}

test_max_finalizer_is_synchronous_and_kernel_scoped() {
  local test_root helper record

  test_root="$(mktemp -d)"
  helper="$test_root/max-finalize"
  record="$test_root/arguments"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf "%s\n" "$@" > "$MAX_FINALIZER_RECORD"' \
    > "$helper"
  chmod 0755 "$helper"

  MAX_FINALIZER_RECORD="$record" run_max_finalizer "$helper" test-kernel
  grep -Fxq -- '--kernel' "$record"
  grep -Fxq -- 'test-kernel' "$record"

  if (run_max_finalizer "$test_root/missing" test-kernel) >/dev/null 2>&1; then
    printf 'expected a missing MAX finalizer to stop reconciliation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi
  rm -rf -- "$test_root"
}

test_packaging_constraints_match_the_public_helper_contracts() {
  local spec="$REPO_ROOT/packaging/fedora/galaxybook-setup.spec"

  grep -Fxq 'Recommends:     akmod-galaxybook-max98390 >= 1.00.12' "$spec"
  grep -Fxq 'Recommends:     galaxybook-max98390-kmod-common >= 1.00.12' "$spec"
  grep -Fxq 'Recommends:     kmod-galaxybook-max98390 >= 1.00.12' "$spec"
  grep -Fxq 'Recommends:     akmod-galaxybook-ov02c10 >= 1.00.25' "$spec"
  grep -Fxq 'Recommends:     galaxybook-ov02c10-kmod-common >= 1.00.25' "$spec"
  grep -Fxq 'Recommends:     kmod-galaxybook-ov02c10 >= 1.00.25' "$spec"
  grep -Fxq 'Recommends:     galaxybook-camera >= 1.00.43' "$spec"
  grep -Fxq 'Suggests:       galaxybook-sound >= 1.00.18' "$spec"
  grep -Fxq 'Requires:       authselect' "$spec"
  grep -Fxq 'Requires:       usbutils' "$spec"
  grep -Fxq 'Requires:       /usr/bin/flock' "$spec"
  grep -Fxq 'Requires:       /usr/bin/sg' "$spec"
  grep -Fxq 'Requires:       (galaxybook-ov02c10-kmod-common >= 1.00.25 if galaxybook-ov02c10-kmod-common)' "$spec"
  grep -Fxq 'Requires:       (galaxybook-max98390-kmod-common >= 1.00.12 if galaxybook-max98390-kmod-common)' "$spec"
  grep -Fxq 'Recommends:     fprintd' "$spec"
  grep -Fxq 'Recommends:     gnome-control-center' "$spec"
  grep -Fxq 'Recommends:     gnome-shell' "$spec"
  grep -Fxq 'Recommends:     wireplumber' "$spec"
  grep -Fq 'install -Dm644 scripts/kmod-lifecycle-lock.sh %{buildroot}%{_libexecdir}/galaxybook-setup/kmod-lifecycle-lock.sh' "$spec"
  grep -Fq 'install -Dm755 scripts/driver-maintenance.sh %{buildroot}%{_libexecdir}/galaxybook-setup/driver-maintenance' "$spec"
  grep -Fq 'install -Dm755 scripts/repair-nvidia-kmod.sh %{buildroot}%{_libexecdir}/galaxybook-setup/repair-nvidia-kmod' "$spec"
  grep -Fxq '%{_libexecdir}/galaxybook-setup/kmod-lifecycle-lock.sh' "$spec"
  grep -Fxq '%{_libexecdir}/galaxybook-setup/driver-maintenance' "$spec"
  grep -Fxq '%{_libexecdir}/galaxybook-setup/repair-nvidia-kmod' "$spec"
  grep -Fxq 'bash tests/kmod-lifecycle-lock.sh' "$spec"
  grep -Fxq 'bash tests/driver-maintenance.sh' "$spec"
  grep -Fxq 'bash tests/repair-nvidia-kmod.sh' "$spec"
}

test_documented_legacy_upgrade_is_two_stage_and_translated() {
  local readme snapshot_contract

  for readme in \
    "$REPO_ROOT/README.md" \
    "$REPO_ROOT/README.en.md" \
    "$REPO_ROOT/README.es.md" \
    "$REPO_ROOT/README.it.md"; do
    grep -Fq 'galaxybook-setup <= 1.00.47' "$readme"
    grep -Fq 'sudo dnf --setopt=install_weak_deps=False --refresh upgrade galaxybook-setup' \
      "$readme"
    grep -Fq '1.00.48' "$readme"
    case "${readme##*/}" in
      README.md)
        snapshot_contract='O coordenador libera esse lock antes de chamar cada família'
        ;;
      README.en.md)
        snapshot_contract='The coordinator releases that lock before invoking each family'
        ;;
      README.es.md)
        snapshot_contract=''
        ;;
      README.it.md)
        snapshot_contract='Il coordinatore rilascia quel lock prima di'
        ;;
      *) return 1 ;;
    esac
    if [[ "${readme##*/}" == README.es.md ]]; then
      grep -Fq 'El coordinador libera ese' "$readme"
      grep -Fq 'lock antes de invocar cada familia' "$readme"
    else
      grep -Fq "$snapshot_contract" "$readme"
    fi
    grep -Fq '`vermagic`' "$readme"
  done
}

test_reconciliation_acquires_global_before_driver_work
test_production_rpm_lock_contract_ignores_environment
test_inhibitor_wraps_the_entire_reconciliation
test_inhibitor_failure_stops_reconciliation
test_coordinated_reconciliation_requires_inhibitor_parent_and_inherited_global
test_rpm_read_lock_rejects_unsafe_state
test_rpm_read_lock_detects_post_acquisition_tampering
test_rpm_read_lock_has_bounded_contention
test_reconciliation_uses_safe_rpm_snapshots_around_private_finalizers
test_rpm_read_lock_releases_on_failure_and_signal
test_rejects_unowned_updates_override
test_accepts_owned_external_payload
test_rpm_queries_fail_closed_on_status_or_stderr
test_kernel_inventory_requires_complete_canonical_queries
test_evidence_probes_fail_closed_on_status_or_stderr
test_certificate_queries_reject_warnings_and_unexpected_grammar
test_pending_mok_requires_reboot_without_live_validation
test_mok_enrollment_requires_a_successful_test_key_status
test_secure_boot_state_is_explicit_and_fails_closed
test_secure_boot_rejects_a_payload_signed_by_another_key
test_secure_boot_rejects_a_same_cn_signature_from_another_key
test_secure_boot_accepts_the_exact_current_signing_identity
test_explicit_reconciliation_forces_a_complete_rebuild
test_mok_fingerprint_matching_is_exact
test_certificate_identity_is_cryptographic_and_well_formed
test_certificate_serial_canonicalizes_der_integer_padding
test_kernel_names_are_strictly_validated
test_max_hardware_discovery_is_tristate_and_side_effect_free
test_max_service_state_queries_fail_closed
test_max_finalizer_is_synchronous_and_kernel_scoped
test_packaging_constraints_match_the_public_helper_contracts
test_documented_legacy_upgrade_is_two_stage_and_translated
