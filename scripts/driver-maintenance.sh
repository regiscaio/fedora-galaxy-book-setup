#!/usr/bin/bash

set -euo pipefail
export LC_ALL=C
export PATH=/usr/sbin:/usr/bin

maintenance_script="/usr/libexec/galaxybook-setup/driver-maintenance"
lifecycle_helper="/usr/libexec/galaxybook-setup/kmod-lifecycle-lock.sh"
reconcile_helper="/usr/libexec/galaxybook-setup/reconcile-kmods"
browser_camera_helper="/usr/libexec/galaxybook-setup/configure-browser-camera"
systemd_inhibit_command="/usr/bin/systemd-inhibit"
systemd_inhibit_stat_command="/usr/bin/stat"
maintenance_inhibitor_parent_fixture=""
rpm_inventory=""

readonly -a UPGRADE_PACKAGES=(
  galaxybook-setup
  galaxybook-camera
  galaxybook-ov02c10-kmod-common
  akmod-galaxybook-ov02c10
  kmod-galaxybook-ov02c10
  galaxybook-sound
  galaxybook-max98390-kmod-common
  akmod-galaxybook-max98390
  kmod-galaxybook-max98390
)

log() {
  printf 'galaxybook-setup: %s\n' "$*"
}

fail() {
  printf 'galaxybook-setup: %s\n' "$*" >&2
  exit 1
}

usage() {
  printf 'Uso: %s {install-main|install-camera|enable-browser-camera|enable-speakers|upgrade}\n' \
    "$0" >&2
  exit 2
}

require_command() {
  command -v "$1" >/dev/null 2>&1 \
    || fail "O comando obrigatório '$1' não está disponível."
}

caller_is_root() {
  (( EUID == 0 ))
}

maintenance_mode_is_valid() {
  case "$1" in
    install-main|install-camera|enable-browser-camera|enable-speakers|upgrade) return 0 ;;
    *) return 1 ;;
  esac
}

inhibitor_parent_is_trusted() {
  local parent="${PPID:-}"
  local executable_metadata executable_mode
  local parent_identity executable_identity

  if [[ "$maintenance_inhibitor_parent_fixture" == 1 ]]; then
    return 0
  fi
  [[ -z "$maintenance_inhibitor_parent_fixture" \
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
  [[ -x "$maintenance_script" ]] \
    || fail "O coordenador de manutenção não é executável: $maintenance_script."

  log "Bloqueando shutdown, suspensão e idle antes da manutenção de módulos"
  exec "$systemd_inhibit_command" \
    --mode=block \
    --what=shutdown:sleep:idle \
    --who="Galaxy Book Setup" \
    --why="Instalando e validando módulos externos com segurança" \
    "$maintenance_script" --inhibited "$mode" \
    || fail "Não foi possível adquirir o inhibitor do systemd para proteger a manutenção."
}

load_rpm_inventory() {
  local error_file inventory_file status

  error_file="$(mktemp /var/tmp/galaxybook-setup-update-rpm.XXXXXX)" \
    || fail "Não foi possível preparar o inventário RPM."
  inventory_file="$(mktemp /var/tmp/galaxybook-setup-update-rpm.XXXXXX)" || {
    rm -f -- "$error_file"
    fail "Não foi possível preparar o inventário RPM."
  }
  if rpm -qa --qf '%{NAME}\n' >"$inventory_file" 2>"$error_file"; then
    status=0
  else
    status=$?
  fi
  if (( status != 0 )) \
    || [[ -s "$error_file" || ! -s "$inventory_file" ]] \
    || ! tail -c 1 -- "$inventory_file" | grep -qx ''; then
    rm -f -- "$error_file" "$inventory_file"
    fail "O RPM não conseguiu produzir um inventário conclusivo dos pacotes instalados. A atualização foi interrompida sem assumir que pacotes estão ausentes."
  fi
  if ! awk '
    $0 !~ /^[[:alnum:]][[:alnum:]+._-]*$/ { invalid = 1 }
    END { exit invalid ? 1 : 0 }
  ' "$inventory_file"; then
    rm -f -- "$error_file" "$inventory_file"
    fail "O RPM retornou um nome de pacote malformado. A atualização foi interrompida sem usar um inventário parcial."
  fi
  rpm_inventory="$(<"$inventory_file")"
  rm -f -- "$error_file" "$inventory_file"
}

package_is_installed() {
  local expected="$1" candidate

  while IFS= read -r candidate; do
    [[ "$candidate" == "$expected" ]] && return 0
  done <<< "$rpm_inventory"
  return 1
}

count_installed_family() {
  local count=0 package

  for package in "$@"; do
    if package_is_installed "$package"; then
      count=$((count + 1))
    fi
  done
  printf '%s\n' "$count"
}

exec_reconcile() {
  local mode="$1"

  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido."
  [[ -x "$reconcile_helper" ]] \
    || fail "O helper de reconciliação está ausente: $reconcile_helper."
  exec "$reconcile_helper" --coordinated "$mode" \
    || fail "Não foi possível iniciar a reconciliação de módulos."
}

install_main_support() {
  dnf --refresh install -y \
    'galaxybook-camera >= 1.00.43' \
    'galaxybook-ov02c10-kmod-common >= 1.00.25' \
    'akmod-galaxybook-ov02c10 >= 1.00.25' \
    'kmod-galaxybook-ov02c10 >= 1.00.25' \
    'galaxybook-max98390-kmod-common >= 1.00.12' \
    'akmod-galaxybook-max98390 >= 1.00.12' \
    'kmod-galaxybook-max98390 >= 1.00.12' \
    i2c-tools
  exec_reconcile all
}

install_camera_support() {
  dnf --refresh install -y \
    'galaxybook-ov02c10-kmod-common >= 1.00.25' \
    'akmod-galaxybook-ov02c10 >= 1.00.25' \
    'kmod-galaxybook-ov02c10 >= 1.00.25' \
    'galaxybook-camera >= 1.00.43'
  exec_reconcile ov02c10
}

enable_speakers() {
  dnf --refresh install -y \
    'galaxybook-max98390-kmod-common >= 1.00.12' \
    'akmod-galaxybook-max98390 >= 1.00.12' \
    'kmod-galaxybook-max98390 >= 1.00.12' \
    i2c-tools
  exec_reconcile max98390
}

enable_browser_camera() {
  dnf install -y \
    v4l2-relayd \
    v4l2loopback \
    gstreamer1-plugins-icamerasrc \
    v4l-utils
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido após o DNF."
  [[ -x "$browser_camera_helper" ]] \
    || fail "O helper da câmera para navegador está ausente: $browser_camera_helper."
  "$browser_camera_helper"
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido durante a configuração da câmera."
}

upgrade_installed_packages() {
  local package ov_count max_count
  local -a installed=()

  load_rpm_inventory
  for package in "${UPGRADE_PACKAGES[@]}"; do
    if package_is_installed "$package"; then
      installed+=("$package")
    fi
  done
  if (( ${#installed[@]} == 0 )); then
    log "Nenhum pacote do Galaxy Book está instalado para atualizar."
    return 0
  fi

  ov_count="$(count_installed_family \
    galaxybook-ov02c10-kmod-common \
    akmod-galaxybook-ov02c10 \
    kmod-galaxybook-ov02c10)"
  max_count="$(count_installed_family \
    galaxybook-max98390-kmod-common \
    akmod-galaxybook-max98390 \
    kmod-galaxybook-max98390)"
  if (( ov_count > 0 && ov_count < 3 )); then
    fail "A família OV02C10 está parcialmente instalada. Use 'Instalar suporte da câmera' ou 'Reparar o driver' para completar os três pacotes antes de atualizar."
  fi
  if (( max_count > 0 && max_count < 3 )); then
    fail "A família MAX98390 está parcialmente instalada. Use 'Ativar alto-falantes internos' para completar os três pacotes antes de atualizar."
  fi

  dnf --refresh upgrade -y "${installed[@]}"
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido após o DNF."
  load_rpm_inventory
  ov_count="$(count_installed_family \
    galaxybook-ov02c10-kmod-common \
    akmod-galaxybook-ov02c10 \
    kmod-galaxybook-ov02c10)"
  max_count="$(count_installed_family \
    galaxybook-max98390-kmod-common \
    akmod-galaxybook-max98390 \
    kmod-galaxybook-max98390)"
  if (( ov_count > 0 && ov_count < 3 )); then
    fail "A atualização deixou a família OV02C10 parcialmente instalada. Complete os três pacotes pela ação de câmera antes de reiniciar."
  fi
  if (( max_count > 0 && max_count < 3 )); then
    fail "A atualização deixou a família MAX98390 parcialmente instalada. Complete os três pacotes pela ação de alto-falantes antes de reiniciar."
  fi

  case "$ov_count:$max_count" in
    3:3) exec_reconcile all ;;
    3:0) exec_reconcile ov02c10 ;;
    0:3) exec_reconcile max98390 ;;
    0:0)
      log "Atualizações concluídas; nenhuma família completa de driver estava instalada para reconciliar."
      ;;
    *) fail "O inventário RPM final possui uma combinação de drivers inesperada." ;;
  esac
}

run_inhibited_maintenance() {
  local mode="$1"

  inhibitor_parent_is_trusted \
    || fail "A fase interna de manutenção não está sob o inhibitor esperado do systemd."
  [[ -r "$lifecycle_helper" ]] \
    || fail "O helper do lock global de módulos está ausente: $lifecycle_helper."
  # shellcheck source=kmod-lifecycle-lock.sh
  source "$lifecycle_helper"

  require_command dnf
  if [[ "$mode" == upgrade ]]; then
    require_command awk
    require_command grep
    require_command mktemp
    require_command rm
    require_command rpm
    require_command tail
  fi
  acquire_kmod_lifecycle_lock \
    || fail "Não foi possível adquirir o lock global de manutenção de módulos."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos não pôde ser revalidado."

  case "$mode" in
    install-main) install_main_support ;;
    install-camera) install_camera_support ;;
    enable-browser-camera) enable_browser_camera ;;
    enable-speakers) enable_speakers ;;
    upgrade) upgrade_installed_packages ;;
    *) usage ;;
  esac
}

main() {
  local mode

  caller_is_root || fail "Este helper precisa ser executado como root."
  case "$#" in
    1)
      mode="$1"
      maintenance_mode_is_valid "$mode" || usage
      enter_systemd_inhibitor "$mode"
      ;;
    2)
      [[ "$1" == --inhibited ]] || usage
      mode="$2"
      maintenance_mode_is_valid "$mode" || usage
      run_inhibited_maintenance "$mode"
      ;;
    *) usage ;;
  esac
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
