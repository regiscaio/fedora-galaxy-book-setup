#!/usr/bin/env bash

set -euo pipefail

readonly TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "$TEST_DIR/.." && pwd)"

# shellcheck source=../scripts/reconcile-kmods.sh
source "$REPO_ROOT/scripts/reconcile-kmods.sh"

SECURE_BOOT_STATE=disabled

test_inhibitor_wraps_the_entire_reconciliation() {
  local test_root record

  test_root="$(mktemp -d)"
  record="$test_root/arguments"
  printf '%s\n' \
    '#!/usr/bin/env bash' \
    'printf "marker=%s\n" "${GALAXYBOOK_SETUP_RECONCILE_INHIBITED:-}" > "$INHIBITOR_RECORD"' \
    'printf "%s\n" "$@" >> "$INHIBITOR_RECORD"' \
    > "$test_root/systemd-inhibit"
  chmod 0755 "$test_root/systemd-inhibit"

  (
    PATH="$test_root:$PATH"
    export PATH INHIBITOR_RECORD="$record"
    unset GALAXYBOOK_SETUP_RECONCILE_INHIBITED
    enter_systemd_inhibitor ov02c10
  )

  grep -Fxq 'marker=1' "$record"
  grep -Fxq -- '--mode=block' "$record"
  grep -Fxq -- '--what=shutdown:sleep:idle' "$record"
  grep -Fxq -- '--who=Galaxy Book Setup' "$record"
  grep -Fxq -- '--why=Gerando e validando módulos externos para todos os kernels' "$record"
  grep -Fxq -- "$REPO_ROOT/scripts/reconcile-kmods.sh" "$record"
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
    unset GALAXYBOOK_SETUP_RECONCILE_INHIBITED
    enter_systemd_inhibitor max98390
  ) >/dev/null 2>&1; then
    printf 'expected inhibitor acquisition failure to stop reconciliation\n' >&2
    rm -rf -- "$test_root"
    return 1
  fi

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

  grep -Fxq 'Recommends:     akmod-galaxybook-max98390 >= 1.00.10' "$spec"
  grep -Fxq 'Recommends:     galaxybook-max98390-kmod-common >= 1.00.10' "$spec"
  grep -Fxq 'Recommends:     kmod-galaxybook-max98390 >= 1.00.10' "$spec"
  grep -Fxq 'Recommends:     akmod-galaxybook-ov02c10 >= 1.00.23' "$spec"
  grep -Fxq 'Recommends:     galaxybook-ov02c10-kmod-common >= 1.00.23' "$spec"
  grep -Fxq 'Recommends:     kmod-galaxybook-ov02c10 >= 1.00.23' "$spec"
  grep -Fxq 'Recommends:     galaxybook-camera >= 1.00.43' "$spec"
  grep -Fxq 'Suggests:       galaxybook-sound >= 1.00.18' "$spec"
  grep -Fxq 'Requires:       authselect' "$spec"
  grep -Fxq 'Requires:       usbutils' "$spec"
  grep -Fxq 'Requires:       /usr/bin/flock' "$spec"
  grep -Fxq 'Requires:       /usr/bin/sg' "$spec"
  grep -Fxq 'Recommends:     fprintd' "$spec"
  grep -Fxq 'Recommends:     gnome-control-center' "$spec"
  grep -Fxq 'Recommends:     gnome-shell' "$spec"
  grep -Fxq 'Recommends:     wireplumber' "$spec"
}

test_inhibitor_wraps_the_entire_reconciliation
test_inhibitor_failure_stops_reconciliation
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
