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
readonly INHIBITOR_MARKER="GALAXYBOOK_SETUP_RECONCILE_INHIBITED"
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
  printf 'Uso: %s {all|ov02c10|max98390}\n' "$0" >&2
  exit 2
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "O comando obrigatório '$1' não está disponível."
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

enter_systemd_inhibitor() {
  local mode="$1"
  local script_path="${BASH_SOURCE[0]}"

  if [[ "${!INHIBITOR_MARKER:-}" == 1 ]]; then
    unset "$INHIBITOR_MARKER"
    return 0
  fi

  require_command systemd-inhibit
  [[ -x "$script_path" ]] \
    || fail "O helper de reconciliação não é executável: $script_path."

  log "Bloqueando shutdown, suspensão e idle até todos os payloads terminarem"
  export "$INHIBITOR_MARKER=1"
  exec systemd-inhibit \
    --mode=block \
    --what=shutdown:sleep:idle \
    --who="Galaxy Book Setup" \
    --why="Gerando e validando módulos externos para todos os kernels" \
    "$script_path" "$mode"
  fail "Não foi possível adquirir o inhibitor do systemd para proteger a reconciliação."
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

build_driver_for_kernel() {
  local akmod="$1"
  local kernel="$2"

  wait_for_akmods_unit akmods.service
  wait_for_akmods_unit "akmods@$kernel.service"
  ensure_kernel_build_tree "$kernel"

  log "Instalando ou atualizando $akmod para $kernel"
  akmods --force --akmod "$akmod" --kernels "$kernel"
  log "Reconstruindo $akmod para $kernel com a chave atual"
  akmods --force --rebuild --akmod "$akmod" --kernels "$kernel"
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
  "$helper" --kernel "$kernel"
}

run_max_finalizer() {
  local helper="$1"
  local kernel="$2"

  [[ -x "$helper" ]] \
    || fail "O pacote galaxybook-max98390-kmod-common é antigo: falta $helper. Atualize-o antes de reparar os alto-falantes."
  "$helper" --kernel "$kernel"
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

main() {
  local mode="${1:-}"
  local kernel kernel_output package
  local mok_requires_reboot=false
  local -a kernels

  case "$mode" in
    all|ov02c10|max98390) ;;
    *) usage ;;
  esac
  (( EUID == 0 )) || fail "Este helper precisa ser executado como root."
  enter_systemd_inhibitor "$mode"

  require_command akmods
  require_command awk
  require_command cat
  require_command cmp
  require_command dnf
  require_command find
  require_command grep
  require_command modinfo
  require_command mktemp
  require_command mokutil
  require_command openssl
  require_command rpm
  require_command sleep
  require_command sort
  require_command systemctl
  require_command tr
  require_command uname
  require_command wc

  detect_secure_boot_state
  if ! kernel_output="$(installed_kernels)"; then
    fail "Não foi possível enumerar o kernel em execução e todos os pacotes kernel-core instalados."
  fi
  mapfile -t kernels <<< "$kernel_output"
  (( ${#kernels[@]} > 0 )) || fail "Nenhum kernel instalado foi encontrado."
  ensure_signing_key_available
  detect_akmods_signing_identity

  for kernel in "${kernels[@]}"; do
    kernel_name_is_safe "$kernel" \
      || fail "A versão de kernel '$kernel' contém caracteres inseguros."
    [[ -d "/lib/modules/$kernel" ]] \
      || fail "A árvore /lib/modules/$kernel está ausente."
    if [[ "$mode" == all || "$mode" == ov02c10 ]]; then
      build_driver_for_kernel "$OV_AKMOD" "$kernel"
    fi
    if [[ "$mode" == all || "$mode" == max98390 ]]; then
      build_driver_for_kernel "$MAX_AKMOD" "$kernel"
    fi
  done

  # Do not ask package-owned finalizers to migrate legacy state until every
  # replacement is an RPM-owned payload for every installed kernel.
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
  done

  if [[ "$mode" == all || "$mode" == max98390 ]]; then
    for kernel in "${kernels[@]}"; do
      finalize_max_kernel "$kernel"
    done
  fi

  for kernel in "${kernels[@]}"; do
    if [[ "$mode" == all || "$mode" == ov02c10 ]]; then
      finalize_ov_kernel "$kernel"
      package="$OV_PACKAGE_PREFIX-$kernel"
      validate_resolved_module "$kernel" "$package" "$OV_MODULE" "extra/galaxybook-ov02c10"
    fi
    if [[ "$mode" == all || "$mode" == max98390 ]]; then
      package="$MAX_PACKAGE_PREFIX-$kernel"
      validate_resolved_module "$kernel" "$package" "$MAX_MODULE_CORE" "extra/galaxybook-max98390"
      validate_resolved_module "$kernel" "$package" "$MAX_MODULE_I2C" "extra/galaxybook-max98390"
    fi
  done

  if mok_reboot_required; then
    mok_requires_reboot=true
  fi

  if [[ "$mode" == all || "$mode" == max98390 ]]; then
    enable_max_services
    if ! "$mok_requires_reboot"; then
      validate_max_service
    else
      log "Validação MAX98390 ao vivo adiada até o Enroll MOK; os payloads em disco já foram validados."
    fi
  fi

  if "$mok_requires_reboot"; then
    log "Payloads prontos e pedido da chave do akmods pendente no MOK. O único próximo passo é reiniciar normalmente e concluir o Enroll MOK."
  fi

  log "Payloads RPM e finalização por kernel validados para: ${kernels[*]}. Reinicie normalmente; módulos já ocupados não foram trocados ao vivo."
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
