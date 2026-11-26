#!/usr/bin/bash

set -euo pipefail
export LC_ALL=C
export PATH=/usr/sbin:/usr/bin

readonly OV_AKMOD="galaxybook-ov02c10"
readonly OV_PACKAGE_PREFIX="kmod-galaxybook-ov02c10"
readonly OV_MODULE="ov02c10"
readonly MAX_AKMOD="galaxybook-max98390"
readonly MAX_PACKAGE_PREFIX="kmod-galaxybook-max98390"
readonly MAX_MODULE_CORE="snd-hda-scodec-max98390"
readonly MAX_MODULE_I2C="snd-hda-scodec-max98390-i2c"
readonly AKMODS_PUBLIC_KEY="/etc/pki/akmods/certs/public_key.der"
readonly AKMODS_PRIVATE_KEY="/etc/pki/akmods/private/private_key.priv"
lifecycle_helper="${BASH_SOURCE[0]%/*}/kmod-lifecycle-lock.sh"
reconcile_script="/usr/libexec/galaxybook-setup/reconcile-kmods"
module_tree_root="/lib/modules"
systemd_inhibit_command="/usr/bin/systemd-inhibit"
systemd_inhibit_stat_command="/usr/bin/stat"
reconcile_inhibitor_parent_fixture=""
rpm_lock_path="/usr/lib/sysimage/rpm/.rpm.lock"
rpm_lock_timeout_seconds="90"
rpm_lock_expected_uid="0"
rpm_lock_expected_gid="0"
rpm_lock_ancestry=(/usr /usr/lib /usr/lib/sysimage /usr/lib/sysimage/rpm)
reconcile_rpm_lock_fd=""
SECURE_BOOT_STATE=""
AKMODS_SIGNER_CN=""
AKMODS_SIGNER_SERIAL=""
AKMODS_CERT_FINGERPRINT=""
PROBE_STDOUT=""

log() {
  printf 'galaxybook-setup: %s\n' "$*"
}

fail() {
  printf 'galaxybook-setup: %s\n' "$*" >&2
  exit 1
}

usage() {
  printf 'Uso: %s {all|ov02c10|max98390|prepare-all}\n' "$0" >&2
  exit 2
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "O comando obrigatório '$1' não está disponível."
}

caller_is_root() {
  (( EUID == 0 ))
}

capture_clean_output() {
  capture_clean_output_preserving_newlines "$@" || return 1
  if [[ "$PROBE_STDOUT" == *$'\n' ]]; then
    PROBE_STDOUT="${PROBE_STDOUT%$'\n'}"
    if [[ "$PROBE_STDOUT" == *$'\n' ]]; then
      PROBE_STDOUT=""
      return 1
    fi
  fi
}

capture_clean_output_preserving_newlines() {
  local error_file output_file output raw_size status
  local marker=$'\036'

  output_file="$(mktemp /var/tmp/galaxybook-setup-probe-output.XXXXXX)" \
    || return 1
  error_file="$(mktemp /var/tmp/galaxybook-setup-probe-error.XXXXXX)" || {
    rm -f -- "$output_file"
    return 1
  }
  if "$@" >"$output_file" 2>"$error_file"; then
    status=0
  else
    status=$?
  fi
  if (( status != 0 )) || [[ -s "$error_file" ]]; then
    rm -f -- "$output_file" "$error_file"
    PROBE_STDOUT=""
    return 1
  fi
  if ! cmp -s -- "$output_file" <(tr -d '\000' < "$output_file"); then
    rm -f -- "$output_file" "$error_file"
    PROBE_STDOUT=""
    return 1
  fi
  if ! output="$(
    cat -- "$output_file" || exit $?
    printf '%s' "$marker"
  )"; then
    rm -f -- "$output_file" "$error_file"
    PROBE_STDOUT=""
    return 1
  fi
  raw_size="$(wc -c < "$output_file")" || {
    rm -f -- "$output_file" "$error_file"
    PROBE_STDOUT=""
    return 1
  }
  rm -f -- "$output_file" "$error_file"

  [[ "$output" == *"$marker" && "$raw_size" =~ ^[0-9]+$ ]] || {
    PROBE_STDOUT=""
    return 1
  }
  PROBE_STDOUT="${output%"$marker"}"
  # Bash variables cannot represent NUL bytes. Comparing the byte counts makes
  # a binary or otherwise truncated query fail closed instead of normalizing it.
  (( ${#PROBE_STDOUT} == raw_size )) || {
    PROBE_STDOUT=""
    return 1
  }
}

inhibitor_parent_is_trusted() {
  local parent="${PPID:-}"
  local executable_metadata executable_mode
  local parent_identity executable_identity

  if [[ "$reconcile_inhibitor_parent_fixture" == 1 ]]; then
    return 0
  fi
  [[ -z "$reconcile_inhibitor_parent_fixture" \
    && "$parent" =~ ^[1-9][0-9]*$ \
    && -x "$systemd_inhibit_command" \
    && -f "$systemd_inhibit_command" \
    && ! -L "$systemd_inhibit_command" \
    && -e "/proc/$parent/exe" ]] || return 1
  executable_metadata="$($systemd_inhibit_stat_command -Lc '%u:%g:%a:%h' -- \
    "$systemd_inhibit_command")" || return 1
  [[ "$executable_metadata" =~ ^0:0:([0-7]{3,4}):1$ ]] || return 1
  executable_mode="${BASH_REMATCH[1]}"
  (( (8#${executable_mode} & 8#022) == 0 \
    && (8#${executable_mode} & 8#111) != 0 )) || return 1
  parent_identity="$($systemd_inhibit_stat_command -Lc '%d:%i' -- \
    "/proc/$parent/exe")" || return 1
  executable_identity="$($systemd_inhibit_stat_command -Lc '%d:%i' -- \
    "$systemd_inhibit_command")" || return 1
  [[ "$parent_identity" == "$executable_identity" ]]
}

enter_systemd_inhibitor() {
  local mode="$1"

  [[ -x "$systemd_inhibit_command" ]] \
    || fail "O systemd-inhibit obrigatório está ausente: $systemd_inhibit_command."
  [[ -x "$reconcile_script" ]] \
    || fail "O helper de reconciliação não é executável: $reconcile_script."

  log "Bloqueando shutdown, suspensão e idle até todos os payloads terminarem"
  exec "$systemd_inhibit_command" \
    --mode=block \
    --what=shutdown:sleep:idle \
    --who="Galaxy Book Setup" \
    --why="Gerando e validando módulos externos para todos os kernels" \
    "$reconcile_script" --inhibited "$mode" \
    || fail "Não foi possível adquirir o inhibitor do systemd para proteger a reconciliação."
}

installed_kernels() {
  local kernel rpm_kernels running_kernel remaining

  capture_clean_output_preserving_newlines \
    rpm -q --qf '%{VERSION}-%{RELEASE}.%{ARCH}\n' -- kernel-core \
    || return 1
  rpm_kernels="$PROBE_STDOUT"
  [[ -n "$rpm_kernels" && "$rpm_kernels" == *$'\n' ]] || return 1

  remaining="$rpm_kernels"
  while [[ -n "$remaining" ]]; do
    kernel="${remaining%%$'\n'*}"
    kernel_name_is_safe "$kernel" || return 1
    remaining="${remaining#*$'\n'}"
  done

  capture_clean_output_preserving_newlines uname -r || return 1
  [[ "$PROBE_STDOUT" == *$'\n' ]] || return 1
  running_kernel="${PROBE_STDOUT%$'\n'}"
  kernel_name_is_safe "$running_kernel" || return 1
  {
    printf '%s\n' "$running_kernel"
    printf '%s' "$rpm_kernels"
  } | awk '!seen[$0]++' | sort -V
}

kernel_name_is_safe() {
  local kernel="$1"

  [[ -n "$kernel" && ${#kernel} -le 128 ]] \
    && [[ "${kernel:0:1}" =~ ^[0-9]$ ]] \
    && [[ "${kernel: -1}" =~ ^[A-Za-z0-9]$ ]] \
    && [[ "$kernel" != *..* ]] \
    && [[ "$kernel" =~ ^[A-Za-z0-9._+-]+$ ]]
}

rpm_lock_configuration_is_safe() {
  local parent

  [[ "$rpm_lock_path" == /* && "$rpm_lock_path" != *$'\n'* \
    && "$rpm_lock_timeout_seconds" =~ ^[1-9][0-9]*$ \
    && "$rpm_lock_expected_uid" =~ ^[0-9]+$ \
    && "$rpm_lock_expected_gid" =~ ^[0-9]+$ \
    && ${#rpm_lock_ancestry[@]} -gt 0 ]] || return 1
  parent="$(dirname -- "$rpm_lock_path")" || return 1
  [[ "$parent" == "${rpm_lock_ancestry[${#rpm_lock_ancestry[@]} - 1]}" ]]
}

rpm_lock_ancestry_is_safe() {
  local directory metadata mode

  rpm_lock_configuration_is_safe || return 1
  for directory in "${rpm_lock_ancestry[@]}"; do
    [[ "$directory" == /* && "$directory" != *$'\n'* \
      && -d "$directory" && ! -L "$directory" ]] || return 1
    metadata="$(stat -Lc '%u:%g:%a' -- "$directory")" || return 1
    [[ "$metadata" =~ ^([0-9]+):([0-9]+):([0-7]{3,4})$ ]] || return 1
    [[ "${BASH_REMATCH[1]}" == "$rpm_lock_expected_uid" \
      && "${BASH_REMATCH[2]}" == "$rpm_lock_expected_gid" ]] || return 1
    mode="${BASH_REMATCH[3]}"
    (( (8#${mode} & 8#022) == 0 )) || return 1
  done
}

rpm_lock_file_is_safe() {
  local path="$1"
  local metadata mode

  [[ -f "$path" && ! -L "$path" ]] || return 1
  metadata="$(stat -Lc '%u:%g:%a:%h' -- "$path")" || return 1
  [[ "$metadata" =~ ^([0-9]+):([0-9]+):([0-7]{3,4}):1$ ]] || return 1
  [[ "${BASH_REMATCH[1]}" == "$rpm_lock_expected_uid" \
    && "${BASH_REMATCH[2]}" == "$rpm_lock_expected_gid" ]] || return 1
  mode="${BASH_REMATCH[3]}"
  (( (8#${mode} & 8#022) == 0 ))
}

rpm_lock_fd_matches_path() {
  local lock_fd="$1"
  local fd_path fd_identity path_identity fd_metadata path_metadata

  [[ "$lock_fd" =~ ^[0-9]+$ && "$lock_fd" -ge 3 ]] || return 1
  fd_path="/proc/${BASHPID}/fd/$lock_fd"
  [[ -r "$fd_path" && -f "$fd_path" \
    && -f "$rpm_lock_path" && ! -L "$rpm_lock_path" ]] || return 1
  fd_identity="$(stat -Lc '%d:%i' -- "$fd_path")" || return 1
  path_identity="$(stat -Lc '%d:%i' -- "$rpm_lock_path")" || return 1
  [[ "$fd_identity" == "$path_identity" ]] || return 1
  fd_metadata="$(stat -Lc '%u:%g:%a:%h' -- "$fd_path")" || return 1
  path_metadata="$(stat -Lc '%u:%g:%a:%h' -- "$rpm_lock_path")" || return 1
  [[ "$fd_metadata" == "$path_metadata" ]] || return 1
  rpm_lock_file_is_safe "$rpm_lock_path"
}

validate_rpm_read_lock() {
  rpm_lock_ancestry_is_safe || return 1
  rpm_lock_fd_matches_path "$reconcile_rpm_lock_fd" || return 1
  flock --fcntl --shared --nonblock "$reconcile_rpm_lock_fd" || return 1
  rpm_lock_ancestry_is_safe || return 1
  rpm_lock_fd_matches_path "$reconcile_rpm_lock_fd"
}

acquire_rpm_read_lock() {
  rpm_lock_ancestry_is_safe \
    || fail "A hierarquia do lock RPM está ausente ou é insegura."
  rpm_lock_file_is_safe "$rpm_lock_path" \
    || fail "O arquivo de lock RPM está ausente ou é inseguro."
  exec {reconcile_rpm_lock_fd}<"$rpm_lock_path" \
    || fail "Não foi possível abrir o lock RPM."
  if ! rpm_lock_ancestry_is_safe \
    || ! rpm_lock_fd_matches_path "$reconcile_rpm_lock_fd"; then
    exec {reconcile_rpm_lock_fd}>&-
    reconcile_rpm_lock_fd=""
    fail "O lock RPM mudou enquanto era aberto."
  fi
  if ! flock --fcntl --shared --timeout "$rpm_lock_timeout_seconds" \
    "$reconcile_rpm_lock_fd"; then
    exec {reconcile_rpm_lock_fd}>&-
    reconcile_rpm_lock_fd=""
    fail "A reconciliação expirou aguardando a transação RPM ativa."
  fi
  validate_rpm_read_lock || {
    exec {reconcile_rpm_lock_fd}>&-
    reconcile_rpm_lock_fd=""
    fail "O lock RPM mudou depois da aquisição."
  }
}

release_rpm_read_lock() {
  [[ "$reconcile_rpm_lock_fd" =~ ^[0-9]+$ \
    && "$reconcile_rpm_lock_fd" -ge 3 ]] || return 1
  exec {reconcile_rpm_lock_fd}>&- || return 1
  reconcile_rpm_lock_fd=""
}

systemd_active_state_is_valid() {
  case "$1" in
    active|reloading|inactive|failed|activating|deactivating|maintenance|refreshing)
      return 0
      ;;
    *) return 1 ;;
  esac
}

wait_for_akmods_unit() {
  local unit="$1"
  local deadline=$((SECONDS + 900))
  local state

  while true; do
    capture_clean_output systemctl show "$unit" --property=ActiveState --value \
      || fail "Não foi possível consultar o estado do serviço $unit."
    state="$PROBE_STDOUT"
    systemd_active_state_is_valid "$state" \
      || fail "O serviço $unit retornou ActiveState inesperado: '${state:-vazio}'."
    case "$state" in
      activating|reloading|deactivating|refreshing)
        if (( SECONDS >= deadline )); then
          fail "O serviço $unit não terminou em 15 minutos. O reparo foi interrompido sem liberar o reboot."
        fi
        sleep 2
        ;;
      active|inactive|failed|maintenance) return 0 ;;
    esac
  done
}

ensure_kernel_build_tree() {
  local kernel="$1"

  if [[ -d "/usr/src/kernels/$kernel" ]]; then
    return 0
  fi

  log "Instalando os arquivos de desenvolvimento necessários para $kernel"
  dnf install -y "kernel-devel-uname-r = $kernel"
  [[ -d "/usr/src/kernels/$kernel" ]] \
    || fail "Os arquivos de desenvolvimento do kernel $kernel continuam ausentes."
}

parse_secure_boot_state() {
  case "$1" in
    'SecureBoot enabled') printf '%s\n' enabled ;;
    'SecureBoot disabled') printf '%s\n' disabled ;;
    *) return 1 ;;
  esac
}

detect_secure_boot_state() {
  local output

  if ! capture_clean_output mokutil --sb-state; then
    fail "Não foi possível consultar o estado do Secure Boot com mokutil."
  fi
  output="$PROBE_STDOUT"
  SECURE_BOOT_STATE="$(parse_secure_boot_state "$output" || true)"
  [[ -n "$SECURE_BOOT_STATE" ]] \
    || fail "A resposta do mokutil para o Secure Boot não foi reconhecida."
}

secure_boot_is_enabled() {
  [[ "$SECURE_BOOT_STATE" == enabled ]]
}

normalize_hex_identifier() {
  printf '%s' "$1" \
    | tr -d '[:space:]:' \
    | tr '[:upper:]' '[:lower:]'
}

normalize_certificate_serial() {
  local serial

  serial="$(normalize_hex_identifier "$1")"
  [[ "$serial" =~ ^[0-9a-f]+$ && $(( ${#serial} % 2 )) -eq 0 ]] \
    || return 1
  while (( ${#serial} > 2 )) && [[ "$serial" == 00* ]]; do
    serial="${serial:2}"
  done
  [[ ${#serial} -ge 2 && ${#serial} -le 40 && "$serial" =~ [1-9a-f] ]] \
    || return 1
  printf '%s\n' "$serial"
}

certificate_sha1_fingerprint() {
  local certificate="${1:-$AKMODS_PUBLIC_KEY}"
  local output fingerprint

  capture_clean_output \
    openssl x509 -inform DER -in "$certificate" -noout -fingerprint -sha1 \
    || return 1
  output="$PROBE_STDOUT"
  [[ "$output" == 'sha1 Fingerprint='* ]] || return 1
  fingerprint="$(normalize_hex_identifier "${output#*=}")"
  [[ "$fingerprint" =~ ^[0-9a-f]{40}$ ]] || return 1
  printf '%s\n' "$fingerprint"
}

certificate_serial() {
  local certificate="${1:-$AKMODS_PUBLIC_KEY}"
  local output serial

  capture_clean_output openssl x509 -inform DER -in "$certificate" -noout -serial \
    || return 1
  output="$PROBE_STDOUT"
  [[ "$output" == serial=* ]] || return 1
  serial="$(normalize_certificate_serial "${output#serial=}")" || return 1
  printf '%s\n' "$serial"
}

mok_output_contains_fingerprint() {
  local expected_fingerprint="$1"

  awk -v expected="$expected_fingerprint" '
    tolower($0) ~ /sha1 fingerprint:/ {
      value = $0
      sub(/^.*[Ff]ingerprint:[[:space:]]*/, "", value)
      gsub(/:|[[:space:]]/, "", value)
      if (tolower(value) == expected) {
        found = 1
      }
    }
    END { exit found ? 0 : 1 }
  '
}

ensure_signing_key_available() {
  secure_boot_is_enabled || return 0

  [[ -r "$AKMODS_PUBLIC_KEY" && -r "$AKMODS_PRIVATE_KEY" ]] \
    || fail "Secure Boot está ativo, mas a chave de assinatura do akmods não está pronta. Use primeiro a ação 'Preparar chave do Secure Boot'."
}

detect_akmods_signing_identity() {
  local subject

  secure_boot_is_enabled || return 0
  capture_clean_output openssl x509 -inform DER -in "$AKMODS_PUBLIC_KEY" \
    -noout -subject -nameopt RFC2253 \
    || fail "Não foi possível ler a identidade do certificado atual do akmods."
  subject="$PROBE_STDOUT"
  AKMODS_SIGNER_CN="${subject#*CN=}"
  AKMODS_SIGNER_CN="${AKMODS_SIGNER_CN%%,*}"
  [[ -n "$AKMODS_SIGNER_CN" && "$AKMODS_SIGNER_CN" != "$subject" ]] \
    && [[ "$AKMODS_SIGNER_CN" != *$'\n'* && "$AKMODS_SIGNER_CN" != *$'\r'* ]] \
    || fail "Não foi possível identificar o CN da chave atual do akmods."
  AKMODS_SIGNER_SERIAL="$(certificate_serial)" \
    || fail "Não foi possível identificar o serial do certificado atual do akmods."
  AKMODS_CERT_FINGERPRINT="$(certificate_sha1_fingerprint)" \
    || fail "Não foi possível calcular a identidade do certificado atual do akmods."
}

akmods_key_enrolled() {
  local enrolled_output

  if capture_clean_output mokutil --test-key "$AKMODS_PUBLIC_KEY"; then
    return 0
  fi
  [[ "$AKMODS_CERT_FINGERPRINT" =~ ^[0-9a-f]{40}$ ]] || return 1
  capture_clean_output mokutil --list-enrolled || return 1
  enrolled_output="$PROBE_STDOUT"
  mok_output_contains_fingerprint "$AKMODS_CERT_FINGERPRINT" <<< "$enrolled_output"
}

mok_enrollment_pending() {
  local pending_output

  [[ "$AKMODS_CERT_FINGERPRINT" =~ ^[0-9a-f]{40}$ ]] || return 1
  capture_clean_output mokutil --list-new || return 1
  pending_output="$PROBE_STDOUT"
  mok_output_contains_fingerprint "$AKMODS_CERT_FINGERPRINT" <<< "$pending_output"
}

mok_reboot_required() {
  secure_boot_is_enabled || return 1
  akmods_key_enrolled && return 1
  if mok_enrollment_pending; then
    return 0
  fi
  fail "Os payloads estão prontos, mas a chave do akmods não está inscrita nem possui pedido pendente no MOK. Use 'Preparar chave do Secure Boot' antes de reiniciar."
}

publish_enrolled_identity_if_needed() {
  local identity_helper="${BASH_SOURCE[0]%/*}/prepare-secure-boot-key"
  local current_fingerprint

  secure_boot_is_enabled || return 0
  require_reconciliation_locks
  [[ -x "$identity_helper" && "$AKMODS_CERT_FINGERPRINT" =~ ^[0-9a-f]{40}$ ]] \
    || fail "A identidade pública do akmods não está pronta para publicação."
  # The child reuses GLOBAL and takes only the directory lock for publishing.
  # Never generate a key, request enrollment or prompt for a password here.
  "$identity_helper" --publish-enrolled \
    || fail "Falha ao publicar a identidade já inscrita do akmods para o diagnóstico sem privilégios."
  current_fingerprint="$(certificate_sha1_fingerprint)" \
    || fail "Não foi possível revalidar o certificado do akmods após a publicação."
  [[ "$current_fingerprint" == "$AKMODS_CERT_FINGERPRINT" ]] \
    || fail "O certificado do akmods mudou durante a reconciliação."
  require_reconciliation_locks
}

build_driver_for_kernel() {
  local akmod="$1"
  local kernel="$2"

  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido."
  wait_for_akmods_unit akmods.service
  wait_for_akmods_unit "akmods@$kernel.service"
  ensure_kernel_build_tree "$kernel"

  log "Instalando ou atualizando $akmod para $kernel"
  akmods --force --akmod "$akmod" --kernels "$kernel"
  log "Reconstruindo $akmod para $kernel com a chave atual"
  akmods --force --rebuild --akmod "$akmod" --kernels "$kernel"
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido após o akmods."
}

package_module_path() {
  local package="$1"
  local module="$2"
  local query_output path matched_path=""

  capture_clean_output rpm -ql -- "$package" || return 1
  query_output="$PROBE_STDOUT"

  while IFS= read -r path; do
    case "$path" in
      */"$module".ko|*/"$module".ko.gz|*/"$module".ko.xz|*/"$module".ko.zst)
        [[ -z "$matched_path" ]] || return 1
        matched_path="$path"
        ;;
    esac
  done <<< "$query_output"
  [[ -n "$matched_path" ]] || return 1
  printf '%s\n' "$matched_path"
}

rpm_file_owner() {
  local path="$1"

  capture_clean_output rpm -qf --qf '%{NAME}\n' -- "$path" || return 1
  [[ -n "$PROBE_STDOUT" && "$PROBE_STDOUT" != *$'\n'* ]] || return 1
  printf '%s\n' "$PROBE_STDOUT"
}

validate_packaged_module() {
  local kernel="$1"
  local package="$2"
  local module="$3"
  local expected_directory="$4"
  local path owner vermagic signer signer_serial

  capture_clean_output rpm -q --qf '%{NAME}\n' -- "$package" \
    || fail "O akmods terminou sem instalar o pacote obrigatório $package."
  [[ "$PROBE_STDOUT" == "$package" ]] \
    || fail "A consulta RPM não confirmou exatamente o pacote obrigatório $package."

  path="$(package_module_path "$package" "$module")"
  [[ -n "$path" && -s "$path" ]] \
    || fail "O pacote $package não contém um payload válido para $module."
  [[ "$path" == *"/$expected_directory/"* ]] \
    || fail "O módulo $module de $package foi instalado fora de $expected_directory."

  owner="$(rpm_file_owner "$path")" \
    || fail "O RPM não confirmou o proprietário do arquivo de $module em $package."
  [[ "$owner" == "$package" ]] \
    || fail "O arquivo de $module não pertence ao pacote por kernel esperado $package."

  capture_clean_output modinfo -F vermagic "$path" \
    || fail "O modinfo não conseguiu ler o vermagic de $module em $package."
  vermagic="$PROBE_STDOUT"
  [[ "${vermagic%% *}" == "$kernel" ]] \
    || fail "O vermagic de $module em $package não corresponde ao kernel $kernel."
  if secure_boot_is_enabled; then
    capture_clean_output modinfo -F signer "$path" \
      || fail "O modinfo não conseguiu ler o signer de $module em $package."
    signer="$PROBE_STDOUT"
    capture_clean_output modinfo -F sig_key "$path" \
      || fail "O modinfo não conseguiu ler o sig_key de $module em $package."
    signer_serial="$(normalize_certificate_serial "$PROBE_STDOUT")" \
      || fail "O sig_key de $module em $package é inválido."
    [[ -n "$AKMODS_SIGNER_CN" && "$signer" == "$AKMODS_SIGNER_CN" ]] \
      || fail "O módulo $module de $package não foi assinado pela chave akmods atual."
    [[ -n "$AKMODS_SIGNER_SERIAL" && "$signer_serial" == "$AKMODS_SIGNER_SERIAL" ]] \
      || fail "A identidade criptográfica da assinatura de $module em $package não corresponde à chave akmods atual."
  fi
}

validate_resolved_module() {
  local kernel="$1"
  local package="$2"
  local module="$3"
  local expected_directory="$4"
  local path owner vermagic signer signer_serial

  capture_clean_output modinfo -k "$kernel" -n "$module" \
    || fail "Para $kernel, modinfo não conseguiu resolver $module."
  path="$PROBE_STDOUT"
  [[ -n "$path" && "$path" == *"/$expected_directory/"* ]] \
    || fail "Para $kernel, modinfo ainda resolve $module para '${path:-ausente}', não para $expected_directory."
  owner="$(rpm_file_owner "$path")" \
    || fail "Para $kernel, o RPM não confirmou o proprietário do módulo resolvido $module."
  [[ "$owner" == "$package" ]] \
    || fail "Para $kernel, o módulo resolvido $module não pertence a $package."
  capture_clean_output modinfo -k "$kernel" -F vermagic "$module" \
    || fail "Para $kernel, modinfo não conseguiu ler o vermagic de $module."
  vermagic="$PROBE_STDOUT"
  [[ "${vermagic%% *}" == "$kernel" ]] \
    || fail "Para $kernel, o vermagic do módulo resolvido $module não corresponde ao kernel."

  if secure_boot_is_enabled; then
    capture_clean_output modinfo -k "$kernel" -F signer "$module" \
      || fail "Para $kernel, modinfo não conseguiu ler o signer de $module."
    signer="$PROBE_STDOUT"
    capture_clean_output modinfo -k "$kernel" -F sig_key "$module" \
      || fail "Para $kernel, modinfo não conseguiu ler o sig_key de $module."
    signer_serial="$(normalize_certificate_serial "$PROBE_STDOUT")" \
      || fail "Para $kernel, o sig_key de $module é inválido."
    [[ -n "$AKMODS_SIGNER_CN" && "$signer" == "$AKMODS_SIGNER_CN" ]] \
      || fail "O módulo $module de $kernel não foi assinado pela chave akmods atual."
    [[ -n "$AKMODS_SIGNER_SERIAL" && "$signer_serial" == "$AKMODS_SIGNER_SERIAL" ]] \
      || fail "A identidade criptográfica da assinatura de $module em $kernel não corresponde à chave akmods atual."
  fi
}

enable_max_services() {
  systemctl enable max98390-hda-i2c-setup.service
  systemctl enable max98390-hda-check-upstream.service
}

finalize_ov_kernel() {
  local kernel="$1"
  local helper="/usr/libexec/galaxybook-ov02c10/finalize-kernel"

  [[ -x "$helper" ]] \
    || fail "O pacote galaxybook-ov02c10-kmod-common é antigo: falta $helper. Atualize-o antes de reparar o driver."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido antes do finalizador OV02C10."
  "$helper" --kernel "$kernel"
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido após o finalizador OV02C10."
}

run_max_finalizer() {
  local helper="$1"
  local kernel="$2"

  [[ -x "$helper" ]] \
    || fail "O pacote galaxybook-max98390-kmod-common é antigo: falta $helper. Atualize-o antes de reparar os alto-falantes."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido antes do finalizador MAX98390."
  "$helper" --kernel "$kernel"
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido após o finalizador MAX98390."
}

finalize_max_kernel() {
  local kernel="$1"
  local helper="/usr/libexec/galaxybook-max98390/max98390-kmod-finalize.sh"

  run_max_finalizer "$helper" "$kernel"
}

max_hardware_present() {
  local status

  if compgen -G '/sys/bus/acpi/devices/MAX98390:*' >/dev/null; then
    return 0
  else
    status=$?
    (( status == 1 )) || return 2
  fi
  if compgen -G '/sys/bus/acpi/devices/MX98390:*' >/dev/null; then
    return 0
  else
    status=$?
    (( status == 1 )) || return 2
  fi
  # Do not use grep -q here: GNU grep may return success immediately after a
  # match and mask a later read error in another sysfs entry.
  if grep -REs 'MA?X98390' /sys/bus/i2c/devices/*/name \
    >/dev/null 2>/dev/null; then
    return 0
  else
    status=$?
  fi
  (( status == 1 )) && return 1
  return 2
}

validate_max_service() {
  local helper="${1:-/usr/libexec/galaxybook-max98390/max98390-hda-i2c-setup.sh}"
  local active result binding_count hardware_status

  if max_hardware_present; then
    :
  else
    hardware_status=$?
    (( hardware_status == 1 )) && return 0
    fail "Não foi possível determinar com segurança se o hardware MAX98390 está presente."
  fi

  [[ -x "$helper" ]] \
    || fail "O pacote galaxybook-max98390-kmod-common é antigo: falta $helper. Atualize-o antes de ativar os alto-falantes."

  systemctl restart max98390-hda-i2c-setup.service \
    || fail "Não foi possível reiniciar o serviço MAX98390."
  capture_clean_output systemctl show max98390-hda-i2c-setup.service \
    --property=ActiveState --value \
    || fail "Não foi possível consultar ActiveState do serviço MAX98390."
  active="$PROBE_STDOUT"
  systemd_active_state_is_valid "$active" \
    || fail "O serviço MAX98390 retornou ActiveState inesperado: '${active:-vazio}'."
  capture_clean_output systemctl show max98390-hda-i2c-setup.service \
    --property=Result --value \
    || fail "Não foi possível consultar Result do serviço MAX98390."
  result="$PROBE_STDOUT"
  [[ "$active" == active && "$result" == success ]] \
    || fail "O serviço MAX98390 não terminou com ActiveState=active e Result=success."

  capture_clean_output find /sys/bus/i2c/drivers/max98390-hda \
    -mindepth 1 -maxdepth 1 -type l ! -name module -printf '.' \
    || fail "Não foi possível enumerar os vínculos I2C do MAX98390."
  binding_count="${#PROBE_STDOUT}"
  [[ "$binding_count" -ge 4 ]] \
    || fail "O serviço MAX98390 terminou, mas apenas $binding_count dos 4 vínculos I2C esperados estão ativos."
}

require_reconciliation_locks() {
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido."
  validate_rpm_read_lock \
    || fail "O lock compartilhado do RPM deixou de ser válido."
}

assert_locked_kernel_inventory() {
  local expected="$1"
  local current

  require_reconciliation_locks
  if ! current="$(installed_kernels)"; then
    fail "Não foi possível revalidar o inventário de kernels sob o lock RPM."
  fi
  [[ "$current" == "$expected" ]] \
    || fail "O inventário de kernels mudou durante a reconciliação; execute a ação novamente."
  require_reconciliation_locks
}

validate_all_packaged_modules() {
  local mode="$1"
  local kernel package
  local -a kernels=("${@:2}")

  for kernel in "${kernels[@]}"; do
    if [[ "$mode" == all || "$mode" == ov02c10 ]]; then
      package="$OV_PACKAGE_PREFIX-$kernel"
      validate_packaged_module "$kernel" "$package" "$OV_MODULE" "extra/galaxybook-ov02c10"
    fi
    if [[ "$mode" == all || "$mode" == max98390 ]]; then
      package="$MAX_PACKAGE_PREFIX-$kernel"
      validate_packaged_module "$kernel" "$package" "$MAX_MODULE_CORE" "extra/galaxybook-max98390"
      validate_packaged_module "$kernel" "$package" "$MAX_MODULE_I2C" "extra/galaxybook-max98390"
    fi
    require_reconciliation_locks
  done
}

run_driver_finalizers() {
  local mode="$1"
  local kernel
  local -a kernels=("${@:2}")

  [[ -z "$reconcile_rpm_lock_fd" ]] \
    || fail "O lock RPM do coordenador não foi liberado antes dos finalizadores."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido antes dos finalizadores."

  if [[ "$mode" == all || "$mode" == max98390 ]]; then
    for kernel in "${kernels[@]}"; do
      finalize_max_kernel "$kernel"
      validate_kmod_lifecycle_lock \
        || fail "O lock global de manutenção de módulos deixou de ser válido após o finalizador MAX98390."
    done
  fi
  if [[ "$mode" == all || "$mode" == ov02c10 ]]; then
    for kernel in "${kernels[@]}"; do
      finalize_ov_kernel "$kernel"
      validate_kmod_lifecycle_lock \
        || fail "O lock global de manutenção de módulos deixou de ser válido após o finalizador OV02C10."
    done
  fi
}

validate_all_resolved_modules() {
  local mode="$1"
  local kernel package
  local -a kernels=("${@:2}")

  for kernel in "${kernels[@]}"; do
    if [[ "$mode" == all || "$mode" == ov02c10 ]]; then
      package="$OV_PACKAGE_PREFIX-$kernel"
      validate_resolved_module "$kernel" "$package" "$OV_MODULE" "extra/galaxybook-ov02c10"
    fi
    if [[ "$mode" == all || "$mode" == max98390 ]]; then
      package="$MAX_PACKAGE_PREFIX-$kernel"
      validate_resolved_module "$kernel" "$package" "$MAX_MODULE_CORE" "extra/galaxybook-max98390"
      validate_resolved_module "$kernel" "$package" "$MAX_MODULE_I2C" "extra/galaxybook-max98390"
    fi
    require_reconciliation_locks
  done
}

require_enrolled_mok_for_preparation() {
  local queue

  secure_boot_is_enabled \
    || fail "A preparação em disco exige o Secure Boot ativo."
  akmods_key_enrolled \
    || fail "A preparação em disco exige que a chave atual do akmods já esteja inscrita no MOK."
  # A currently trusted key can still be scheduled for deletion. Do not
  # certify the next boot while any MOK change is pending or unobservable.
  for queue in --list-new --list-delete; do
    capture_clean_output_preserving_newlines mokutil "$queue" \
      || fail "Não foi possível confirmar o estado dos pedidos pendentes do MOK."
    [[ -z "$PROBE_STDOUT" ]] \
      || fail "Há uma alteração pendente no MOK. A preparação em disco não foi confirmada."
  done
}

complete_reconciliation() {
  local mode="$1"
  local expected_kernel_output="$2"
  local mok_requires_reboot=false prepare_only=false
  local -a kernels=("${@:3}")

  if [[ "$mode" == prepare-all ]]; then
    mode=all
    prepare_only=true
  fi
  [[ -n "$expected_kernel_output" ]] && (( ${#kernels[@]} > 0 )) \
    || fail "A finalização não recebeu nenhum kernel validado."

  # Phase 2: every DNF/akmods writer above has finished. Take an RPM6 OFD
  # snapshot, require the exact initial kernel inventory and prevalidate every
  # replacement before any package-owned finalizer migrates legacy state.
  acquire_rpm_read_lock
  assert_locked_kernel_inventory "$expected_kernel_output"
  validate_all_packaged_modules "$mode" "${kernels[@]}"
  require_reconciliation_locks
  release_rpm_read_lock \
    || fail "Não foi possível liberar o lock RPM antes dos finalizadores."

  # Phase 3: finalizers own their private locks and then acquire RPM shared.
  # Never carry the parent's RPM lock into them; the universal order remains
  # GLOBAL -> driver-private -> RPM.
  run_driver_finalizers "$mode" "${kernels[@]}"

  # Phase 4: establish a fresh stable RPM snapshot after all finalizers. Repeat
  # package, owner, path, vermagic and signing checks, then hold the snapshot
  # through resolved-module checks, live MAX work and the final acknowledgement.
  acquire_rpm_read_lock
  assert_locked_kernel_inventory "$expected_kernel_output"
  validate_all_packaged_modules "$mode" "${kernels[@]}"
  validate_all_resolved_modules "$mode" "${kernels[@]}"

  if "$prepare_only"; then
    require_enrolled_mok_for_preparation
  elif mok_reboot_required; then
    mok_requires_reboot=true
  fi
  require_reconciliation_locks
  if ! "$mok_requires_reboot"; then
    publish_enrolled_identity_if_needed
  fi

  if ! "$prepare_only" && [[ "$mode" == all || "$mode" == max98390 ]]; then
    enable_max_services
    require_reconciliation_locks
    if ! "$mok_requires_reboot"; then
      validate_max_service
      require_reconciliation_locks
    else
      log "Validação MAX98390 ao vivo adiada até o Enroll MOK; os payloads em disco já foram validados."
    fi
  fi

  if "$mok_requires_reboot"; then
    require_reconciliation_locks
    log "Payloads prontos e pedido da chave do akmods pendente no MOK. O único próximo passo é reiniciar normalmente e concluir o Enroll MOK."
  fi

  require_reconciliation_locks
  if "$prepare_only"; then
    log "Preparação em disco validada para: ${kernels[*]}. A chave do akmods está inscrita no MOK. A câmera e o áudio ainda precisam de validação após um novo boot; nenhum reinício foi solicitado."
  else
    log "Payloads RPM e finalização por kernel validados para: ${kernels[*]}. Reinicie normalmente; módulos já ocupados não foram trocados ao vivo."
  fi
  require_reconciliation_locks
  release_rpm_read_lock \
    || fail "Não foi possível liberar corretamente o lock compartilhado do RPM."
}

main() {
  local invocation mode build_scope
  local kernel kernel_output
  local -a kernels

  case "$#" in
    1)
      invocation=public
      mode="$1"
      ;;
    2)
      invocation="$1"
      mode="$2"
      [[ "$invocation" == --inhibited || "$invocation" == --coordinated ]] \
        || usage
      ;;
    *) usage ;;
  esac
  case "$mode" in
    all|ov02c10|max98390) build_scope="$mode" ;;
    prepare-all)
      [[ "$invocation" != --coordinated ]] || usage
      build_scope=all
      ;;
    *) usage ;;
  esac
  caller_is_root || fail "Este helper precisa ser executado como root."
  if [[ "$invocation" == public ]]; then
    enter_systemd_inhibitor "$mode"
  fi
  inhibitor_parent_is_trusted \
    || fail "A reconciliação interna não está sob o inhibitor esperado do systemd."

  [[ -r "$lifecycle_helper" ]] \
    || fail "O helper do lock global de módulos está ausente: $lifecycle_helper."
  # shellcheck source=kmod-lifecycle-lock.sh
  source "$lifecycle_helper"
  if [[ "$invocation" == --coordinated ]]; then
    [[ -n "${GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD:-}" ]] \
      || fail "A reconciliação coordenada não herdou o lock global de módulos."
    validate_kmod_lifecycle_lock \
      || fail "A reconciliação coordenada herdou um lock global inválido."
  fi
  acquire_kmod_lifecycle_lock \
    || fail "Não foi possível adquirir o lock global de manutenção de módulos."

  require_command akmods
  require_command awk
  require_command cat
  require_command cmp
  require_command dnf
  require_command dirname
  require_command find
  require_command flock
  require_command grep
  require_command modinfo
  require_command mktemp
  require_command mokutil
  require_command openssl
  require_command rpm
  require_command sleep
  require_command sort
  require_command stat
  require_command systemctl
  require_command tr
  require_command uname
  require_command wc

  detect_secure_boot_state
  # Phase 1: capture the canonical sorted kernel inventory under RPM shared,
  # then release it before any DNF or akmods writer can run.
  acquire_rpm_read_lock
  if ! kernel_output="$(installed_kernels)"; then
    fail "Não foi possível enumerar o kernel em execução e todos os pacotes kernel-core instalados."
  fi
  require_reconciliation_locks
  release_rpm_read_lock \
    || fail "Não foi possível liberar o lock RPM após o inventário inicial."
  mapfile -t kernels <<< "$kernel_output"
  (( ${#kernels[@]} > 0 )) || fail "Nenhum kernel instalado foi encontrado."
  ensure_signing_key_available
  detect_akmods_signing_identity
  if [[ "$mode" == prepare-all ]]; then
    require_enrolled_mok_for_preparation
  fi

  for kernel in "${kernels[@]}"; do
    kernel_name_is_safe "$kernel" \
      || fail "A versão de kernel '$kernel' contém caracteres inseguros."
    [[ -d "$module_tree_root/$kernel" ]] \
      || fail "A árvore $module_tree_root/$kernel está ausente."
    if [[ "$build_scope" == all || "$build_scope" == ov02c10 ]]; then
      build_driver_for_kernel "$OV_AKMOD" "$kernel"
    fi
    if [[ "$build_scope" == all || "$build_scope" == max98390 ]]; then
      build_driver_for_kernel "$MAX_AKMOD" "$kernel"
    fi
  done
  complete_reconciliation "$mode" "$kernel_output" "${kernels[@]}"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
