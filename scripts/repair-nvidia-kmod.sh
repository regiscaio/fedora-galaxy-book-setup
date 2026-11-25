#!/usr/bin/bash

set -euo pipefail
export LC_ALL=C
export PATH=/usr/sbin:/usr/bin

nvidia_script="/usr/libexec/galaxybook-setup/repair-nvidia-kmod"
lifecycle_helper="/usr/libexec/galaxybook-setup/kmod-lifecycle-lock.sh"
systemd_inhibit_command="/usr/bin/systemd-inhibit"
systemd_inhibit_stat_command="/usr/bin/stat"
nvidia_inhibitor_parent_fixture=""
nvidia_kernel_fixture=""
modules_root="/lib/modules"
rpm_lock_path="/usr/lib/sysimage/rpm/.rpm.lock"
rpm_lock_timeout_seconds="90"
rpm_lock_expected_uid="0"
rpm_lock_expected_gid="0"
rpm_lock_ancestry=(/usr /usr/lib /usr/lib/sysimage /usr/lib/sysimage/rpm)
nvidia_rpm_lock_fd=""

log() {
  printf 'galaxybook-setup: %s\n' "$*"
}

fail() {
  printf 'galaxybook-setup: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 \
    || fail "O comando obrigatório '$1' não está disponível."
}

caller_is_root() {
  (( EUID == 0 ))
}

kernel_name_is_safe() {
  local kernel="$1"

  [[ -n "$kernel" && ${#kernel} -le 128 ]] \
    && [[ "${kernel:0:1}" =~ ^[0-9]$ ]] \
    && [[ "${kernel: -1}" =~ ^[A-Za-z0-9]$ ]] \
    && [[ "$kernel" != *..* ]] \
    && [[ "$kernel" =~ ^[A-Za-z0-9._+-]+$ ]]
}

running_kernel() {
  local error_file="" kernel output_file="" raw_size status

  if [[ -n "$nvidia_kernel_fixture" ]]; then
    kernel="$nvidia_kernel_fixture"
  else
    output_file="$(mktemp /var/tmp/galaxybook-setup-nvidia-uname.XXXXXX)" \
      || fail "Não foi possível preparar a consulta do kernel em execução."
    error_file="$(mktemp /var/tmp/galaxybook-setup-nvidia-uname.XXXXXX)" || {
      rm -f -- "$output_file"
      fail "Não foi possível preparar a consulta do kernel em execução."
    }
    if uname -r >"$output_file" 2>"$error_file"; then
      status=0
    else
      status=$?
    fi
    if (( status != 0 )) || [[ -s "$error_file" ]] \
      || ! IFS= read -r kernel <"$output_file"; then
      rm -f -- "$output_file" "$error_file"
      fail "Não foi possível identificar o kernel em execução de forma conclusiva."
    fi
    raw_size="$(wc -c < "$output_file")" || {
      rm -f -- "$output_file" "$error_file"
      fail "Não foi possível validar a resposta da consulta do kernel."
    }
    rm -f -- "$output_file" "$error_file"
    [[ "$raw_size" =~ ^[0-9]+$ ]] \
      && (( raw_size == ${#kernel} + 1 )) \
      || fail "A consulta do kernel retornou uma resposta incompleta ou com múltiplas linhas."
  fi
  [[ "$kernel" != *$'\n'* && "$kernel" != *$'\r'* ]] \
    && kernel_name_is_safe "$kernel" \
    || fail "A versão do kernel em execução não é segura."
  printf '%s\n' "$kernel"
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
  rpm_lock_fd_matches_path "$nvidia_rpm_lock_fd" || return 1
  flock --fcntl --shared --nonblock "$nvidia_rpm_lock_fd" || return 1
  rpm_lock_ancestry_is_safe || return 1
  rpm_lock_fd_matches_path "$nvidia_rpm_lock_fd"
}

acquire_rpm_read_lock() {
  rpm_lock_ancestry_is_safe \
    || fail "A hierarquia do lock RPM está ausente ou é insegura."
  rpm_lock_file_is_safe "$rpm_lock_path" \
    || fail "O arquivo de lock RPM está ausente ou é inseguro."
  exec {nvidia_rpm_lock_fd}<"$rpm_lock_path" \
    || fail "Não foi possível abrir o lock RPM."
  if ! rpm_lock_ancestry_is_safe \
    || ! rpm_lock_fd_matches_path "$nvidia_rpm_lock_fd"; then
    exec {nvidia_rpm_lock_fd}>&-
    fail "O lock RPM mudou enquanto era aberto."
  fi
  if ! flock --fcntl --shared --timeout "$rpm_lock_timeout_seconds" \
    "$nvidia_rpm_lock_fd"; then
    exec {nvidia_rpm_lock_fd}>&-
    fail "A manutenção NVIDIA expirou aguardando a transação RPM ativa."
  fi
  validate_rpm_read_lock || {
    exec {nvidia_rpm_lock_fd}>&-
    fail "O lock RPM mudou depois da aquisição."
  }
}

inhibitor_parent_is_trusted() {
  local parent="${PPID:-}"
  local executable_metadata executable_mode
  local parent_identity executable_identity

  if [[ "$nvidia_inhibitor_parent_fixture" == 1 ]]; then
    return 0
  fi
  [[ -z "$nvidia_inhibitor_parent_fixture" \
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
  [[ -x "$systemd_inhibit_command" ]] \
    || fail "O systemd-inhibit obrigatório está ausente: $systemd_inhibit_command."
  [[ -x "$nvidia_script" ]] \
    || fail "O helper NVIDIA não é executável: $nvidia_script."

  log "Bloqueando shutdown, suspensão e idle durante a manutenção NVIDIA"
  exec "$systemd_inhibit_command" \
    --mode=block \
    --what=shutdown:sleep:idle \
    --who="Galaxy Book Setup" \
    --why="Gerando e finalizando o módulo NVIDIA para o kernel atual" \
    "$nvidia_script" --inhibited \
    || fail "Não foi possível adquirir o inhibitor do systemd para proteger a manutenção NVIDIA."
}

main() {
  local invocation kernel

  case "$#" in
    0) invocation=public ;;
    1)
      [[ "$1" == --inhibited ]] \
        || fail "Este helper não aceita esse argumento."
      invocation=inhibited
      ;;
    *) fail "Este helper não aceita argumentos adicionais." ;;
  esac
  caller_is_root || fail "Este helper precisa ser executado como root."
  if [[ "$invocation" == public ]]; then
    enter_systemd_inhibitor
  fi
  inhibitor_parent_is_trusted \
    || fail "A manutenção NVIDIA interna não está sob o inhibitor esperado do systemd."

  [[ -r "$lifecycle_helper" ]] \
    || fail "O helper do lock global de módulos está ausente: $lifecycle_helper."
  # shellcheck source=kmod-lifecycle-lock.sh
  source "$lifecycle_helper"

  require_command akmods
  require_command depmod
  require_command dnf
  require_command dirname
  require_command dracut
  require_command flock
  require_command mktemp
  require_command rm
  require_command stat
  require_command uname
  require_command wc

  acquire_kmod_lifecycle_lock \
    || fail "Não foi possível adquirir o lock global de manutenção de módulos."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos não pôde ser revalidado."
  kernel="$(running_kernel)"
  [[ -d "$modules_root/$kernel" ]] \
    || fail "A árvore $modules_root/$kernel está ausente."

  log "Instalando e reconstruindo o akmod-nvidia para $kernel"
  dnf install -y akmod-nvidia
  akmods --force --akmod nvidia --kernels "$kernel"

  log "Atualizando os índices e o initramfs de $kernel"
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido após o akmods."
  acquire_rpm_read_lock
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido antes do depmod."
  depmod -a "$kernel"
  dracut --force --kver "$kernel"
  validate_rpm_read_lock \
    || fail "O lock RPM deixou de ser válido durante a manutenção NVIDIA."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido durante a manutenção NVIDIA."
  log "Manutenção NVIDIA concluída para $kernel."
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
