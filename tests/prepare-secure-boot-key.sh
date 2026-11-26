#!/usr/bin/env bash

set -euo pipefail

TEST_DIRECTORY="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIRECTORY
REPOSITORY_ROOT="$(cd -- "$TEST_DIRECTORY/.." && pwd)"
readonly REPOSITORY_ROOT
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf -- "$TEST_ROOT"' EXIT

# shellcheck source=../scripts/prepare-secure-boot-key.sh
source "$REPOSITORY_ROOT/scripts/prepare-secure-boot-key.sh"

fail_test() {
  printf 'prepare-secure-boot-key test failed: %s\n' "$1" >&2
  exit 1
}

write_prepare_lifecycle_fixture() {
  local fixture="$1"

  printf '%s\n' \
    'source "$PREPARE_CANONICAL_LIFECYCLE_HELPER"' \
    'kmod_lifecycle_lock_path="$PREPARE_LIFECYCLE_LOCK_PATH"' \
    'kmod_lifecycle_lock_ancestry=("$PREPARE_LIFECYCLE_TEST_ROOT" "$PREPARE_LIFECYCLE_LOCK_PARENT")' \
    'kmod_lifecycle_lock_expected_uid="$EUID"' \
    'kmod_lifecycle_lock_expected_gid="$(id -g)"' \
    'kmod_lifecycle_lock_timeout_seconds=1' \
    > "$fixture"
}

test_prepare_helper_uses_its_packaged_lifecycle_source() {
  env lifecycle_helper=/tmp/unsafe-lifecycle \
    bash --noprofile --norc -c '
      set -euo pipefail
      source "$1"
      [[ "$lifecycle_helper" == "${1%/*}/kmod-lifecycle-lock.sh" ]]
    ' bash "$REPOSITORY_ROOT/scripts/prepare-secure-boot-key.sh"
}

test_global_precedes_all_signing_key_mutations() {
  local helper="$REPOSITORY_ROOT/scripts/prepare-secure-boot-key.sh"
  local acquire inspect generate import publish

  acquire="$(grep -n '^[[:space:]]*acquire_kmod_lifecycle_lock \\' "$helper" | tail -1 | cut -d: -f1)"
  inspect="$(grep -n '^[[:space:]]*key_state="\$(key_pair_presence_state)"$' "$helper" | cut -d: -f1)"
  generate="$(grep -n '^[[:space:]]*kmodgenca -a$' "$helper" | cut -d: -f1)"
  import="$(grep -n '^[[:space:]]*mokutil --import ' "$helper" | cut -d: -f1)"
  publish="$(grep -n '^[[:space:]]*publish_key_identity_marker ' "$helper" | tail -1 | cut -d: -f1)"
  [[ -n "$acquire" && -n "$inspect" && -n "$generate" \
    && -n "$import" && -n "$publish" ]]
  (( acquire < inspect && inspect < generate && generate < import \
    && import < publish ))
}

test_global_failure_stops_before_signing_key_work() {
  local test_root lifecycle_fixture trace status

  test_root="$(mktemp -d)"
  lifecycle_fixture="$test_root/lifecycle-helper"
  trace="$test_root/trace"
  printf '%s\n' \
    'acquire_kmod_lifecycle_lock() { return 75; }' \
    'validate_kmod_lifecycle_lock() { return 1; }' \
    > "$lifecycle_fixture"
  if (
    caller_is_root() { return 0; }
    require_command() { :; }
    lifecycle_helper="$lifecycle_fixture"
    root_owned_directory_is_trusted() { printf 'KEY_WORK\n' >> "$trace"; }
    kmodgenca() { printf 'KMODGENCA\n' >> "$trace"; }
    mokutil() { printf 'MOKUTIL\n' >> "$trace"; }
    main
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  [[ "$status" -ne 0 && ! -e "$trace" ]]
  rm -rf -- "$test_root"
}

test_external_global_holder_blocks_key_work_with_a_timeout() {
  local test_root lock_parent lock_path lifecycle_fixture trace
  local holder_ready holder_release holder_pid status

  test_root="$(mktemp -d /tmp/galaxybook-mok-global-test.XXXXXX)"
  lock_parent="$test_root/lock"
  lock_path="$lock_parent/galaxybook-kmod-lifecycle.lock"
  lifecycle_fixture="$test_root/lifecycle-helper"
  trace="$test_root/trace"
  holder_ready="$test_root/holder.ready"
  holder_release="$test_root/holder.release"
  mkdir -m 0755 -- "$lock_parent"
  chmod 0755 -- "$test_root"
  (umask 077; : > "$lock_path")
  write_prepare_lifecycle_fixture "$lifecycle_fixture"

  (
    flock --exclusive "$lock_path" bash -c '
      : > "$1"
      while [[ ! -e "$2" ]]; do /usr/bin/sleep 0.01; done
    ' bash "$holder_ready" "$holder_release"
  ) &
  holder_pid=$!
  for ((attempt = 1; attempt <= 200; attempt++)); do
    [[ -e "$holder_ready" ]] && break
    /usr/bin/sleep 0.01
  done
  [[ -e "$holder_ready" ]]

  if (
    export PREPARE_CANONICAL_LIFECYCLE_HELPER="$REPOSITORY_ROOT/scripts/kmod-lifecycle-lock.sh"
    export PREPARE_LIFECYCLE_LOCK_PATH="$lock_path"
    export PREPARE_LIFECYCLE_TEST_ROOT="$test_root"
    export PREPARE_LIFECYCLE_LOCK_PARENT="$lock_parent"
    caller_is_root() { return 0; }
    require_command() { :; }
    lifecycle_helper="$lifecycle_fixture"
    root_owned_directory_is_trusted() { printf 'KEY_WORK\n' >> "$trace"; }
    main
  ) >/dev/null 2>&1; then
    status=0
  else
    status=$?
  fi
  : > "$holder_release"
  wait "$holder_pid"
  [[ "$status" -ne 0 && ! -e "$trace" ]]
  rm -rf -- "$test_root"
}

test_inherited_global_is_reused_through_final_key_validation() {
  local test_root lock_parent lock_path lifecycle_fixture trace

  test_root="$(mktemp -d /tmp/galaxybook-mok-inherited-test.XXXXXX)"
  lock_parent="$test_root/lock"
  lock_path="$lock_parent/galaxybook-kmod-lifecycle.lock"
  lifecycle_fixture="$test_root/lifecycle-helper"
  trace="$test_root/trace"
  mkdir -m 0755 -- "$lock_parent"
  chmod 0755 -- "$test_root"
  write_prepare_lifecycle_fixture "$lifecycle_fixture"

  (
    export PREPARE_CANONICAL_LIFECYCLE_HELPER="$REPOSITORY_ROOT/scripts/kmod-lifecycle-lock.sh"
    export PREPARE_LIFECYCLE_LOCK_PATH="$lock_path"
    export PREPARE_LIFECYCLE_TEST_ROOT="$test_root"
    export PREPARE_LIFECYCLE_LOCK_PARENT="$lock_parent"
    source "$lifecycle_fixture"
    acquire_kmod_lifecycle_lock
    inherited_fd="$GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD"

    caller_is_root() { return 0; }
    require_command() { :; }
    lifecycle_helper="$lifecycle_fixture"
    root_owned_directory_is_trusted() { return 0; }
    ensure_key_directory() { :; }
    key_pair_presence_state() { printf 'complete\n'; }
    key_pair_matches() { return 0; }
    certificate_identity() {
      printf -v "$2" '%s' galaxybook-test-key
      printf -v "$3" '%s' AA:BB
      printf -v "$4" '%s' AABB
    }
    mok_key_is_enrolled() {
      validate_kmod_lifecycle_lock
      [[ "$GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD" == "$inherited_fd" ]]
    }
    finish_enrolled_key() {
      validate_kmod_lifecycle_lock
      [[ "$GALAXYBOOK_KMOD_LIFECYCLE_LOCK_FD" == "$inherited_fd" ]]
      if flock --exclusive --nonblock "$lock_path" true; then
        return 91
      fi
      printf 'INHERITED_LOCK_HELD_THROUGH_PUBLICATION\n' >> "$trace"
    }
    main
  ) >/dev/null

  grep -Fxq INHERITED_LOCK_HELD_THROUGH_PUBLICATION "$trace"
  flock --exclusive --nonblock "$lock_path" true
  rm -rf -- "$test_root"
}

test_prepare_helper_uses_its_packaged_lifecycle_source
test_global_precedes_all_signing_key_mutations
test_global_failure_stops_before_signing_key_work
test_external_global_holder_blocks_key_work_with_a_timeout
test_inherited_global_is_reused_through_final_key_validation

test_publish_only_dispatch() (
  local trace="$TEST_ROOT/public-mode.trace"
  caller_is_root() { return 0; }
  require_command() { [[ "$1" != kmodgenca && "$1" != sg ]] || exit 96; }
  lifecycle_helper=/dev/null
  acquire_kmod_lifecycle_lock() { printf 'locked\n' >> "$trace"; }
  validate_kmod_lifecycle_lock() { grep -Fxq locked "$trace"; }
  root_owned_directory_is_trusted() { return 0; }
  publish_existing_enrollment() { require_kmod_lifecycle_lock; printf 'published\n' >> "$trace"; }
  ensure_key_directory() { exit 93; }
  key_pair_presence_state() { exit 94; }
  mokutil() { exit 95; }
  main --publish-enrolled
  [[ "$(<"$trace")" == $'locked\npublished' ]]
)
test_publish_only_dispatch
if ( main --publish-enrolled unexpected ) >/dev/null 2>&1; then
  fail_test 'unexpected arguments were accepted'
fi

expect_state() {
  local expected_state="$1"
  local public_key="$2"
  local private_key="$3"
  local public_directory="$4"
  local private_directory="$5"
  local owner="$6"
  local group="$7"
  local actual_state

  actual_state="$(
    key_pair_presence_state \
      "$public_key" "$private_key" \
      "$public_directory" "$private_directory" \
      "$owner" "$group"
  )"
  [[ "$actual_state" == "$expected_state" ]] \
    || fail_test "expected state $expected_state, got $actual_state"
}

(
  install() {
    printf '%s\n' "$*" > "$TEST_ROOT/install-arguments"
    mkdir -p -- "${*: -1}"
  }
  stat() {
    case "$*" in
      *%U:%G:%a*) printf '%s\n' root:akmods:750 ;;
      *%u:%g:%a*) printf '%s\n' 0:0:755 ;;
      *) return 1 ;;
    esac
  }

  ensure_key_directory "$TEST_ROOT/key-directory"
  grep -Fq -- '-d -o root -g akmods -m 0750' "$TEST_ROOT/install-arguments"
  mkdir -p "$TEST_ROOT/ancestry"
  root_owned_directory_is_trusted "$TEST_ROOT/ancestry"
  ensure_state_directory "$TEST_ROOT/new-state-directory"
  grep -Fq -- '-d -o root -g root -m 0755' "$TEST_ROOT/install-arguments"
)

(
  stat() {
    printf '%s\n' 0:1:755
  }
  if root_owned_directory_is_trusted "$TEST_ROOT"; then
    fail_test 'a state-ancestry directory with the wrong group was accepted'
  fi
)
(
  stat() {
    printf '%s\n' 0:0:750
  }
  if root_owned_directory_is_trusted "$TEST_ROOT"; then
    fail_test 'a state-ancestry directory with the wrong mode was accepted'
  fi
)

TEST_OWNER="$(stat -c '%U' -- "$TEST_ROOT")"
readonly TEST_OWNER
TEST_GROUP="$(stat -c '%G' -- "$TEST_ROOT")"
readonly TEST_GROUP
readonly PUBLIC_DIRECTORY="$TEST_ROOT/certs"
readonly PRIVATE_DIRECTORY="$TEST_ROOT/private"
readonly PUBLIC_DEFAULT="$PUBLIC_DIRECTORY/public_key.der"
readonly PRIVATE_DEFAULT="$PRIVATE_DIRECTORY/private_key.priv"
mkdir -m 0750 -- "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY"

TEST_UID="$(stat -c '%u' -- "$TEST_ROOT")"
readonly TEST_UID
TEST_GID="$(stat -c '%g' -- "$TEST_ROOT")"
readonly TEST_GID
key_directory_is_safe "$PUBLIC_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
mkdir -m 0700 -- "$TEST_ROOT/key-directory-0700"
if (ensure_key_directory "$TEST_ROOT/key-directory-0700") >/dev/null 2>&1; then
  fail_test 'an existing 0700 key directory was accepted'
fi
[[ "$(stat -c '%a' -- "$TEST_ROOT/key-directory-0700")" == 700 ]] \
  || fail_test 'an existing 0700 key directory was modified'
mkdir -m 0777 -- "$TEST_ROOT/key-directory-0777"
if (ensure_key_directory "$TEST_ROOT/key-directory-0777") >/dev/null 2>&1; then
  fail_test 'an existing 0777 key directory was accepted'
fi
[[ "$(stat -c '%a' -- "$TEST_ROOT/key-directory-0777")" == 777 ]] \
  || fail_test 'an existing 0777 key directory was modified'
mkdir -m 0750 -- "$TEST_ROOT/key-directory-wrong-owner"
if (
  install() {
    : > "$TEST_ROOT/unexpected-key-directory-mutation"
  }
  stat() {
    printf '%s\n' administrator:akmods:750
  }
  ensure_key_directory "$TEST_ROOT/key-directory-wrong-owner"
) >/dev/null 2>&1; then
  fail_test 'an existing key directory with the wrong owner was accepted'
fi
[[ ! -e "$TEST_ROOT/unexpected-key-directory-mutation" ]] \
  || fail_test 'an existing key directory with the wrong owner was mutated'
mkdir -m 0755 -- "$TEST_ROOT/state-0755"
ensure_state_directory "$TEST_ROOT/state-0755" "$TEST_UID" "$TEST_GID"
mkdir -m 0700 -- "$TEST_ROOT/state-0700"
if ensure_state_directory \
  "$TEST_ROOT/state-0700" "$TEST_UID" "$TEST_GID"; then
  fail_test 'an existing 0700 state directory was accepted'
fi
[[ "$(stat -c '%a' -- "$TEST_ROOT/state-0700")" == 700 ]] \
  || fail_test 'an existing 0700 state directory was widened'
mkdir -m 0777 -- "$TEST_ROOT/state-0777"
if ensure_state_directory \
  "$TEST_ROOT/state-0777" "$TEST_UID" "$TEST_GID"; then
  fail_test 'an existing 0777 state directory was accepted'
fi
[[ "$(stat -c '%a' -- "$TEST_ROOT/state-0777")" == 777 ]] \
  || fail_test 'an existing 0777 state directory was modified'
ln -s "$TEST_ROOT/state-0755" "$TEST_ROOT/state-link"
if ensure_state_directory \
  "$TEST_ROOT/state-link" "$TEST_UID" "$TEST_GID"; then
  fail_test 'a state-directory symlink was accepted'
fi

expect_state missing \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
printf '%s\n' public > "$PUBLIC_DEFAULT"
chmod 0640 -- "$PUBLIC_DEFAULT"
expect_state incomplete \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
printf '%s\n' private > "$PRIVATE_DEFAULT"
chmod 0640 -- "$PRIVATE_DEFAULT"
expect_state complete \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
chmod 0644 -- "$PRIVATE_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
chmod 0640 -- "$PRIVATE_DEFAULT"
ln -- "$PRIVATE_DEFAULT" "$PRIVATE_DIRECTORY/private-hardlink.priv"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
rm -f -- "$PRIVATE_DIRECTORY/private-hardlink.priv" \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT"

openssl req -x509 -newkey rsa:2048 -nodes \
  -subj '/CN=galaxybook-test-key' \
  -keyout "$PRIVATE_DIRECTORY/fedora44-key.priv" \
  -out "$TEST_ROOT/pair-public.pem" \
  -days 1 >/dev/null 2>&1
openssl x509 -in "$TEST_ROOT/pair-public.pem" -outform DER \
  -out "$PUBLIC_DIRECTORY/fedora44-key.der"
chmod 0640 -- \
  "$PUBLIC_DIRECTORY/fedora44-key.der" \
  "$PRIVATE_DIRECTORY/fedora44-key.priv"
ln -s "$PUBLIC_DIRECTORY/fedora44-key.der" "$PUBLIC_DEFAULT"
ln -s "$PRIVATE_DIRECTORY/fedora44-key.priv" "$PRIVATE_DEFAULT"
expect_state complete \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
key_pair_matches "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT"

certificate_cn=""
certificate_fingerprint=""
certificate_serial=""
certificate_identity \
  "$PUBLIC_DEFAULT" \
  certificate_cn certificate_fingerprint certificate_serial
[[ "$certificate_cn" == galaxybook-test-key ]] \
  || fail_test 'certificate CN was not extracted'
normalized_sha1_identifier_is_valid "$certificate_fingerprint" \
  || fail_test 'certificate fingerprint was not normalized'
normalized_certificate_serial_is_valid "$certificate_serial" \
  || fail_test 'certificate serial was not normalized'
openssl() {
  /usr/bin/openssl "$@"
  printf '%s\n' 'openssl warning' >&2
}
if certificate_identity \
  "$PUBLIC_DEFAULT" \
  certificate_cn certificate_fingerprint certificate_serial; then
  fail_test 'certificate identity accompanied by OpenSSL stderr was accepted'
fi
unset -f openssl
openssl() {
  /usr/bin/openssl "$@"
  printf '\377'
}
if certificate_identity \
  "$PUBLIC_DEFAULT" \
  certificate_cn certificate_fingerprint certificate_serial; then
  fail_test 'certificate identity containing invalid UTF-8 was accepted'
fi
unset -f openssl
normalized_serial=""
normalize_certificate_serial '01:ab:CD' normalized_serial
[[ "$normalized_serial" == 01ABCD ]] \
  || fail_test 'a colonized certificate serial was not normalized'
normalize_certificate_serial 0080 normalized_serial
[[ "$normalized_serial" == 80 ]] \
  || fail_test 'DER sign padding was not removed from a high-bit serial'
normalize_certificate_serial 0001 normalized_serial
[[ "$normalized_serial" == 01 ]] \
  || fail_test 'numeric leading-zero padding was not canonicalized'
if normalize_certificate_serial ABC normalized_serial; then
  fail_test 'an odd-length certificate serial was accepted'
fi
if normalize_certificate_serial 0000 normalized_serial; then
  fail_test 'an all-zero certificate serial was accepted'
fi

write_key_identity_marker \
  "$TEST_ROOT/key-identity.state" \
  "$certificate_cn" "$certificate_fingerprint" "$certificate_serial"
mapfile -t identity_lines < "$TEST_ROOT/key-identity.state"
[[ ${#identity_lines[@]} == 4 \
  && "${identity_lines[0]}" == version=2 \
  && "${identity_lines[1]}" == "cn=$certificate_cn" \
  && "${identity_lines[2]}" == "fingerprint_sha1=$certificate_fingerprint" \
  && "${identity_lines[3]}" == "certificate_serial=$certificate_serial" ]] \
  || fail_test 'versioned public identity marker has unexpected contents'
if write_key_identity_marker \
  "$TEST_ROOT/invalid-identity.state" \
  $'invalid\ncn' "$certificate_fingerprint" "$certificate_serial"; then
  fail_test 'control characters in a marker CN were accepted'
fi
if write_key_identity_marker \
  "$TEST_ROOT/invalid-identity.state" \
  valid-cn invalid "$certificate_serial"; then
  fail_test 'an invalid certificate fingerprint was accepted'
fi

readonly PUBLIC_STATE_TEST="$TEST_ROOT/published-public-key.der"
publish_public_certificate_state \
  "$PUBLIC_DEFAULT" "$PUBLIC_STATE_TEST" \
  "$certificate_cn" "$certificate_fingerprint" "$certificate_serial" \
  "$TEST_OWNER" "$TEST_GROUP" "$TEST_UID" "$TEST_GID" \
  || fail_test 'a valid public certificate could not be published'
managed_public_certificate_target_is_safe \
  "$PUBLIC_STATE_TEST" "$TEST_UID" "$TEST_GID" \
  || fail_test 'the published public certificate metadata is unsafe'
cmp -s -- "$PUBLIC_DEFAULT" "$PUBLIC_STATE_TEST" \
  || fail_test 'the published public certificate differs from its validated source'
close_public_certificate_fd

OTHER_UID=0
if (( TEST_UID == 0 )); then
  OTHER_UID=1
fi
if managed_public_certificate_target_is_safe \
  "$PUBLIC_STATE_TEST" "$OTHER_UID" "$TEST_GID"; then
  fail_test 'a public certificate with the wrong owner was accepted'
fi
chmod 0666 -- "$PUBLIC_STATE_TEST"
if managed_public_certificate_target_is_safe \
  "$PUBLIC_STATE_TEST" "$TEST_UID" "$TEST_GID"; then
  fail_test 'a world-writable public certificate was accepted'
fi
chmod 0644 -- "$PUBLIC_STATE_TEST"
ln -- "$PUBLIC_STATE_TEST" "$TEST_ROOT/public-state-hardlink.der"
if managed_public_certificate_target_is_safe \
  "$PUBLIC_STATE_TEST" "$TEST_UID" "$TEST_GID"; then
  fail_test 'a hard-linked public certificate was accepted'
fi
rm -f -- "$TEST_ROOT/public-state-hardlink.der" "$PUBLIC_STATE_TEST"

printf '%s\n' placeholder > "$TEST_ROOT/public-state-target"
chmod 0644 -- "$TEST_ROOT/public-state-target"
ln -s "$TEST_ROOT/public-state-target" "$TEST_ROOT/public-state-link.der"
if managed_public_certificate_target_is_safe \
  "$TEST_ROOT/public-state-link.der" "$TEST_UID" "$TEST_GID"; then
  fail_test 'a public-certificate symlink was accepted'
fi
if publish_public_certificate_state \
  "$PUBLIC_DEFAULT" "$TEST_ROOT/public-state-link.der" \
  "$certificate_cn" "$certificate_fingerprint" "$certificate_serial" \
  "$TEST_OWNER" "$TEST_GROUP" "$TEST_UID" "$TEST_GID"; then
  fail_test 'publication replaced an unsafe public-certificate symlink'
fi
cleanup
printf '' > "$TEST_ROOT/public-state-empty.der"
chmod 0644 -- "$TEST_ROOT/public-state-empty.der"
if managed_public_certificate_target_is_safe \
  "$TEST_ROOT/public-state-empty.der" "$TEST_UID" "$TEST_GID"; then
  fail_test 'a truncated public certificate was accepted'
fi
if publish_public_certificate_state \
  "$PUBLIC_DEFAULT" "$TEST_ROOT/public-state-mismatch.der" \
  galaxybook-rotated-key "$certificate_fingerprint" "$certificate_serial" \
  "$TEST_OWNER" "$TEST_GROUP" "$TEST_UID" "$TEST_GID"; then
  fail_test 'publication accepted an identity that does not match the DER'
fi
cleanup
[[ ! -e "$TEST_ROOT/public-state-mismatch.der" ]] \
  || fail_test 'a mismatched public certificate was published'

(
  unchanged_checks=0
  public_certificate_source_is_unchanged() {
    (( unchanged_checks += 1 ))
    (( unchanged_checks < 2 ))
  }
  if publish_public_certificate_state \
    "$PUBLIC_DEFAULT" "$TEST_ROOT/public-state-raced.der" \
    "$certificate_cn" "$certificate_fingerprint" "$certificate_serial" \
    "$TEST_OWNER" "$TEST_GROUP" "$TEST_UID" "$TEST_GID"; then
    fail_test 'publication accepted a source replacement during validation'
  fi
  cleanup
  [[ ! -e "$TEST_ROOT/public-state-raced.der" ]] \
    || fail_test 'a raced public certificate was published'
)

openssl req -x509 -newkey rsa:2048 -nodes \
  -subj '/CN=galaxybook-other-key' \
  -keyout "$TEST_ROOT/other-private.pem" \
  -out "$TEST_ROOT/other-public.pem" \
  -days 1 >/dev/null 2>&1
if key_pair_matches "$PUBLIC_DEFAULT" "$TEST_ROOT/other-private.pem"; then
  fail_test 'a mismatched certificate and private key were accepted'
fi

mkdir -p "$TEST_ROOT/target-directory"
printf '%s\n' regular > "$TEST_ROOT/target-file"
ln -s "$TEST_ROOT/target-file" "$TEST_ROOT/file-link"
ln -s "$TEST_ROOT/target-directory" "$TEST_ROOT/directory-link"

managed_marker_target_is_safe "$TEST_ROOT/absent"
if managed_marker_target_is_safe "$TEST_ROOT/file-link"; then
  fail_test 'a marker symlink to a file was accepted'
fi
if managed_marker_target_is_safe "$TEST_ROOT/directory-link"; then
  fail_test 'a marker symlink to a directory was accepted'
fi
if (ensure_key_directory "$TEST_ROOT/directory-link") >/dev/null 2>&1; then
  fail_test 'a protected key-directory symlink was accepted'
fi
if root_owned_directory_is_trusted "$TEST_ROOT/directory-link"; then
  fail_test 'a root-owned ancestry symlink was accepted'
fi

published_identity=""
publish_key_identity_marker() {
  published_identity="$1|$2|$3"
}
finish_enrolled_key \
  galaxybook-test-key "$certificate_fingerprint" "$certificate_serial" \
  > "$TEST_ROOT/enrolled-output"
[[ "$published_identity" == \
  "galaxybook-test-key|$certificate_fingerprint|$certificate_serial" ]]
grep -Fq -- "já está inscrita" "$TEST_ROOT/enrolled-output"

printf 'SHA1 Fingerprint: %s\n' "$certificate_fingerprint" \
  | mok_output_contains_fingerprint "$certificate_fingerprint"
different_fingerprint="${certificate_fingerprint%??}00"
if [[ "$different_fingerprint" == "$certificate_fingerprint" ]]; then
  different_fingerprint="${certificate_fingerprint%??}01"
fi
if printf 'SHA1 Fingerprint: %s\n' "$different_fingerprint" \
  | mok_output_contains_fingerprint "$certificate_fingerprint"; then
  fail_test 'a different MOK fingerprint was accepted'
fi

mok_test_key_status=0
mok_test_key_output=""
mok_test_key_stderr=""
mok_list_enrolled_output=""
mok_list_enrolled_stderr=""
mok_list_new_output=""
mok_list_new_stderr=""
mok_generate_hash_output=""
mok_generate_hash_stderr=""
mok_generate_hash_status=0
mokutil() {
  case "$1" in
    --test-key)
      printf '%s' "$mok_test_key_output"
      printf '%s' "$mok_test_key_stderr" >&2
      return "$mok_test_key_status"
      ;;
    --list-enrolled)
      printf '%s' "$mok_list_enrolled_output"
      printf '%s' "$mok_list_enrolled_stderr" >&2
      ;;
    --list-new)
      printf '%s' "$mok_list_new_output"
      printf '%s' "$mok_list_new_stderr" >&2
      ;;
    --generate-hash)
      printf '%s' "$mok_generate_hash_output"
      printf '%s' "$mok_generate_hash_stderr" >&2
      return "$mok_generate_hash_status"
      ;;
    *)
      return 1
      ;;
  esac
}
mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"
mok_test_key_output=$'trusted-looking output\377'
if mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"; then
  fail_test 'a successful --test-key containing invalid UTF-8 was accepted'
fi
mok_test_key_output=""
mok_test_key_stderr="warning\n"
if mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"; then
  fail_test 'a successful --test-key accompanied by stderr was accepted'
fi
mok_test_key_stderr="already enrolled\n"
mok_test_key_status=1
mok_list_enrolled_output="$(
  printf 'SHA1 Fingerprint: %s\n' "$certificate_fingerprint"
)"
mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"
mok_list_enrolled_stderr="warning\n"
if mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"; then
  fail_test 'an enrolled fingerprint accompanied by stderr was accepted'
fi
mok_list_enrolled_stderr=""
mok_list_enrolled_output="$(
  printf 'SHA1 Fingerprint: %s' "$certificate_fingerprint"
)"$'\377'
if mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"; then
  fail_test 'an enrolled fingerprint containing invalid UTF-8 was accepted'
fi
mok_list_enrolled_output="$(
  printf 'SHA1 Fingerprint: %s\n' "$different_fingerprint"
)"
if mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"; then
  fail_test 'a failed --test-key with a different enrolled fingerprint passed'
fi
mok_list_enrolled_output=""
if mok_key_is_enrolled "$PUBLIC_DEFAULT" "$certificate_fingerprint"; then
  fail_test 'locale-dependent --test-key failure text was accepted'
fi
mok_list_new_output="$(printf 'SHA1 Fingerprint: %s\n' "$certificate_fingerprint")"
mok_request_is_pending "$certificate_fingerprint"
mok_list_new_stderr="warning\n"
if mok_request_is_pending "$certificate_fingerprint"; then
  fail_test 'a pending fingerprint accompanied by stderr was accepted'
fi
mok_list_new_stderr=""
mok_list_new_output="$(
  printf 'SHA1 Fingerprint: %s' "$certificate_fingerprint"
)"$'\377'
if mok_request_is_pending "$certificate_fingerprint"; then
  fail_test 'a pending fingerprint containing invalid UTF-8 was accepted'
fi

generated_hash_file="$(mktemp "$TEST_ROOT/generated-hash.XXXXXX")"
chmod 0600 -- "$generated_hash_file"
generated_hash_fixture='$test$fixture$not.a.real.hash'
mok_generate_hash_output=$'input password: \ninput password again: \n'\
"$generated_hash_fixture"$'\n'
generate_mok_password_hash_file "$generated_hash_file"
IFS= read -r generated_hash_line < "$generated_hash_file"
[[ "$generated_hash_line" == "$generated_hash_fixture" \
  && "$(wc -l < "$generated_hash_file")" == 1 ]] \
  || fail_test 'canonical generated-hash output was not isolated safely'

printf '%s\n' unchanged > "$generated_hash_file"
mok_generate_hash_stderr="warning\n"
if generate_mok_password_hash_file "$generated_hash_file"; then
  fail_test 'trusted-looking generated-hash output with stderr was accepted'
fi
[[ "$(<"$generated_hash_file")" == unchanged ]] \
  || fail_test 'rejected generated-hash output modified the hash file'
mok_generate_hash_stderr=""
mok_generate_hash_status=1
if generate_mok_password_hash_file "$generated_hash_file"; then
  fail_test 'trusted-looking generated-hash output with failure status was accepted'
fi
mok_generate_hash_status=0
mok_generate_hash_output=$'input password: \ninput password again: \nnot-a-crypt-hash\n'
if generate_mok_password_hash_file "$generated_hash_file"; then
  fail_test 'malformed generated-hash output was accepted'
fi
mok_generate_hash_output=$'input password: \ninput password again: \n'\
"$generated_hash_fixture"$'\nextra\n'
if generate_mok_password_hash_file "$generated_hash_file"; then
  fail_test 'generated-hash output with an extra line was accepted'
fi
mok_generate_hash_output=$'input password: \ninput password again: \n'\
"$generated_hash_fixture"
if generate_mok_password_hash_file "$generated_hash_file"; then
  fail_test 'generated-hash output without its final newline was accepted'
fi
mok_generate_hash_output=$'input password: \ninput password again: \n'\
"$generated_hash_fixture"$'\n\377'
if generate_mok_password_hash_file "$generated_hash_file"; then
  fail_test 'trusted-looking generated-hash output containing invalid UTF-8 was accepted'
fi

clean_probe() {
  printf 'valid\n'
}
capture_clean_output clean_probe
[[ "$PROBE_STDOUT" == valid ]] \
  || fail_test 'a canonical clean probe was rejected'
clean_probe() {
  printf 'valid\n\n'
}
if capture_clean_output clean_probe; then
  fail_test 'a clean probe with an extra trailing line was accepted'
fi
clean_probe() {
  printf 'valid\000'
}
if capture_clean_output clean_probe; then
  fail_test 'binary clean-probe output was accepted'
fi
clean_probe() {
  printf 'valid\377\n'
}
if capture_clean_output clean_probe; then
  fail_test 'clean-probe output containing invalid UTF-8 was accepted'
fi
unset -f clean_probe
unset -f mokutil

# Rebuild the Fedora 44 fixture for adversarial link tests without exposing key data.
rm -f -- "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT"

make_valid_links() {
  rm -f -- "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT"
  ln -s "$PUBLIC_DIRECTORY/fedora44-key.der" "$PUBLIC_DEFAULT"
  ln -s "$PRIVATE_DIRECTORY/fedora44-key.priv" "$PRIVATE_DEFAULT"
}

make_valid_links
rm -f -- "$PRIVATE_DEFAULT"
expect_state incomplete \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PRIVATE_DEFAULT"
printf '%s\n' private > "$PRIVATE_DEFAULT"
chmod 0640 -- "$PRIVATE_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$PUBLIC_DIRECTORY/missing.der" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s public-target.der "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$TEST_ROOT/pair-public.pem" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

mkdir -p "$TEST_ROOT/certs-evil"
cp -- "$PUBLIC_DIRECTORY/fedora44-key.der" \
  "$TEST_ROOT/certs-evil/fedora44-key.der"
chmod 0640 -- "$TEST_ROOT/certs-evil/fedora44-key.der"
make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$TEST_ROOT/certs-evil/fedora44-key.der" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$PUBLIC_DIRECTORY/../certs/fedora44-key.der" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

mkdir -p "$PUBLIC_DIRECTORY/nested"
cp -- "$PUBLIC_DIRECTORY/fedora44-key.der" \
  "$PUBLIC_DIRECTORY/nested/fedora44-key.der"
chmod 0640 -- "$PUBLIC_DIRECTORY/nested/fedora44-key.der"
make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$PUBLIC_DIRECTORY/nested/fedora44-key.der" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
ln -s "$PUBLIC_DIRECTORY/fedora44-key.der" "$PUBLIC_DIRECTORY/chain.der"
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$PUBLIC_DIRECTORY/chain.der" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PRIVATE_DEFAULT"
ln -s "$PRIVATE_DIRECTORY/other-stem.priv" "$PRIVATE_DEFAULT"
cp -- "$PRIVATE_DIRECTORY/fedora44-key.priv" \
  "$PRIVATE_DIRECTORY/other-stem.priv"
chmod 0640 -- "$PRIVATE_DIRECTORY/other-stem.priv"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
chmod 0644 -- "$PRIVATE_DIRECTORY/fedora44-key.priv"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
chmod 0640 -- "$PRIVATE_DIRECTORY/fedora44-key.priv"

make_valid_links
ln -- "$PUBLIC_DIRECTORY/fedora44-key.der" \
  "$PUBLIC_DIRECTORY/fedora44-key-hardlink.der"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
rm -f -- "$PUBLIC_DIRECTORY/fedora44-key-hardlink.der"

make_valid_links
ln -P -- "$PUBLIC_DEFAULT" "$PUBLIC_DIRECTORY/public-link-hardlink.der"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
rm -f -- "$PUBLIC_DIRECTORY/public-link-hardlink.der"

make_valid_links
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" not-the-owner "$TEST_GROUP"

make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
ln -s "$PUBLIC_DEFAULT" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"

make_valid_links
rm -f -- "$PUBLIC_DEFAULT"
mkfifo "$PUBLIC_DIRECTORY/not-a-file.der"
ln -s "$PUBLIC_DIRECTORY/not-a-file.der" "$PUBLIC_DEFAULT"
expect_state unsafe \
  "$PUBLIC_DEFAULT" "$PRIVATE_DEFAULT" \
  "$PUBLIC_DIRECTORY" "$PRIVATE_DIRECTORY" "$TEST_OWNER" "$TEST_GROUP"
