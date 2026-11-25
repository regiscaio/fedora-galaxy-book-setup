#!/usr/bin/bash

set -euo pipefail
export LC_ALL=C
export PATH=/usr/sbin:/usr/bin

readonly ETC_ROOT="${GALAXYBOOK_ETC_ROOT:-/etc}"
readonly SYS_ROOT="${GALAXYBOOK_SYS_ROOT:-/sys}"
lifecycle_helper="${BASH_SOURCE[0]%/*}/kmod-lifecycle-lock.sh"
WORKDIR=""

# These defaults are deliberately not configurable through the environment.
# The test suite sources this file and replaces them only inside its own shell.
MANAGED_FILE_EXPECTED_OWNER="0:0"
MANAGED_PATH_ANCHOR="${ETC_ROOT%/}"
[[ -n "$MANAGED_PATH_ANCHOR" ]] || MANAGED_PATH_ANCHOR="/"
ACTION_LOCK_EXPECTED_OWNER="0:0"
ACTION_LOCK_PATH="/run/lock/galaxybook-setup-browser-camera.lock"
ACTION_LOCK_PATH_ANCHOR="/"
ACTION_LOCK_TIMEOUT_SECONDS="30"
ACTION_LOCK_FD=""
RPM_LOCK_EXPECTED_OWNER="0:0"
RPM_LOCK_PATH="/usr/lib/sysimage/rpm/.rpm.lock"
RPM_LOCK_PATH_ANCHOR="/"
RPM_LOCK_TIMEOUT_SECONDS="30"
FLOCK_COMMAND="/usr/bin/flock"
RPM_READ_LOCK_FD=""
OPENED_MANAGED_FILE_FD=""
OPENED_MANAGED_FILE_SNAPSHOT=""
OPENED_CONFIGURATION_FILE_FD=""
OPENED_CONFIGURATION_FILE_SNAPSHOT=""
OPENED_CONFIGURATION_FILE_LINKS=""
readonly MAX_CONFIGURATION_FILE_SIZE=65536
BRIDGE_RUNTIME_LINK="/run/v4l2-relayd"
BRIDGE_DEFAULTS_ROOT="/usr/share/defaults/etc/camera"
BRIDGE_LINK_PACKAGE="v4l2-relayd"
BRIDGE_TARGET_PACKAGE="ipu6-camera-hal"

log() {
  printf 'galaxybook-setup: %s\n' "$*"
}

fail() {
  printf 'galaxybook-setup: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "O comando obrigatório '$1' não está disponível."
}

caller_is_root() {
  (( EUID == 0 ))
}

cleanup() {
  if [[ -n "$WORKDIR" && -d "$WORKDIR" ]]; then
    rm -rf -- "$WORKDIR"
  fi
  if [[ "$OPENED_CONFIGURATION_FILE_FD" =~ ^[0-9]+$ ]]; then
    exec {OPENED_CONFIGURATION_FILE_FD}<&-
  fi
  OPENED_CONFIGURATION_FILE_FD=""
  OPENED_CONFIGURATION_FILE_SNAPSHOT=""
  OPENED_CONFIGURATION_FILE_LINKS=""
}

path_has_symlink_component() {
  local path="$1"
  local component current=""
  local -a components

  [[ "$path" == /* ]] || return 0
  IFS='/' read -r -a components <<< "$path"
  for component in "${components[@]}"; do
    [[ -n "$component" ]] || continue
    [[ "$component" != "." && "$component" != ".." ]] || return 0
    current="$current/$component"
    [[ ! -L "$current" ]] || return 0
  done
  return 1
}

directory_component_is_safe() {
  local directory="$1"
  local expected_owner="$2"
  local metadata mode

  if path_has_symlink_component "$directory" \
    || [[ -L "$directory" || ! -d "$directory" ]]; then
    return 1
  fi
  metadata="$(stat -Lc '%u:%g:%a' -- "$directory")" || return 1
  mode="${metadata##*:}"
  if [[ "${metadata%:*}" != "$expected_owner" \
    || ! "$mode" =~ ^[0-7]{3,4}$ ]]; then
    return 1
  fi
  (( ((8#${mode}) & 8#022) == 0 ))
}

directory_ancestry_is_safe() {
  local anchor="$1"
  local directory="$2"
  local expected_owner="$3"
  local component current relative
  local -a components

  [[ "$anchor" == /* && "$directory" == /* ]] || return 1
  if [[ "$anchor" == "/" ]]; then
    relative="${directory#/}"
  else
    [[ "$directory" == "$anchor" || "$directory" == "$anchor/"* ]] || return 1
    relative="${directory#"$anchor"}"
    relative="${relative#/}"
  fi

  directory_component_is_safe "$anchor" "$expected_owner" || return 1
  current="$anchor"
  IFS='/' read -r -a components <<< "$relative"
  for component in "${components[@]}"; do
    [[ -n "$component" ]] || continue
    [[ "$component" != "." && "$component" != ".." ]] || return 1
    if [[ "$current" == "/" ]]; then
      current="/$component"
    else
      current="$current/$component"
    fi
    directory_component_is_safe "$current" "$expected_owner" || return 1
  done
  [[ "$current" == "$directory" ]]
}

managed_directory_component_is_safe() {
  directory_component_is_safe "$1" "$MANAGED_FILE_EXPECTED_OWNER"
}

managed_directory_is_safe() {
  directory_ancestry_is_safe \
    "$MANAGED_PATH_ANCHOR" \
    "$1" \
    "$MANAGED_FILE_EXPECTED_OWNER"
}

ensure_managed_directory() {
  local directory="$1"
  local component current relative
  local -a components

  [[ "$MANAGED_PATH_ANCHOR" == /* && "$directory" == /* ]] \
    || fail "O diretório gerenciado $directory não é absoluto."
  if [[ "$MANAGED_PATH_ANCHOR" == "/" ]]; then
    relative="${directory#/}"
  else
    [[ "$directory" == "$MANAGED_PATH_ANCHOR" \
      || "$directory" == "$MANAGED_PATH_ANCHOR/"* ]] \
      || fail "O diretório $directory está fora da hierarquia gerenciada $MANAGED_PATH_ANCHOR."
    relative="${directory#"$MANAGED_PATH_ANCHOR"}"
    relative="${relative#/}"
  fi

  managed_directory_component_is_safe "$MANAGED_PATH_ANCHOR" \
    || fail "A âncora gerenciada $MANAGED_PATH_ANCHOR tem proprietário ou permissões inseguras."
  current="$MANAGED_PATH_ANCHOR"
  IFS='/' read -r -a components <<< "$relative"
  for component in "${components[@]}"; do
    [[ -n "$component" ]] || continue
    [[ "$component" != "." && "$component" != ".." ]] \
      || fail "A hierarquia de $directory contém navegação relativa insegura."
    if [[ "$current" == "/" ]]; then
      current="/$component"
    else
      current="$current/$component"
    fi
    if [[ ! -e "$current" && ! -L "$current" ]]; then
      # Every parent was just validated as non-writable by non-root users.
      # A racing EEXIST is accepted only if the resulting directory is safe.
      mkdir -m 0755 -- "$current" \
        || managed_directory_component_is_safe "$current" \
        || fail "Não foi possível criar com segurança o diretório $current."
    fi
    managed_directory_component_is_safe "$current" \
      || fail "O diretório $current tem proprietário ou permissões inseguras e foi preservado."
  done
  if ! managed_directory_is_safe "$directory"; then
    fail "A hierarquia de $directory mudou durante a validação e foi preservada."
  fi
}

action_lock_file_is_safe() {
  local parent metadata

  parent="$(dirname -- "$ACTION_LOCK_PATH")"
  directory_ancestry_is_safe \
    "$ACTION_LOCK_PATH_ANCHOR" \
    "$parent" \
    "$ACTION_LOCK_EXPECTED_OWNER" \
    || return 1
  if path_has_symlink_component "$ACTION_LOCK_PATH" \
    || [[ ! -f "$ACTION_LOCK_PATH" || -L "$ACTION_LOCK_PATH" ]]; then
    return 1
  fi
  metadata="$(stat -Lc '%u:%g:%a:%h' -- "$ACTION_LOCK_PATH")" || return 1
  [[ "$metadata" == "${ACTION_LOCK_EXPECTED_OWNER}:600:1" ]]
}

ensure_action_lock_file() {
  local parent temporary

  parent="$(dirname -- "$ACTION_LOCK_PATH")"
  directory_ancestry_is_safe \
    "$ACTION_LOCK_PATH_ANCHOR" \
    "$parent" \
    "$ACTION_LOCK_EXPECTED_OWNER" \
    || return 1

  if [[ ! -e "$ACTION_LOCK_PATH" && ! -L "$ACTION_LOCK_PATH" ]]; then
    temporary="$(mktemp "$parent/.galaxybook-browser-camera-lock.XXXXXX")" \
      || return 1
    chmod 0600 -- "$temporary" || {
      rm -f -- "$temporary"
      return 1
    }
    if ln -- "$temporary" "$ACTION_LOCK_PATH"; then
      if ! rm -f -- "$temporary"; then
        return 1
      fi
    else
      rm -f -- "$temporary"
      [[ -e "$ACTION_LOCK_PATH" || -L "$ACTION_LOCK_PATH" ]] || return 1
    fi
  fi

  action_lock_file_is_safe
}

acquire_action_lock() {
  local descriptor fd_path fd_snapshot path_snapshot

  ACTION_LOCK_FD=""
  ensure_action_lock_file || return 1
  if ! exec {descriptor}<>"$ACTION_LOCK_PATH"; then
    return 1
  fi
  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  fd_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h' -- "$fd_path")" || {
    exec {descriptor}>&-
    return 1
  }
  path_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h' -- "$ACTION_LOCK_PATH")" || {
    exec {descriptor}>&-
    return 1
  }
  if [[ "$fd_snapshot" != "$path_snapshot" \
    || "${fd_snapshot#*|}" != "${ACTION_LOCK_EXPECTED_OWNER}:600:1" ]] \
    || ! action_lock_file_is_safe; then
    exec {descriptor}>&-
    return 1
  fi
  if ! "$FLOCK_COMMAND" \
    --fcntl \
    --exclusive \
    --timeout "$ACTION_LOCK_TIMEOUT_SECONDS" \
    "$descriptor"; then
    exec {descriptor}>&-
    return 1
  fi

  ACTION_LOCK_FD="$descriptor"
  validate_action_lock || {
    exec {descriptor}>&-
    ACTION_LOCK_FD=""
    return 1
  }
}

validate_action_lock() {
  local descriptor="${ACTION_LOCK_FD:-}"
  local fd_path fd_snapshot path_snapshot

  [[ "$descriptor" =~ ^[0-9]+$ ]] || return 1
  action_lock_file_is_safe || return 1
  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  fd_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h' -- "$fd_path")" || return 1
  path_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h' -- "$ACTION_LOCK_PATH")" \
    || return 1
  [[ "$fd_snapshot" == "$path_snapshot" \
    && "${fd_snapshot#*|}" == "${ACTION_LOCK_EXPECTED_OWNER}:600:1" ]] \
    || return 1

  # Reapplying an OFD lock through the same open-file description is
  # idempotent and proves that this process still owns the action lock.
  "$FLOCK_COMMAND" --fcntl --exclusive --nonblock "$descriptor" || return 1
  fd_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h' -- "$fd_path")" || return 1
  path_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h' -- "$ACTION_LOCK_PATH")" \
    || return 1
  [[ "$fd_snapshot" == "$path_snapshot" ]]
}

require_action_lock() {
  validate_action_lock \
    || fail "O lock exclusivo da configuração da câmera deixou de ser válido. Nenhuma alteração adicional foi feita."
}

close_opened_managed_file() {
  local descriptor="${OPENED_MANAGED_FILE_FD:-}"

  if [[ "$descriptor" =~ ^[0-9]+$ ]]; then
    exec {descriptor}<&-
  fi
  OPENED_MANAGED_FILE_FD=""
  OPENED_MANAGED_FILE_SNAPSHOT=""
}

open_managed_file_if_safe() {
  local target="$1"
  local descriptor fd_path metadata path_snapshot fd_snapshot

  close_opened_managed_file
  if path_has_symlink_component "$target"; then
    return 1
  fi
  [[ -f "$target" && ! -L "$target" ]] || return 1

  if ! exec {descriptor}<"$target"; then
    return 1
  fi
  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  if [[ ! -f "$fd_path" ]]; then
    exec {descriptor}<&-
    return 1
  fi

  metadata="$(stat -Lc '%u:%g:%a:%h' -- "$fd_path")" || {
    exec {descriptor}<&-
    return 1
  }
  if [[ "$metadata" != "${MANAGED_FILE_EXPECTED_OWNER}:644:1" ]]; then
    exec {descriptor}<&-
    return 1
  fi

  fd_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h|%s|%y|%z' -- "$fd_path")" || {
    exec {descriptor}<&-
    return 1
  }
  path_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h|%s|%y|%z' -- "$target")" || {
    exec {descriptor}<&-
    return 1
  }
  if path_has_symlink_component "$target" || [[ "$path_snapshot" != "$fd_snapshot" ]]; then
    exec {descriptor}<&-
    return 1
  fi

  OPENED_MANAGED_FILE_FD="$descriptor"
  OPENED_MANAGED_FILE_SNAPSHOT="$fd_snapshot"
}

opened_managed_file_is_current() {
  local target="$1"
  local descriptor="${OPENED_MANAGED_FILE_FD:-}"
  local fd_path path_snapshot fd_snapshot

  [[ "$descriptor" =~ ^[0-9]+$ ]] || return 1
  if path_has_symlink_component "$target"; then
    return 1
  fi
  [[ -f "$target" && ! -L "$target" ]] || return 1

  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  fd_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h|%s|%y|%z' -- "$fd_path")" \
    || return 1
  path_snapshot="$(stat -Lc '%d:%i|%u:%g:%a:%h|%s|%y|%z' -- "$target")" \
    || return 1
  [[ "$fd_snapshot" == "$OPENED_MANAGED_FILE_SNAPSHOT" \
    && "$path_snapshot" == "$OPENED_MANAGED_FILE_SNAPSHOT" ]]
}

managed_file_matches_exactly() {
  local target="$1"
  local source="$2"
  local descriptor status=1

  open_managed_file_if_safe "$target" || return 1
  descriptor="$OPENED_MANAGED_FILE_FD"
  if cmp -s -- "/proc/${BASHPID}/fd/${descriptor}" "$source" \
    && opened_managed_file_is_current "$target"; then
    status=0
  fi
  close_opened_managed_file
  return "$status"
}

write_managed_file() {
  local target="$1"
  local source="$2"
  local directory temporary

  require_action_lock
  directory="$(dirname -- "$target")"
  ensure_managed_directory "$directory"
  if [[ -e "$target" ]]; then
    managed_file_matches_exactly "$target" "$source" \
      || fail "O arquivo $target não é uma cópia gerenciada regular, exclusiva, root:root 0644 e foi preservado."
    managed_directory_is_safe "$directory" \
      || fail "A hierarquia de $target mudou durante a validação e foi preservada."
    require_action_lock
    return 0
  fi
  if [[ -L "$target" ]]; then
    fail "O caminho gerenciado $target é um link simbólico e foi preservado."
  fi

  temporary="$(mktemp "$directory/.galaxybook-setup.XXXXXX")"
  if ! install -m 0644 -- "$source" "$temporary"; then
    rm -f -- "$temporary"
    fail "Não foi possível preparar o arquivo gerenciado $target."
  fi
  if ! managed_file_matches_exactly "$temporary" "$source"; then
    rm -f -- "$temporary"
    fail "O arquivo temporário de $target não passou na validação de segurança."
  fi
  if ! managed_directory_is_safe "$directory"; then
    rm -f -- "$temporary"
    fail "A hierarquia de $target mudou antes da publicação e foi preservada."
  fi
  require_action_lock
  if ! ln -- "$temporary" "$target"; then
    rm -f -- "$temporary"
    fail "O caminho $target apareceu durante a gravação e foi preservado."
  fi
  if ! rm -f -- "$temporary"; then
    fail "Não foi possível remover o nome temporário de $target. O arquivo publicado foi preservado para evitar remoção por pathname após uma corrida."
  fi
  if ! managed_file_matches_exactly "$target" "$source" \
    || ! managed_directory_is_safe "$directory" \
    || ! validate_action_lock; then
    fail "O arquivo publicado em $target não passou na validação final de segurança e foi preservado para evitar remoção por pathname após uma corrida."
  fi
}

write_managed_file_if_legacy_absent() (
  local legacy="$1"
  local target="$2"
  local source="$3"
  local legacy_directory target_directory

  require_action_lock
  if ! acquire_rpm_read_lock || ! validate_rpm_read_lock; then
    fail "Não foi possível adquirir com segurança o lock de leitura do RPM para reconciliar $legacy e $target."
  fi
  if [[ -e "$legacy" || -L "$legacy" ]]; then
    [[ ! -e "$target" && ! -L "$target" ]] \
      || fail "As configurações $legacy e $target coexistem. Ambas foram preservadas; remova manualmente apenas a configuração que não deve permanecer ativa."
    log "Não criando $target para evitar configuração duplicada enquanto o arquivo legado preservado existir."
    return 0
  fi

  legacy_directory="$(dirname -- "$legacy")"
  target_directory="$(dirname -- "$target")"
  [[ "$legacy_directory" == "$target_directory" ]] \
    || fail "As configurações correlacionadas $legacy e $target não pertencem ao mesmo diretório gerenciado."
  validate_rpm_read_lock \
    || fail "O lock de leitura do RPM mudou antes da publicação de $target."
  write_managed_file "$target" "$source"
  if ! validate_rpm_read_lock \
    || ! validate_action_lock \
    || ! managed_directory_is_safe "$legacy_directory" \
    || ! managed_directory_is_safe "$target_directory"; then
    fail "A hierarquia ou os locks mudaram durante a reconciliação de $legacy e $target. Os arquivos existentes foram preservados."
  fi
  if [[ -e "$legacy" || -L "$legacy" ]]; then
    fail "A configuração $legacy apareceu durante a publicação de $target. Ambas foram preservadas para evitar remoção automática de configuração administrativa."
  fi
)

close_opened_configuration_file() {
  local descriptor="${OPENED_CONFIGURATION_FILE_FD:-}"

  if [[ "$descriptor" =~ ^[0-9]+$ ]]; then
    exec {descriptor}<&-
  fi
  OPENED_CONFIGURATION_FILE_FD=""
  OPENED_CONFIGURATION_FILE_SNAPSHOT=""
  OPENED_CONFIGURATION_FILE_LINKS=""
}

open_configuration_file_if_safe() {
  local target="$1"
  local expected_owner="$2"
  local minimum_size="$3"
  local maximum_size="$4"
  local descriptor fd_path fd_snapshot path_snapshot leaf_metadata
  local owner group mode links size

  close_opened_configuration_file
  [[ "$minimum_size" =~ ^[0-9]+$ \
    && "$maximum_size" =~ ^[0-9]+$ ]] \
    && (( maximum_size >= minimum_size )) || return 1
  if path_has_symlink_component "$target" \
    || [[ ! -f "$target" || -L "$target" ]]; then
    return 1
  fi
  if ! exec {descriptor}<"$target"; then
    return 1
  fi
  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  [[ -f "$fd_path" ]] || {
    exec {descriptor}<&-
    return 1
  }

  fd_snapshot="$(
    stat -Lc '%d:%i|%u:%g:%a:%h:%s|%Y:%Z' -- "$fd_path"
  )" || {
    exec {descriptor}<&-
    return 1
  }
  path_snapshot="$(
    stat -Lc '%d:%i|%u:%g:%a:%h:%s|%Y:%Z' -- "$target"
  )" || {
    exec {descriptor}<&-
    return 1
  }
  leaf_metadata="${fd_snapshot#*|}"
  leaf_metadata="${leaf_metadata%%|*}"
  IFS=: read -r owner group mode links size <<< "$leaf_metadata"
  if [[ "$owner:$group" != "$expected_owner" \
    || ! "$mode" =~ ^[0-7]{3,4}$ \
    || ! "$links" =~ ^[1-9][0-9]*$ \
    || ! "$size" =~ ^[0-9]+$ ]] \
    || (( (8#${mode}) & 8#022 )) \
    || (( size < minimum_size || size > maximum_size )) \
    || path_has_symlink_component "$target" \
    || [[ "$fd_snapshot" != "$path_snapshot" ]]; then
    exec {descriptor}<&-
    return 1
  fi

  OPENED_CONFIGURATION_FILE_FD="$descriptor"
  OPENED_CONFIGURATION_FILE_SNAPSHOT="$fd_snapshot"
  OPENED_CONFIGURATION_FILE_LINKS="$links"
}

opened_configuration_file_is_current() {
  local target="$1"
  local descriptor="${OPENED_CONFIGURATION_FILE_FD:-}"
  local fd_path fd_snapshot path_snapshot

  [[ "$descriptor" =~ ^[0-9]+$ ]] || return 1
  if path_has_symlink_component "$target" \
    || [[ ! -f "$target" || -L "$target" ]]; then
    return 1
  fi
  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  fd_snapshot="$(
    stat -Lc '%d:%i|%u:%g:%a:%h:%s|%Y:%Z' -- "$fd_path"
  )" || return 1
  path_snapshot="$(
    stat -Lc '%d:%i|%u:%g:%a:%h:%s|%Y:%Z' -- "$target"
  )" || return 1
  [[ "$fd_snapshot" == "$OPENED_CONFIGURATION_FILE_SNAPSHOT" \
    && "$path_snapshot" == "$OPENED_CONFIGURATION_FILE_SNAPSHOT" ]]
}

rpm_file_owners() {
  local target="$1"
  local output_variable="$2"
  local error_file output_file ownership_output ownership_status

  validate_rpm_read_lock || return 1
  error_file="$(mktemp /var/tmp/galaxybook-browser-rpm-owner.XXXXXX)" \
    || return 1
  output_file="$(mktemp /var/tmp/galaxybook-browser-rpm-owner.XXXXXX)" || {
    rm -f -- "$error_file"
    return 1
  }
  if rpm -qf --qf '%{NAME}\n' -- "$target" \
    >"$output_file" 2>"$error_file"; then
    ownership_status=0
  else
    ownership_status=$?
  fi
  if (( ownership_status != 0 )) \
    || [[ -s "$error_file" || ! -s "$output_file" ]] \
    || ! tail -c 1 -- "$output_file" | grep -qx '' \
    || ! cmp -s -- "$output_file" \
      <(LC_ALL=C tr -cd '\012\040-\176' < "$output_file") \
    || ! LC_ALL=C awk '
      BEGIN { count = 0; valid = 1 }
      /^[0-9A-Za-z][0-9A-Za-z+._-]*$/ { count += 1; next }
      { valid = 0 }
      END { exit valid && count > 0 ? 0 : 1 }
    ' "$output_file"; then
    rm -f -- "$error_file" "$output_file"
    return 1
  fi
  ownership_output="$(<"$output_file")"
  rm -f -- "$error_file" "$output_file"

  validate_rpm_read_lock || return 1
  printf -v "$output_variable" '%s' "$ownership_output"
}

rpm_file_has_conclusive_owner() {
  local target="$1"
  local ownership_output

  rpm_file_owners "$target" ownership_output \
    && opened_configuration_file_is_current "$target"
}

safe_configuration_directory() {
  local directory="$1"

  if [[ "$directory" == "$MANAGED_PATH_ANCHOR" \
    || "$directory" == "$MANAGED_PATH_ANCHOR/"* ]]; then
    directory_ancestry_is_safe \
      "$MANAGED_PATH_ANCHOR" "$directory" "$MANAGED_FILE_EXPECTED_OWNER"
  else
    directory_ancestry_is_safe / "$directory" 0:0
  fi
}

safe_root_symlink() {
  local path="$1"
  local expected_owner="$2"
  local metadata

  [[ -L "$path" ]] || return 1
  metadata="$(stat -c '%u:%g:%a:%h' -- "$path")" || return 1
  [[ "$metadata" == "$expected_owner:777:1" ]]
}

fedora_bridge_chain_target() {
  local bridge_link="$1"
  local output_variable="$2"
  local runtime_target final_target bridge_owners target_owners

  safe_root_symlink "$bridge_link" "$MANAGED_FILE_EXPECTED_OWNER" \
    || return 1
  [[ "$(readlink -- "$bridge_link")" == "$BRIDGE_RUNTIME_LINK" ]] \
    || return 1
  safe_configuration_directory "$(dirname -- "$bridge_link")" || return 1
  safe_configuration_directory "$(dirname -- "$BRIDGE_RUNTIME_LINK")" || return 1
  safe_root_symlink "$BRIDGE_RUNTIME_LINK" "$MANAGED_FILE_EXPECTED_OWNER" \
    || return 1
  runtime_target="$(readlink -- "$BRIDGE_RUNTIME_LINK")" || return 1
  case "$runtime_target" in
    "$BRIDGE_DEFAULTS_ROOT/ipu6/v4l2-relayd" \
      | "$BRIDGE_DEFAULTS_ROOT/ipu6ep/v4l2-relayd" \
      | "$BRIDGE_DEFAULTS_ROOT/ipu6epmtl/v4l2-relayd")
      ;;
    *) return 1 ;;
  esac
  safe_configuration_directory "$(dirname -- "$runtime_target")" || return 1
  final_target="$(readlink -e -- "$bridge_link")" || return 1
  [[ "$final_target" == "$runtime_target" ]] || return 1

  rpm_file_owners "$bridge_link" bridge_owners \
    && [[ "$bridge_owners" == "$BRIDGE_LINK_PACKAGE" ]] \
    && rpm_file_owners "$final_target" target_owners \
    && [[ "$target_owners" == "$BRIDGE_TARGET_PACKAGE" ]] \
    || return 1
  printf -v "$output_variable" '%s' "$final_target"
}

validate_existing_bridge_configuration() (
  local target="$1"
  local directory validated_target links ownership_output chain_target_after

  require_action_lock
  directory="$(dirname -- "$target")"
  managed_directory_is_safe "$directory" \
    || fail "A hierarquia da configuração existente $target tem proprietário ou permissões inseguras e foi preservada."
  if ! acquire_rpm_read_lock || ! validate_rpm_read_lock; then
    fail "Não foi possível adquirir com segurança o lock de leitura do RPM para validar $target."
  fi
  if [[ -L "$target" ]]; then
    fedora_bridge_chain_target "$target" validated_target \
      || fail "A cadeia de links da configuração $target não corresponde ao layout Fedora package-owned e root-safe; ela foi preservada."
  else
    [[ -e "$target" ]] \
      || fail "A configuração $target desapareceu durante a validação."
    validated_target="$target"
  fi
  open_configuration_file_if_safe \
    "$validated_target" "$MANAGED_FILE_EXPECTED_OWNER" 1 "$MAX_CONFIGURATION_FILE_SIZE" \
    || fail "A configuração existente $target não é um arquivo regular root-owned, sem escrita por grupo/outros, estável e foi preservada."
  links="$OPENED_CONFIGURATION_FILE_LINKS"
  if (( links > 1 )); then
    rpm_file_owners "$validated_target" ownership_output \
      || fail "A configuração hardlinked $target não teve dono RPM conclusivamente validado; ela foi preservada."
  fi
  if [[ -L "$target" ]]; then
    fedora_bridge_chain_target "$target" chain_target_after \
      && [[ "$chain_target_after" == "$validated_target" ]] \
      || fail "A cadeia de links da configuração $target mudou durante a validação e foi preservada."
  fi
  if ! opened_configuration_file_is_current "$validated_target" \
    || ! managed_directory_is_safe "$directory" \
    || ! validate_rpm_read_lock \
    || ! validate_action_lock; then
    fail "A configuração $target, sua hierarquia ou os locks mudaram durante a validação e foram preservados."
  fi
  close_opened_configuration_file
)

prepare_bridge_configuration() {
  local target="$1"
  local expected_content="$2"

  require_action_lock
  if [[ -e "$target" || -L "$target" ]]; then
    validate_existing_bridge_configuration "$target" \
      || fail "A configuração existente do v4l2-relayd não passou no preflight de segurança e foi preservada."
    log "Preservando a configuração existente e validada do v4l2-relayd: $target"
  else
    write_managed_file "$target" "$expected_content"
  fi
}

v4l2loopback_option_exists_outside_managed() {
  local managed="$1"
  local directory candidate expected_owner descriptor grep_status links
  local option_exists=0
  shift

  close_opened_configuration_file
  validate_rpm_read_lock || return 2
  for directory in "$@"; do
    modprobe_scan_directory_is_safe "$directory" || return 2
    [[ -d "$directory" ]] || continue
    if [[ "$directory" == "$MANAGED_PATH_ANCHOR" \
      || "$directory" == "$MANAGED_PATH_ANCHOR/"* ]]; then
      expected_owner="$MANAGED_FILE_EXPECTED_OWNER"
    else
      expected_owner="0:0"
    fi
    for candidate in "$directory"/*.conf; do
      [[ "$candidate" == "$managed" ]] && continue
      [[ -e "$candidate" || -L "$candidate" ]] || continue
      open_configuration_file_if_safe \
        "$candidate" "$expected_owner" 0 "$MAX_CONFIGURATION_FILE_SIZE" \
        || return 2
      descriptor="$OPENED_CONFIGURATION_FILE_FD"
      links="$OPENED_CONFIGURATION_FILE_LINKS"
      if (( links > 1 )) && ! rpm_file_has_conclusive_owner "$candidate"; then
        close_opened_configuration_file
        return 2
      fi
      if grep -qsE \
        '^[[:space:]]*options[[:space:]]+v4l2loopback([[:space:]]|$)' \
        "/proc/${BASHPID}/fd/${descriptor}"; then
        grep_status=0
      else
        grep_status=$?
      fi
      if ! opened_configuration_file_is_current "$candidate" \
        || ! modprobe_scan_directory_is_safe "$directory" \
        || ! validate_rpm_read_lock; then
        close_opened_configuration_file
        return 2
      fi
      close_opened_configuration_file
      if (( grep_status == 0 )); then
        option_exists=1
        continue
      fi
      (( grep_status == 1 )) || return 2
    done
  done
  validate_rpm_read_lock || return 2
  (( option_exists == 1 ))
}

modprobe_scan_directory_is_safe() {
  local directory="$1"
  local anchor expected_owner parent

  if [[ "$directory" == "$MANAGED_PATH_ANCHOR" \
    || "$directory" == "$MANAGED_PATH_ANCHOR/"* ]]; then
    anchor="$MANAGED_PATH_ANCHOR"
    expected_owner="$MANAGED_FILE_EXPECTED_OWNER"
  else
    anchor="/"
    expected_owner="0:0"
  fi

  if path_has_symlink_component "$directory"; then
    return 1
  fi
  if [[ -e "$directory" || -L "$directory" ]]; then
    directory_ancestry_is_safe "$anchor" "$directory" "$expected_owner"
    return
  fi

  # An absent optional vendor directory is safe to skip only while its nearest
  # existing ancestor is trusted. This prevents an unprivileged writer from
  # creating the directory between the negative scan and publication in /etc.
  parent="$(dirname -- "$directory")"
  while [[ "$parent" != "$anchor" \
    && ! -e "$parent" && ! -L "$parent" ]]; do
    parent="$(dirname -- "$parent")"
  done
  directory_ancestry_is_safe "$anchor" "$parent" "$expected_owner"
}

validate_modprobe_scan_directories() {
  local directory

  for directory in "$@"; do
    modprobe_scan_directory_is_safe "$directory" || return 1
  done
}

reconcile_v4l2loopback_option() (
  local managed="$1"
  local source="$2"
  local external_option_exists=0 scan_status
  local -a scan_directories
  shift 2
  scan_directories=("$@")

  (( ${#scan_directories[@]} > 0 )) \
    || fail "Nenhum diretório de configuração do v4l2loopback foi informado."
  require_action_lock
  if ! acquire_rpm_read_lock || ! validate_rpm_read_lock; then
    fail "Não foi possível adquirir com segurança o lock de leitura do RPM para reconciliar as opções do v4l2loopback."
  fi
  validate_modprobe_scan_directories "${scan_directories[@]}" \
    || fail "Um diretório de configuração do v4l2loopback tem hierarquia insegura e foi preservado."

  if v4l2loopback_option_exists_outside_managed \
    "$managed" "${scan_directories[@]}"; then
    external_option_exists=1
  else
    scan_status=$?
    (( scan_status == 1 )) \
      || fail "A leitura das configurações v4l2loopback existentes foi inconclusiva. Nenhum arquivo adicional foi publicado."
  fi
  if ! validate_rpm_read_lock \
    || ! validate_action_lock \
    || ! validate_modprobe_scan_directories "${scan_directories[@]}"; then
    fail "A hierarquia ou os locks mudaram durante a leitura das opções do v4l2loopback. Nenhum arquivo adicional foi publicado."
  fi

  if (( external_option_exists == 1 )); then
    diagnose_existing_file "$managed" "$source"
    if ! validate_rpm_read_lock \
      || ! validate_action_lock \
      || ! validate_modprobe_scan_directories "${scan_directories[@]}"; then
      fail "A hierarquia ou os locks mudaram durante a validação da configuração v4l2loopback existente. Todos os arquivos foram preservados."
    fi
    [[ ! -e "$managed" && ! -L "$managed" ]] \
      || fail "Outra configuração do v4l2loopback e $managed coexistem. Todos os arquivos foram preservados; resolva manualmente a duplicação antes de continuar."
    if v4l2loopback_option_exists_outside_managed \
      "$managed" "${scan_directories[@]}"; then
      :
    else
      scan_status=$?
      (( scan_status == 1 )) \
        && fail "A configuração v4l2loopback existente mudou durante a validação. Nenhum arquivo adicional foi publicado; execute a ação novamente."
      fail "A revalidação da configuração v4l2loopback existente foi inconclusiva. Nenhum arquivo adicional foi publicado."
    fi
    if ! validate_rpm_read_lock \
      || ! validate_action_lock \
      || ! validate_modprobe_scan_directories "${scan_directories[@]}"; then
      fail "A hierarquia ou os locks mudaram após a validação da configuração v4l2loopback existente. Todos os arquivos foram preservados."
    fi
    log "Usando a configuração de v4l2loopback já existente sem criar uma segunda regra."
    return 0
  fi

  write_managed_file "$managed" "$source"
  if ! validate_rpm_read_lock \
    || ! validate_action_lock \
    || ! validate_modprobe_scan_directories "${scan_directories[@]}"; then
    fail "A hierarquia ou os locks mudaram durante a publicação de $managed. Os arquivos existentes foram preservados."
  fi
  if v4l2loopback_option_exists_outside_managed \
    "$managed" "${scan_directories[@]}"; then
    external_option_exists=1
  else
    scan_status=$?
    (( scan_status == 1 )) \
      || fail "A validação final das configurações v4l2loopback foi inconclusiva. Os arquivos existentes foram preservados."
  fi
  if ! validate_rpm_read_lock \
    || ! validate_action_lock \
    || ! validate_modprobe_scan_directories "${scan_directories[@]}" \
    || ! managed_file_matches_exactly "$managed" "$source"; then
    fail "A hierarquia ou os locks mudaram durante a validação final de $managed. Os arquivos existentes foram preservados."
  fi
  (( external_option_exists == 0 )) \
    || fail "Outra configuração do v4l2loopback apareceu durante a publicação de $managed. Todos os arquivos foram preservados; resolva manualmente a duplicação antes de continuar."
)

acquire_rpm_read_lock() {
  local parent descriptor fd_path metadata path_identity fd_identity
  local lock_owner lock_mode_and_links lock_mode lock_links

  RPM_READ_LOCK_FD=""
  parent="$(dirname -- "$RPM_LOCK_PATH")"
  if path_has_symlink_component "$RPM_LOCK_PATH" \
    || [[ ! -d "$parent" || ! -f "$RPM_LOCK_PATH" || -L "$RPM_LOCK_PATH" ]]; then
    return 1
  fi
  directory_ancestry_is_safe \
    "$RPM_LOCK_PATH_ANCHOR" \
    "$parent" \
    "$RPM_LOCK_EXPECTED_OWNER" \
    || return 1

  metadata="$(stat -Lc '%u:%g:%a:%h' -- "$RPM_LOCK_PATH")" || return 1
  lock_owner="${metadata%:*:*}"
  lock_mode_and_links="${metadata#*:*:}"
  lock_mode="${lock_mode_and_links%%:*}"
  lock_links="${lock_mode_and_links##*:}"
  if [[ "$lock_owner" != "$RPM_LOCK_EXPECTED_OWNER" \
    || ! "$lock_mode" =~ ^[0-7]{3,4}$ \
    || "$lock_links" != "1" ]]; then
    return 1
  fi
  if (( (8#${lock_mode}) & 8#022 )); then
    return 1
  fi

  path_identity="$(stat -Lc '%d:%i' -- "$RPM_LOCK_PATH")" || return 1
  if ! exec {descriptor}<"$RPM_LOCK_PATH"; then
    return 1
  fi
  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  metadata="$(stat -Lc '%u:%g:%a:%h' -- "$fd_path")" || {
    exec {descriptor}<&-
    return 1
  }
  if [[ "$metadata" != "${RPM_LOCK_EXPECTED_OWNER}:${lock_mode}:1" ]]; then
    exec {descriptor}<&-
    return 1
  fi
  fd_identity="$(stat -Lc '%d:%i' -- "$fd_path")" || {
    exec {descriptor}<&-
    return 1
  }
  if [[ "$fd_identity" != "$path_identity" ]]; then
    exec {descriptor}<&-
    return 1
  fi
  if ! "$FLOCK_COMMAND" --fcntl --shared --timeout "$RPM_LOCK_TIMEOUT_SECONDS" "$descriptor"; then
    exec {descriptor}<&-
    return 1
  fi
  fd_identity="$(stat -Lc '%d:%i' -- "$fd_path")" || {
    exec {descriptor}<&-
    return 1
  }
  path_identity="$(stat -Lc '%d:%i' -- "$RPM_LOCK_PATH")" || {
    exec {descriptor}<&-
    return 1
  }
  if path_has_symlink_component "$RPM_LOCK_PATH" || [[ "$fd_identity" != "$path_identity" ]]; then
    exec {descriptor}<&-
    return 1
  fi

  RPM_READ_LOCK_FD="$descriptor"
}

validate_rpm_read_lock() {
  local descriptor="${RPM_READ_LOCK_FD:-}"
  local parent fd_path fd_identity path_identity metadata fd_metadata
  local lock_owner lock_mode_and_links lock_mode lock_links

  [[ "$descriptor" =~ ^[0-9]+$ ]] || return 1
  parent="$(dirname -- "$RPM_LOCK_PATH")"
  if path_has_symlink_component "$RPM_LOCK_PATH" \
    || [[ ! -f "$RPM_LOCK_PATH" || -L "$RPM_LOCK_PATH" ]]; then
    return 1
  fi
  directory_ancestry_is_safe \
    "$RPM_LOCK_PATH_ANCHOR" \
    "$parent" \
    "$RPM_LOCK_EXPECTED_OWNER" \
    || return 1

  metadata="$(stat -Lc '%u:%g:%a:%h' -- "$RPM_LOCK_PATH")" || return 1
  lock_owner="${metadata%:*:*}"
  lock_mode_and_links="${metadata#*:*:}"
  lock_mode="${lock_mode_and_links%%:*}"
  lock_links="${lock_mode_and_links##*:}"
  if [[ "$lock_owner" != "$RPM_LOCK_EXPECTED_OWNER" \
    || ! "$lock_mode" =~ ^[0-7]{3,4}$ \
    || "$lock_links" != "1" ]]; then
    return 1
  fi
  if (( (8#${lock_mode}) & 8#022 )); then
    return 1
  fi

  fd_path="/proc/${BASHPID}/fd/${descriptor}"
  fd_metadata="$(stat -Lc '%u:%g:%a:%h' -- "$fd_path")" || return 1
  [[ "$fd_metadata" == "$metadata" ]] || return 1
  fd_identity="$(stat -Lc '%d:%i' -- "$fd_path")" || return 1
  path_identity="$(stat -Lc '%d:%i' -- "$RPM_LOCK_PATH")" || return 1
  [[ "$fd_identity" == "$path_identity" ]] || return 1

  # Reapplying this lock is idempotent on the same open-file description and
  # proves that the inherited descriptor still carries the RPM6-compatible
  # shared OFD lock.
  "$FLOCK_COMMAND" --fcntl --shared --nonblock "$descriptor" || return 1
  fd_identity="$(stat -Lc '%d:%i' -- "$fd_path")" || return 1
  path_identity="$(stat -Lc '%d:%i' -- "$RPM_LOCK_PATH")" || return 1
  [[ "$fd_identity" == "$path_identity" ]]
}

diagnose_existing_file() (
  local target="$1"
  local known_content="$2"
  local directory ownership_output descriptor ownership_status

  [[ -e "$target" || -L "$target" ]] || return 0
  require_action_lock
  if path_has_symlink_component "$target"; then
    log "Preservando $target porque sua hierarquia contém um link simbólico."
    return 0
  fi
  directory="$(dirname -- "$target")"
  if ! managed_directory_is_safe "$directory"; then
    log "Preservando $target porque seu diretório tem proprietário ou permissões inseguras."
    return 0
  fi
  if ! acquire_rpm_read_lock; then
    log "Preservando $target porque não foi possível adquirir com segurança o lock de leitura do RPM."
    return 0
  fi
  if ! open_managed_file_if_safe "$target"; then
    log "Preservando arquivo com tipo, proprietário, modo ou número de links inesperado: $target"
    return 0
  fi
  descriptor="$OPENED_MANAGED_FILE_FD"

  if ! validate_rpm_read_lock; then
    log "Preservando $target porque o lock de leitura do RPM deixou de ser válido."
    return 0
  fi

  if ownership_output="$(rpm -qf "$target" 2>&1)"; then
    ownership_status=0
  else
    ownership_status=$?
  fi
  if ! validate_rpm_read_lock \
    || ! validate_action_lock \
    || ! managed_directory_is_safe "$directory" \
    || ! opened_managed_file_is_current "$target"; then
    log "Preservando $target porque ele mudou durante a validação."
    return 0
  fi

  if (( ownership_status == 0 )); then
    log "Preservando arquivo gerenciado por RPM: $target"
    return 0
  fi
  if [[ "$ownership_output" != "file $target is not owned by any package" ]]; then
    log "Preservando $target porque o RPM não confirmou que o arquivo está sem dono."
    return 0
  fi
  if cmp -s -- "/proc/${BASHPID}/fd/${descriptor}" "$known_content" \
    && opened_managed_file_is_current "$target"; then
    log "Preservando configuração legada reconhecida e sem dono de pacote: $target"
  else
    log "Preservando configuração administrativa desconhecida: $target"
  fi
)

refuse_legacy_udev_configuration() {
  local legacy="$1"
  local known_content="$2"

  [[ -e "$legacy" || -L "$legacy" ]] || return 0
  diagnose_existing_file "$legacy" "$known_content"
  fail "A regra udev legada $legacy ainda existe e foi preservada. Remova-a manualmente após confirmar que não contém personalizações locais e execute novamente a configuração da câmera."
}

find_virtual_camera() {
  local name_file

  for name_file in "$SYS_ROOT"/devices/virtual/video4linux/*/name; do
    [[ -f "$name_file" ]] || continue
    if [[ "$(tr -d '\n' < "$name_file")" == "Intel MIPI Camera" ]]; then
      basename -- "$(dirname -- "$name_file")"
      return 0
    fi
  done
  return 1
}

# A loopback node can survive a failed probe of the real sensor. Starting
# icamerasrc cannot bind that sensor. Other camera models keep their usual path.
require_bound_ov_sensor_if_present() {
  local devices="$1" sensor found=0
  for sensor in "$devices"/i2c-OVTI02C1:*; do
    [[ -d "$sensor" ]] || continue
    found=1
    [[ ! -d "$sensor/driver" ]] || return 0
  done
  (( found == 0 )) || fail "O sensor OV02C10 foi detectado, mas está sem driver neste boot. Consulte o diagnóstico da câmera no Setup e reaplique esta configuração quando o sensor estiver vinculado."
}

virtual_camera_has_capture_capability() {
  local device="$1"
  local error_file output_file status

  [[ -n "$WORKDIR" && -d "$WORKDIR" ]] || return 1
  error_file="$WORKDIR/v4l2-ctl.stderr"
  output_file="$WORKDIR/v4l2-ctl.stdout"
  : > "$error_file" || return 1
  : > "$output_file" || return 1
  if v4l2-ctl -D -d "$device" >"$output_file" 2>"$error_file"; then
    status=0
  else
    status=$?
  fi
  (( status == 0 )) \
    && [[ ! -s "$error_file" ]] \
    && [[ -s "$output_file" ]] \
    && tail -c 1 -- "$output_file" | grep -qx '' \
    && cmp -s -- "$output_file" \
      <(LC_ALL=C tr -cd '\011\012\040-\176' < "$output_file") \
    && grep -Eq '^[[:blank:]]*Video Capture[[:blank:]]*$' "$output_file"
}

wait_for_virtual_capture() {
  local attempt device
  for attempt in {1..10}; do
    if device="$(find_virtual_camera)" && [[ -e "/dev/$device" ]] \
      && virtual_camera_has_capture_capability "/dev/$device"; then
      printf '%s\n' "/dev/$device"
      return 0
    fi
    if (( attempt < 10 )); then sleep 1; fi
  done
  return 1
}

activate_browser_relay() {
  local instance=v4l2-relayd@icamerasrc.service was_active=0 device
  if systemctl is-active --quiet "$instance"; then was_active=1; fi
  systemctl enable --now v4l2-relayd.service "$instance" || return 1
  if ! device="$(wait_for_virtual_capture)"; then
    (( was_active )) || fail "O relay iniciou, mas a webcam virtual não anunciou capacidade de captura."
    # Recover a previously running but stuck pipeline once. Starting a new
    # instance or reapplying a healthy one does not require a second launch.
    systemctl restart "$instance" || return 1
    device="$(wait_for_virtual_capture)" || fail "A webcam virtual continua sem capacidade de captura após reiniciar o relay."
  fi
  systemctl is-active --quiet "$instance" || fail "O relay de câmera não permaneceu ativo."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos deixou de ser válido durante a configuração da câmera."
  log "Bridge V4L2 validado em $device."
}

main() {
  local bridge_config legacy_modprobe managed_modprobe
  local legacy_udev legacy_wireplumber managed_wireplumber

  caller_is_root || fail "Este helper precisa ser executado como root."
  [[ -r "$lifecycle_helper" ]] \
    || fail "O helper do lock global de módulos está ausente: $lifecycle_helper."
  # shellcheck source=kmod-lifecycle-lock.sh
  source "$lifecycle_helper"
  acquire_kmod_lifecycle_lock \
    || fail "Não foi possível adquirir o lock global de manutenção de módulos."
  validate_kmod_lifecycle_lock \
    || fail "O lock global de manutenção de módulos não pôde ser revalidado."
  require_command awk
  require_command basename
  require_command cat
  require_command chmod
  require_command cmp
  require_command dirname
  require_command "$FLOCK_COMMAND"
  require_command grep
  require_command install
  require_command ln
  require_command mktemp
  require_command mkdir
  require_command readlink
  require_command rm
  require_command rpm
  require_command sleep
  require_command stat
  require_command systemctl
  require_command tail
  require_command tr
  require_command udevadm
  require_command v4l2-ctl
  acquire_action_lock \
    || fail "Não foi possível adquirir com segurança o lock exclusivo da configuração da câmera."
  require_bound_ov_sensor_if_present "$SYS_ROOT/bus/i2c/devices"
  WORKDIR="$(mktemp -d /var/tmp/galaxybook-browser-camera.XXXXXX)"
  trap cleanup EXIT

  bridge_config="$ETC_ROOT/v4l2-relayd.d/icamerasrc.conf"
  legacy_modprobe="$ETC_ROOT/modprobe.d/galaxybook-v4l2loopback.conf"
  managed_modprobe="$ETC_ROOT/modprobe.d/99-galaxybook-v4l2loopback.conf"
  legacy_udev="$ETC_ROOT/udev/rules.d/90-hide-ipu6-v4l2.rules"
  legacy_wireplumber="$ETC_ROOT/wireplumber/wireplumber.conf.d/50-disable-ipu6-v4l2.conf"
  managed_wireplumber="$ETC_ROOT/wireplumber/wireplumber.conf.d/50-galaxybook-disable-ipu6-v4l2.conf"

  # This old rule hides the physical IPU6 nodes and can defeat the current
  # browser-camera pipeline. Refuse it before creating configuration files or
  # touching udev/systemd; administrative files are never removed implicitly.
  printf '%s\n' \
    'SUBSYSTEM=="video4linux", KERNEL=="video*", ATTR{name}=="Intel IPU6 ISYS Capture*", TAG-="uaccess"' \
    'SUBSYSTEM=="video4linux", KERNEL=="video*", ATTR{name}=="Intel IPU6 CSI2*", TAG-="uaccess"' \
    > "$WORKDIR/legacy-udev.rules"
  refuse_legacy_udev_configuration "$legacy_udev" "$WORKDIR/legacy-udev.rules"

  printf '%s\n' \
    'VIDEOSRC="icamerasrc"' \
    'FORMAT=NV12' \
    'WIDTH=1280' \
    'HEIGHT=720' \
    'FRAMERATE=30/1' \
    'CARD_LABEL="Intel MIPI Camera"' \
    > "$WORKDIR/icamerasrc.conf"
  prepare_bridge_configuration "$bridge_config" "$WORKDIR/icamerasrc.conf"

  printf '%s\n' \
    'options v4l2loopback exclusive_caps=1 card_label="Intel MIPI Camera"' \
    > "$WORKDIR/v4l2loopback.conf"
  diagnose_existing_file "$legacy_modprobe" "$WORKDIR/v4l2loopback.conf"
  # Fedora's usrmerge exposes /lib/modprobe.d through a symlink. Scan its
  # canonical /usr path once so the trusted hierarchy remains symlink-free.
  reconcile_v4l2loopback_option \
    "$managed_modprobe" \
    "$WORKDIR/v4l2loopback.conf" \
    "$ETC_ROOT/modprobe.d" \
    /run/modprobe.d \
    /usr/local/lib/modprobe.d \
    /usr/lib/modprobe.d

  cat > "$WORKDIR/wireplumber.conf" <<'EOF'
monitor.v4l2.rules = [
  {
    matches = [
      { node.name = "~v4l2_input.pci-0000_00_05*" }
    ]
    actions = {
      update-props = {
        node.disabled = true
      }
    }
  }
]
EOF
  diagnose_existing_file "$legacy_wireplumber" "$WORKDIR/wireplumber.conf"
  write_managed_file_if_legacy_absent \
    "$legacy_wireplumber" \
    "$managed_wireplumber" \
    "$WORKDIR/wireplumber.conf"

  udevadm control --reload-rules
  udevadm trigger --action=change --subsystem-match=video4linux
  udevadm settle

  activate_browser_relay
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
