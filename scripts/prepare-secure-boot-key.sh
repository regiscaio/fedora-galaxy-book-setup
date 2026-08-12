#!/usr/bin/bash

set -euo pipefail
export LC_ALL=C
export PATH=/usr/sbin:/usr/bin

readonly PUBLIC_KEY="/etc/pki/akmods/certs/public_key.der"
readonly PRIVATE_KEY="/etc/pki/akmods/private/private_key.priv"
readonly CERTIFICATE_DIRECTORY="${PUBLIC_KEY%/*}"
readonly PRIVATE_KEY_DIRECTORY="${PRIVATE_KEY%/*}"
readonly STATE_DIRECTORY="/var/lib/galaxybook-setup"
readonly KEY_IDENTITY_MARKER="$STATE_DIRECTORY/akmods-key.cn"
readonly PUBLIC_CERTIFICATE_STATE="$STATE_DIRECTORY/akmods-public-key.der"
HASH_FILE=""
PENDING_TEMPORARY=""
PUBLIC_CERTIFICATE_TEMPORARY=""
PUBLIC_CERTIFICATE_FD=""
PUBLIC_CERTIFICATE_FD_PATH=""
PUBLIC_CERTIFICATE_SOURCE_METADATA=""
STATE_LOCK_FD=""
KEY_CHECK_DIRECTORY=""
PROBE_STDOUT=""

fail() {
  printf 'galaxybook-setup: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 \
    || fail "O comando obrigatório '$1' não está disponível."
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
  local error_file invalid_byte_file output_file output raw_size status
  local marker=$'\036'

  output_file="$(mktemp /var/tmp/galaxybook-setup-mok-probe-output.XXXXXX)" \
    || return 1
  error_file="$(mktemp /var/tmp/galaxybook-setup-mok-probe-error.XXXXXX)" || {
    rm -f -- "$output_file"
    return 1
  }
  invalid_byte_file="$(
    mktemp /var/tmp/galaxybook-setup-mok-probe-invalid.XXXXXX
  )" || {
    rm -f -- "$output_file" "$error_file"
    return 1
  }
  if "$@" >"$output_file" 2>"$error_file"; then
    status=0
  else
    status=$?
  fi
  if ! tr -d '\011\012\040-\176' \
      < "$output_file" > "$invalid_byte_file" \
    || (( status != 0 )) || [[ -s "$error_file" ]] \
    || [[ -s "$invalid_byte_file" ]]; then
    rm -f -- "$output_file" "$error_file" "$invalid_byte_file"
    PROBE_STDOUT=""
    return 1
  fi
  if ! output="$(
    cat -- "$output_file" || exit $?
    printf '%s' "$marker"
  )"; then
    rm -f -- "$output_file" "$error_file" "$invalid_byte_file"
    PROBE_STDOUT=""
    return 1
  fi
  raw_size="$(wc -c < "$output_file")" || {
    rm -f -- "$output_file" "$error_file" "$invalid_byte_file"
    PROBE_STDOUT=""
    return 1
  }
  rm -f -- "$output_file" "$error_file" "$invalid_byte_file"

  [[ "$output" == *"$marker" && "$raw_size" =~ ^[0-9]+$ ]] || {
    PROBE_STDOUT=""
    return 1
  }
  PROBE_STDOUT="${output%"$marker"}"
  (( ${#PROBE_STDOUT} == raw_size )) || {
    PROBE_STDOUT=""
    return 1
  }
}

generate_mok_password_hash_file() {
  local target="$1"
  local generated_hash generated_output
  local output_prefix=$'input password: \ninput password again: \n'

  if ! capture_clean_output_preserving_newlines mokutil --generate-hash; then
    PROBE_STDOUT=""
    return 1
  fi
  generated_output="$PROBE_STDOUT"
  PROBE_STDOUT=""
  if [[ "$generated_output" != "$output_prefix"*$'\n' ]]; then
    generated_output=""
    return 1
  fi
  generated_hash="${generated_output#"$output_prefix"}"
  generated_output=""
  generated_hash="${generated_hash%$'\n'}"
  if [[ "$generated_hash" == *$'\n'* ]] \
    || [[ ! "$generated_hash" =~ ^\$[0-9A-Za-z.-]+\$[./0-9A-Za-z]+\$[./0-9A-Za-z]+$ ]]; then
    generated_hash=""
    return 1
  fi
  if ! printf '%s\n' "$generated_hash" > "$target"; then
    generated_hash=""
    return 1
  fi
  generated_hash=""
}

cleanup() {
  if [[ -n "$HASH_FILE" ]]; then
    rm -f -- "$HASH_FILE"
  fi
  if [[ -n "$PENDING_TEMPORARY" ]]; then
    rm -f -- "$PENDING_TEMPORARY"
  fi
  if [[ -n "$PUBLIC_CERTIFICATE_TEMPORARY" ]]; then
    rm -f -- "$PUBLIC_CERTIFICATE_TEMPORARY"
  fi
  if [[ -n "$PUBLIC_CERTIFICATE_FD" ]]; then
    exec {PUBLIC_CERTIFICATE_FD}<&-
    PUBLIC_CERTIFICATE_FD=""
    PUBLIC_CERTIFICATE_FD_PATH=""
    PUBLIC_CERTIFICATE_SOURCE_METADATA=""
  fi
  if [[ -n "$STATE_LOCK_FD" ]]; then
    flock --unlock "$STATE_LOCK_FD" >/dev/null 2>&1 || true
    exec {STATE_LOCK_FD}<&-
    STATE_LOCK_FD=""
  fi
  if [[ -n "$KEY_CHECK_DIRECTORY" && -d "$KEY_CHECK_DIRECTORY" ]]; then
    rm -rf -- "$KEY_CHECK_DIRECTORY"
  fi
}

mok_output_contains_fingerprint() {
  local expected_fingerprint="$1"

  normalized_sha1_identifier_is_valid "$expected_fingerprint" || return 1
  awk -v expected="$expected_fingerprint" '
    index($0, "SHA1 Fingerprint:") {
      fingerprint = $0
      sub(/^.*SHA1 Fingerprint:[[:space:]]*/, "", fingerprint)
      sub(/[[:space:]]*$/, "", fingerprint)
      if (toupper(fingerprint) == expected) {
        found = 1
      }
    }
    END { exit found ? 0 : 1 }
  '
}

mok_key_is_enrolled() {
  local certificate="$1"
  local expected_fingerprint="$2"
  local enrolled_output

  normalized_sha1_identifier_is_valid "$expected_fingerprint" || return 1
  if capture_clean_output mokutil --test-key "$certificate"; then
    return 0
  fi
  if ! capture_clean_output mokutil --list-enrolled; then
    return 1
  fi
  enrolled_output="$PROBE_STDOUT"
  mok_output_contains_fingerprint "$expected_fingerprint" <<< "$enrolled_output"
}

mok_request_is_pending() {
  local expected_fingerprint="$1"
  local pending_output

  normalized_sha1_identifier_is_valid "$expected_fingerprint" || return 1
  capture_clean_output mokutil --list-new || return 1
  pending_output="$PROBE_STDOUT"
  mok_output_contains_fingerprint "$expected_fingerprint" <<< "$pending_output"
}

normalize_sha1_identifier() {
  local raw_value="$1"
  local output_variable="$2"
  local compact_value constructed_value=""
  local offset

  compact_value="${raw_value//:/}"
  compact_value="${compact_value//[[:space:]]/}"
  compact_value="${compact_value^^}"
  [[ "$compact_value" =~ ^[0-9A-F]{40}$ ]] || return 1
  for (( offset = 0; offset < 40; offset += 2 )); do
    if [[ -n "$constructed_value" ]]; then
      constructed_value+=:
    fi
    constructed_value+="${compact_value:offset:2}"
  done
  printf -v "$output_variable" '%s' "$constructed_value"
}

normalized_sha1_identifier_is_valid() {
  local value="$1"
  local normalized_value

  normalize_sha1_identifier "$value" normalized_value \
    && [[ "$value" == "$normalized_value" ]]
}

normalize_certificate_serial() {
  local raw_value="$1"
  local output_variable="$2"
  local constructed_value

  constructed_value="${raw_value//:/}"
  constructed_value="${constructed_value//[[:space:]]/}"
  constructed_value="${constructed_value^^}"
  [[ "$constructed_value" =~ ^[0-9A-F]+$ ]] || return 1
  (( ${#constructed_value} % 2 == 0 )) || return 1
  while (( ${#constructed_value} > 2 )) \
    && [[ "$constructed_value" == 00* ]]; do
    constructed_value="${constructed_value:2}"
  done
  (( ${#constructed_value} >= 2 && ${#constructed_value} <= 40 )) \
    || return 1
  [[ "$constructed_value" =~ [1-9A-F] ]] || return 1
  printf -v "$output_variable" '%s' "$constructed_value"
}

normalized_certificate_serial_is_valid() {
  local value="$1"
  local normalized_value

  normalize_certificate_serial "$value" normalized_value \
    && [[ "$value" == "$normalized_value" ]]
}

marker_cn_is_valid() {
  local key_cn="$1"

  [[ -n "$key_cn" && ${#key_cn} -le 255 ]] \
    && [[ "$key_cn" != *$'\n'* && "$key_cn" != *$'\r'* ]] \
    && [[ ! "$key_cn" =~ [[:cntrl:]] ]]
}

certificate_identity() {
  local certificate="${1:-$PUBLIC_KEY}"
  local cn_output_variable="$2"
  local fingerprint_output_variable="$3"
  local serial_output_variable="$4"
  local certificate_subject fingerprint_line serial_line
  local parsed_cn fingerprint_value serial_value

  capture_clean_output \
    openssl x509 -inform DER -in "$certificate" -noout \
      -subject -nameopt RFC2253 \
    || return 1
  certificate_subject="$PROBE_STDOUT"
  certificate_subject="${certificate_subject#subject=}"
  if [[ "$certificate_subject" =~ (^|,)CN=([0-9A-Za-z._-]+)(,|$) ]]; then
    parsed_cn="${BASH_REMATCH[2]}"
  else
    return 1
  fi
  marker_cn_is_valid "$parsed_cn" || return 1

  capture_clean_output \
    openssl x509 -inform DER -in "$certificate" -noout \
      -fingerprint -sha1 \
    || return 1
  fingerprint_line="$PROBE_STDOUT"
  [[ "$fingerprint_line" == *=* ]] || return 1
  normalize_sha1_identifier "${fingerprint_line#*=}" fingerprint_value \
    || return 1

  capture_clean_output \
    openssl x509 -inform DER -in "$certificate" -noout \
      -serial \
    || return 1
  serial_line="$PROBE_STDOUT"
  [[ "$serial_line" == serial=* ]] || return 1
  normalize_certificate_serial "${serial_line#serial=}" serial_value \
    || return 1

  printf -v "$cn_output_variable" '%s' "$parsed_cn"
  printf -v "$fingerprint_output_variable" '%s' "$fingerprint_value"
  printf -v "$serial_output_variable" '%s' "$serial_value"
}

regular_key_file_is_safe() {
  local key_path="$1"
  local expected_owner="${2:-root}"
  local expected_group="${3:-akmods}"
  local metadata

  [[ ! -L "$key_path" && -f "$key_path" && -r "$key_path" ]] || return 1
  metadata="$(stat -c '%U:%G:%a:%h' -- "$key_path")" || return 1
  [[ "$metadata" == "$expected_owner:$expected_group:640:1" ]]
}

fedora_key_link_target() {
  local key_link="$1"
  local expected_directory="$2"
  local expected_extension="$3"
  local expected_owner="$4"
  local expected_group="$5"
  local output_variable="$6"
  local link_metadata link_target canonical_target target_name target_stem

  [[ -L "$key_link" ]] || return 1
  link_metadata="$(stat -c '%U:%G:%a:%h' -- "$key_link")" || return 1
  [[ "$link_metadata" == "$expected_owner:$expected_group:777:1" ]] || return 1

  link_target="$(readlink -- "$key_link")" || return 1
  [[ "$link_target" == "$expected_directory/"* ]] || return 1
  [[ "${link_target%/*}" == "$expected_directory" ]] || return 1
  target_name="${link_target##*/}"
  [[ "$target_name" == *".$expected_extension" ]] || return 1
  target_stem="${target_name%."${expected_extension}"}"
  [[ -n "$target_stem" && "$target_stem" != . && "$target_stem" != .. ]] \
    || return 1
  [[ "$target_stem" =~ ^[0-9A-Za-z._-]+$ ]] || return 1

  canonical_target="$(readlink -e -- "$key_link")" || return 1
  [[ "$canonical_target" == "$link_target" ]] || return 1
  [[ "${canonical_target%/*}" == "$expected_directory" ]] || return 1
  regular_key_file_is_safe \
    "$canonical_target" "$expected_owner" "$expected_group" || return 1
  printf -v "$output_variable" '%s' "$canonical_target"
}

key_pair_presence_state() {
  local public_key="${1:-$PUBLIC_KEY}"
  local private_key="${2:-$PRIVATE_KEY}"
  local public_directory="${3:-${public_key%/*}}"
  local private_directory="${4:-${private_key%/*}}"
  local expected_owner="${5:-root}"
  local expected_group="${6:-akmods}"
  local public_target private_target public_stem private_stem
  local public_absent=0 private_absent=0

  [[ ! -e "$public_key" && ! -L "$public_key" ]] && public_absent=1
  [[ ! -e "$private_key" && ! -L "$private_key" ]] && private_absent=1
  if (( public_absent == 1 && private_absent == 1 )); then
    printf '%s\n' missing
    return 0
  fi
  if (( public_absent != private_absent )); then
    printf '%s\n' incomplete
    return 0
  fi

  if [[ ! -L "$public_key" && ! -L "$private_key" ]]; then
    if regular_key_file_is_safe \
      "$public_key" "$expected_owner" "$expected_group" \
      && regular_key_file_is_safe \
        "$private_key" "$expected_owner" "$expected_group"; then
      printf '%s\n' complete
    else
      printf '%s\n' unsafe
    fi
    return 0
  fi

  if [[ -L "$public_key" && -L "$private_key" ]] \
    && fedora_key_link_target \
      "$public_key" "$public_directory" der \
      "$expected_owner" "$expected_group" public_target \
    && fedora_key_link_target \
      "$private_key" "$private_directory" priv \
      "$expected_owner" "$expected_group" private_target; then
    public_stem="${public_target##*/}"
    public_stem="${public_stem%.der}"
    private_stem="${private_target##*/}"
    private_stem="${private_stem%.priv}"
    if [[ "$public_stem" == "$private_stem" ]]; then
      printf '%s\n' complete
    else
      printf '%s\n' unsafe
    fi
  else
    printf '%s\n' unsafe
  fi
}

key_pair_matches() {
  local public_key="${1:-$PUBLIC_KEY}"
  local private_key="${2:-$PRIVATE_KEY}"
  local result=0

  KEY_CHECK_DIRECTORY="$(mktemp -d /var/tmp/galaxybook-akmods-key-check.XXXXXX)"
  chmod 0700 -- "$KEY_CHECK_DIRECTORY"
  if ! openssl x509 -inform DER -in "$public_key" -pubkey -noout 2>/dev/null \
    | openssl pkey -pubin -outform DER 2>/dev/null \
      > "$KEY_CHECK_DIRECTORY/public.der"; then
    result=1
  fi
  if (( result == 0 )) \
    && ! openssl pkey -in "$private_key" -pubout -outform DER 2>/dev/null \
    > "$KEY_CHECK_DIRECTORY/private.der"; then
    result=1
  fi
  if (( result == 0 )) \
    && ! cmp -s -- \
      "$KEY_CHECK_DIRECTORY/public.der" \
      "$KEY_CHECK_DIRECTORY/private.der"; then
    result=1
  fi
  rm -rf -- "$KEY_CHECK_DIRECTORY"
  KEY_CHECK_DIRECTORY=""
  return "$result"
}

managed_state_target_is_safe() {
  local target="$1"
  local minimum_size="$2"
  local maximum_size="$3"
  local expected_uid="${4:-0}"
  local expected_gid="${5:-0}"
  local metadata owner group mode links size

  [[ -e "$target" || -L "$target" ]] || return 0
  [[ ! -L "$target" && -f "$target" ]] || return 1
  metadata="$(stat -Lc '%u:%g:%a:%h:%s' -- "$target")" || return 1
  IFS=: read -r owner group mode links size <<< "$metadata"
  [[ "$owner" == "$expected_uid" \
    && "$group" == "$expected_gid" \
    && "$mode" == 644 \
    && "$links" == 1 \
    && "$size" =~ ^[0-9]+$ ]] || return 1
  (( size >= minimum_size && size <= maximum_size ))
}

managed_marker_target_is_safe() {
  managed_state_target_is_safe "$1" 1 4096
}

managed_public_certificate_target_is_safe() {
  managed_state_target_is_safe \
    "$1" 1 16384 "${2:-0}" "${3:-0}"
}

close_public_certificate_fd() {
  if [[ -n "$PUBLIC_CERTIFICATE_FD" ]]; then
    exec {PUBLIC_CERTIFICATE_FD}<&-
  fi
  PUBLIC_CERTIFICATE_FD=""
  PUBLIC_CERTIFICATE_FD_PATH=""
  PUBLIC_CERTIFICATE_SOURCE_METADATA=""
}

open_validated_public_certificate() {
  local source="$1"
  local expected_owner="${2:-root}"
  local expected_group="${3:-akmods}"
  local source_metadata fd_metadata source_size

  close_public_certificate_fd
  [[ -f "$source" && -r "$source" ]] || return 1
  source_metadata="$(
    stat -Lc '%d:%i:%s:%Y:%Z:%U:%G:%a:%h' -- "$source"
  )" || return 1
  IFS=: read -r _ _ source_size _ _ _ _ _ _ <<< "$source_metadata"
  [[ "$source_size" =~ ^[0-9]+$ ]] \
    && (( source_size >= 1 && source_size <= 16384 )) || return 1
  [[ "$source_metadata" == \
    *":$expected_owner:$expected_group:640:1" ]] || return 1

  exec {PUBLIC_CERTIFICATE_FD}< "$source" || return 1
  PUBLIC_CERTIFICATE_FD_PATH="/proc/$BASHPID/fd/$PUBLIC_CERTIFICATE_FD"
  fd_metadata="$(
    stat -Lc '%d:%i:%s:%Y:%Z:%U:%G:%a:%h' \
      -- "$PUBLIC_CERTIFICATE_FD_PATH"
  )" || {
    close_public_certificate_fd
    return 1
  }
  if [[ "$fd_metadata" != "$source_metadata" ]]; then
    close_public_certificate_fd
    return 1
  fi
  PUBLIC_CERTIFICATE_SOURCE_METADATA="$source_metadata"
}

public_certificate_source_is_unchanged() {
  local source="$1"
  local path_metadata fd_metadata

  [[ -n "$PUBLIC_CERTIFICATE_FD" \
    && -n "$PUBLIC_CERTIFICATE_FD_PATH" \
    && -n "$PUBLIC_CERTIFICATE_SOURCE_METADATA" ]] || return 1
  path_metadata="$(
    stat -Lc '%d:%i:%s:%Y:%Z:%U:%G:%a:%h' -- "$source"
  )" || return 1
  fd_metadata="$(
    stat -Lc '%d:%i:%s:%Y:%Z:%U:%G:%a:%h' \
      -- "$PUBLIC_CERTIFICATE_FD_PATH"
  )" || return 1
  [[ "$path_metadata" == "$PUBLIC_CERTIFICATE_SOURCE_METADATA" \
    && "$fd_metadata" == "$PUBLIC_CERTIFICATE_SOURCE_METADATA" ]]
}

publish_public_certificate_state() {
  local source="$1"
  local target="$2"
  local expected_cn="$3"
  local expected_fingerprint="$4"
  local expected_serial="$5"
  local source_owner="${6:-root}"
  local source_group="${7:-akmods}"
  local target_uid="${8:-0}"
  local target_gid="${9:-0}"
  local published_cn published_fingerprint published_serial

  managed_public_certificate_target_is_safe \
    "$target" "$target_uid" "$target_gid" || return 1
  open_validated_public_certificate \
    "$source" "$source_owner" "$source_group" || return 1

  PUBLIC_CERTIFICATE_TEMPORARY="$(
    mktemp "${target%/*}/.akmods-public-key.XXXXXX"
  )" || return 1
  if ! dd bs=64K status=none conv=fsync \
      <&"$PUBLIC_CERTIFICATE_FD" \
      > "$PUBLIC_CERTIFICATE_TEMPORARY" \
    || ! cmp -s -- \
      "$PUBLIC_CERTIFICATE_FD_PATH" "$PUBLIC_CERTIFICATE_TEMPORARY" \
    || ! public_certificate_source_is_unchanged "$source" \
    || ! certificate_identity \
      "$PUBLIC_CERTIFICATE_TEMPORARY" \
      published_cn published_fingerprint published_serial \
    || [[ "$published_cn" != "$expected_cn" \
      || "$published_fingerprint" != "$expected_fingerprint" \
      || "$published_serial" != "$expected_serial" ]] \
    || ! chown "$target_uid:$target_gid" -- "$PUBLIC_CERTIFICATE_TEMPORARY" \
    || ! chmod 0644 -- "$PUBLIC_CERTIFICATE_TEMPORARY" \
    || ! managed_public_certificate_target_is_safe \
      "$PUBLIC_CERTIFICATE_TEMPORARY" "$target_uid" "$target_gid" \
    || ! public_certificate_source_is_unchanged "$source" \
    || ! mv -fT -- "$PUBLIC_CERTIFICATE_TEMPORARY" "$target"; then
    return 1
  fi
  PUBLIC_CERTIFICATE_TEMPORARY=""
  sync -f -- "${target%/*}" || return 1

  certificate_identity \
    "$target" published_cn published_fingerprint published_serial \
    && [[ "$published_cn" == "$expected_cn" \
      && "$published_fingerprint" == "$expected_fingerprint" \
      && "$published_serial" == "$expected_serial" ]] \
    && managed_public_certificate_target_is_safe \
      "$target" "$target_uid" "$target_gid" \
    && cmp -s -- "$PUBLIC_CERTIFICATE_FD_PATH" "$target" \
    && public_certificate_source_is_unchanged "$source"
}

root_owned_directory_is_trusted() {
  local directory="$1"
  local metadata

  [[ ! -L "$directory" && -d "$directory" ]] || return 1
  metadata="$(stat -Lc '%u:%g:%a' -- "$directory")" || return 1
  [[ "$metadata" == 0:0:755 ]]
}

state_directory_is_safe() {
  local directory="$1"
  local expected_uid="${2:-0}"
  local expected_gid="${3:-0}"
  local metadata

  [[ ! -L "$directory" && -d "$directory" ]] || return 1
  metadata="$(stat -Lc '%u:%g:%a' -- "$directory")" || return 1
  [[ "$metadata" == "$expected_uid:$expected_gid:755" ]]
}

ensure_state_directory() {
  local directory="$1"
  local expected_uid="${2:-0}"
  local expected_gid="${3:-0}"

  if [[ ! -e "$directory" && ! -L "$directory" ]]; then
    install -d -o root -g root -m 0755 -- "$directory" || return 1
  fi
  state_directory_is_safe "$directory" "$expected_uid" "$expected_gid"
}

key_directory_is_safe() {
  local directory="$1"
  local expected_owner="${2:-root}"
  local expected_group="${3:-akmods}"
  local metadata

  [[ ! -L "$directory" && -d "$directory" ]] || return 1
  metadata="$(stat -Lc '%U:%G:%a' -- "$directory")" || return 1
  [[ "$metadata" == "$expected_owner:$expected_group:750" ]]
}

ensure_key_directory() {
  local directory="$1"

  if [[ ! -e "$directory" && ! -L "$directory" ]]; then
    install -d -o root -g akmods -m 0750 -- "$directory" \
      || fail "Não foi possível criar o diretório protegido $directory."
  fi
  key_directory_is_safe "$directory" root akmods \
    || fail "O diretório protegido $directory existente não é root:akmods 0750 e foi preservado sem alteração."
}

write_key_identity_marker() {
  local marker_path="$1"
  local key_cn="$2"
  local fingerprint_sha1="$3"
  local certificate_serial="$4"

  marker_cn_is_valid "$key_cn" \
    && normalized_sha1_identifier_is_valid "$fingerprint_sha1" \
    && normalized_certificate_serial_is_valid "$certificate_serial" \
    || return 1
  printf 'version=2\ncn=%s\nfingerprint_sha1=%s\ncertificate_serial=%s\n' \
    "$key_cn" "$fingerprint_sha1" "$certificate_serial" \
    > "$marker_path"
}

publish_key_identity_marker() {
  local key_cn="$1"
  local fingerprint_sha1="$2"
  local certificate_serial="$3"
  local published_value published_cn published_fingerprint published_serial

  if ! marker_cn_is_valid "$key_cn" \
    || ! normalized_sha1_identifier_is_valid "$fingerprint_sha1" \
    || ! normalized_certificate_serial_is_valid "$certificate_serial"; then
    fail "A identidade pública do certificado do akmods é inválida."
  fi

  if ! root_owned_directory_is_trusted /var \
    || ! root_owned_directory_is_trusted /var/lib; then
    fail "A hierarquia /var/lib não é um diretório root seguro para o estado do setup."
  fi
  ensure_state_directory "$STATE_DIRECTORY" \
    || fail "O diretório de estado do setup não é um diretório root:root 0755 seguro e foi preservado."
  exec {STATE_LOCK_FD}< "$STATE_DIRECTORY" \
    || fail "Não foi possível abrir o diretório de estado seguro do setup."
  flock --exclusive "$STATE_LOCK_FD" \
    || fail "Não foi possível obter o lock exclusivo do estado MOK."

  managed_marker_target_is_safe "$KEY_IDENTITY_MARKER" \
    || fail "O marcador MOK existente não é um arquivo root:root 0644 seguro e foi preservado."
  managed_public_certificate_target_is_safe "$PUBLIC_CERTIFICATE_STATE" \
    || fail "A cópia pública existente do certificado do akmods não é um arquivo root:root 0644 seguro e foi preservada."
  publish_public_certificate_state \
    "$PUBLIC_KEY" "$PUBLIC_CERTIFICATE_STATE" \
    "$key_cn" "$fingerprint_sha1" "$certificate_serial" \
    root akmods 0 0 \
    || fail "Não foi possível publicar atomicamente uma cópia pública íntegra do certificado do akmods."

  PENDING_TEMPORARY="$(mktemp "$STATE_DIRECTORY/.akmods-key.XXXXXX")"
  if ! write_key_identity_marker \
    "$PENDING_TEMPORARY" "$key_cn" \
    "$fingerprint_sha1" "$certificate_serial" \
    || ! chown root:root -- "$PENDING_TEMPORARY" \
    || ! chmod 0644 -- "$PENDING_TEMPORARY" \
    || ! managed_marker_target_is_safe "$PENDING_TEMPORARY" \
    || ! sync -f -- "$PENDING_TEMPORARY" \
    || ! mv -fT -- "$PENDING_TEMPORARY" "$KEY_IDENTITY_MARKER"; then
    fail "Não foi possível publicar atomicamente o marcador do pedido MOK."
  fi
  PENDING_TEMPORARY=""
  sync -f -- "$STATE_DIRECTORY" \
    || fail "Não foi possível sincronizar a publicação do estado MOK."

  published_value="$(<"$KEY_IDENTITY_MARKER")"
  managed_marker_target_is_safe "$KEY_IDENTITY_MARKER" \
    && [[ "$published_value" == \
      "version=2"$'\n'"cn=$key_cn"$'\n'"fingerprint_sha1=$fingerprint_sha1"$'\n'"certificate_serial=$certificate_serial" ]] \
    && [[ "$(wc -l < "$KEY_IDENTITY_MARKER")" == 4 ]] \
    && managed_public_certificate_target_is_safe "$PUBLIC_CERTIFICATE_STATE" \
    && certificate_identity \
      "$PUBLIC_CERTIFICATE_STATE" \
      published_cn published_fingerprint published_serial \
    && [[ "$published_cn" == "$key_cn" \
      && "$published_fingerprint" == "$fingerprint_sha1" \
      && "$published_serial" == "$certificate_serial" ]] \
    && cmp -s -- "$PUBLIC_CERTIFICATE_FD_PATH" "$PUBLIC_CERTIFICATE_STATE" \
    && public_certificate_source_is_unchanged "$PUBLIC_KEY" \
    || fail "O marcador MOK publicado não passou na validação final."
  close_public_certificate_fd
  flock --unlock "$STATE_LOCK_FD" \
    || fail "Não foi possível liberar o lock do estado MOK."
  exec {STATE_LOCK_FD}<&-
  STATE_LOCK_FD=""
}

finish_enrolled_key() {
  local key_cn="$1"
  local fingerprint_sha1="$2"
  local certificate_serial="$3"

  publish_key_identity_marker \
    "$key_cn" "$fingerprint_sha1" "$certificate_serial"
  printf '%s\n' "A chave do akmods já está inscrita no MOK."
}

main() {
  local key_cn key_fingerprint key_serial key_state

  (( EUID == 0 )) || fail "Este helper precisa ser executado como root."
  require_command awk
  require_command cat
  require_command chmod
  require_command chown
  require_command cmp
  require_command dd
  require_command flock
  require_command install
  require_command kmodgenca
  require_command mktemp
  require_command mokutil
  require_command mv
  require_command openssl
  require_command readlink
  require_command rm
  require_command sg
  require_command stat
  require_command sync
  require_command tr
  require_command wc
  trap cleanup EXIT

  if ! root_owned_directory_is_trusted /etc \
    || ! root_owned_directory_is_trusted /etc/pki \
    || ! root_owned_directory_is_trusted /etc/pki/akmods; then
    fail "A hierarquia /etc/pki/akmods não é um diretório root seguro."
  fi
  ensure_key_directory "$CERTIFICATE_DIRECTORY"
  ensure_key_directory "$PRIVATE_KEY_DIRECTORY"
  key_state="$(key_pair_presence_state)"
  case "$key_state" in
    missing)
      kmodgenca -a
      ;;
    complete)
      ;;
    incomplete)
      fail "O par de chaves do akmods está incompleto. O setup preservou a chave existente em vez de rotacioná-la silenciosamente."
      ;;
    unsafe)
      fail "O par de chaves do akmods tem estrutura ou permissões inseguras e foi preservado."
      ;;
    *)
      fail "Não foi possível classificar o par de chaves do akmods."
      ;;
  esac
  [[ "$(key_pair_presence_state)" == complete ]] \
    || fail "Não foi possível gerar um par de chaves local completo para o akmods."
  key_pair_matches \
    || fail "O certificado e a chave privada do akmods não formam o mesmo par; nenhum deles foi substituído."
  certificate_identity \
    "$PUBLIC_KEY" key_cn key_fingerprint key_serial \
    || fail "Não foi possível identificar de forma íntegra o certificado do akmods."

  if mok_key_is_enrolled "$PUBLIC_KEY" "$key_fingerprint"; then
    finish_enrolled_key "$key_cn" "$key_fingerprint" "$key_serial"
    return 0
  fi

  HASH_FILE="$(mktemp)"
  chmod 0600 -- "$HASH_FILE"
  generate_mok_password_hash_file "$HASH_FILE" \
    || fail "Não foi possível gerar de forma íntegra o hash da senha temporária do MOK."
  mokutil --import "$PUBLIC_KEY" --hash-file "$HASH_FILE"

  if mok_request_is_pending "$key_fingerprint"; then
    publish_key_identity_marker "$key_cn" "$key_fingerprint" "$key_serial"
    printf '%s\n' "O pedido de importação da chave do akmods foi criado. Reinicie o notebook e conclua 'Enroll MOK' na tela azul do boot usando a senha definida nesta etapa."
    return 0
  fi
  fail "A chave do akmods foi preparada, mas o mokutil não listou nenhum pedido pendente. Revise a saída acima antes de continuar."
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  main "$@"
fi
