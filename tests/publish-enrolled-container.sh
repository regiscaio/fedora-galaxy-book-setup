#!/usr/bin/bash
# Optional end-to-end check; run outside an RPM build with rootless Podman.
set -euo pipefail

if [[ ${1:-} != --fixture ]]; then
  [[ $# == 0 && $(id -u) != 0 && $(podman info --format '{{.Host.Security.Rootless}}') == true ]]
  repo=$(cd "$(dirname "$0")/.." && pwd)
  # Stream only the three test inputs. Nothing on the host is mounted or relabeled.
  tar -C "$repo" -cf - tests/publish-enrolled-container.sh scripts/{prepare-secure-boot-key,kmod-lifecycle-lock}.sh |
    podman run --rm -i --network=none --pull=never --hostname setup-mok-fixture --user 0:0 \
      localhost/galaxybook-setup-builder:fedora44 /bin/bash -ec \
      'cd /tmp && tar -xf - && exec bash tests/publish-enrolled-container.sh --fixture'
  exit
fi
[[ $# == 1 && -e /run/.containerenv && $EUID == 0 && $(uname -n) == setup-mok-fixture ]]
certs=/etc/pki/akmods/certs
state=/var/lib/galaxybook-setup
[[ ! -e /etc/pki/akmods && ! -e $state ]]
getent group akmods >/dev/null || groupadd -r akmods
install -d -o root -g root -m 0755 /etc/pki/akmods
install -d -o root -g akmods -m 0755 "$certs"
make_cert() {
  openssl req -x509 -newkey rsa:2048 -noenc -days 1 -subj "/CN=$1" \
    -keyout /dev/null -outform DER -out /tmp/fixture.der 2>/dev/null
  install -o root -g akmods -m 0640 /tmp/fixture.der "$certs/fixture.der"
}
make_cert fixture-enrolled
cp -- "$certs/fixture.der" /tmp/enrolled.der
ln -s "$certs/fixture.der" "$certs/public_key.der"
chown -h root:akmods "$certs/public_key.der"
cat >/usr/bin/mokutil <<'SH'
#!/usr/bin/bash
case "$1" in
  --test-key) [[ $# == 2 ]] && cmp -s /tmp/enrolled.der "$2";;
  --list-enrolled) [[ $# == 1 ]];; # No other certificates in the fixture.
  *) touch /tmp/forbidden; exit 99;;
esac
SH
printf '%s\n' '#!/usr/bin/bash' 'touch /tmp/forbidden; exit 99' >/usr/bin/kmodgenca
chmod 0755 /usr/bin/{mokutil,kmodgenca}

bash scripts/prepare-secure-boot-key.sh --publish-enrolled </dev/null
cmp "$certs/fixture.der" "$state/akmods-public-key.der"
grep -qx 'version=2' "$state/akmods-key.cn"
grep -qx 'cn=fixture-enrolled' "$state/akmods-key.cn"
for f in "$state"/{akmods-public-key.der,akmods-key.cn}; do
  [[ $(stat -c '%u:%g:%a:%h' "$f") == 0:0:644:1 ]]
done
setpriv --reuid=1000 --regid=1000 --clear-groups /usr/bin/bash -ec \
  '! test -r "$1" && test -r "$2" && test -r "$3"' _ \
  "$certs/public_key.der" "$state/akmods-public-key.der" "$state/akmods-key.cn"

cp -- "$state/akmods-key.cn" /tmp/previous-marker
make_cert fixture-not-enrolled
if bash scripts/prepare-secure-boot-key.sh --publish-enrolled </dev/null >/dev/null 2>&1; then
  echo 'An unregistered certificate was incorrectly accepted' >&2; exit 1
fi
cmp /tmp/enrolled.der "$state/akmods-public-key.der"
cmp /tmp/previous-marker "$state/akmods-key.cn"
[[ ! -e /etc/pki/akmods/private && ! -e /tmp/forbidden ]]
echo 'Public MOK evidence: enrolled/unregistered and unprivileged reader checks passed.'
