#!/usr/bin/env bash

set -euo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly APP_ID="com.caioregis.GalaxyBookSetup"
readonly MAKE_CMD="${MAKE:-make}"
readonly APP_BINARY="${REPO_ROOT}/target/release/galaxybook-setup"
readonly ICON_SOURCE="${REPO_ROOT}/assets/galaxybook-setup.svg"
readonly DESKTOP_SOURCE="${REPO_ROOT}/data/${APP_ID}.desktop"
readonly LOCAL_BIN_DIR="${HOME}/.local/bin"
readonly LOCAL_APPS_DIR="${HOME}/.local/share/applications"
readonly LOCAL_ICON_DIR="${HOME}/.local/share/icons/hicolor/scalable/apps"
readonly WRAPPER_PATH="${LOCAL_BIN_DIR}/galaxybook-setup"
readonly DESKTOP_ENTRY_PATH="${LOCAL_APPS_DIR}/${APP_ID}.desktop"
readonly ICON_TARGET_PATH="${LOCAL_ICON_DIR}/${APP_ID}.svg"

require_cmd() {
	command -v "$1" >/dev/null 2>&1 || {
		echo "Missing required command: $1" >&2
		exit 1
	}
}

main() {
	require_cmd bash
	require_cmd "${MAKE_CMD}"
	"${MAKE_CMD}" -C "${REPO_ROOT}" build >/dev/null

	mkdir -p "${LOCAL_BIN_DIR}" "${LOCAL_APPS_DIR}" "${LOCAL_ICON_DIR}"

	install -m 0644 "${ICON_SOURCE}" "${ICON_TARGET_PATH}"

cat >"${WRAPPER_PATH}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
if [[ "\${XDG_SESSION_TYPE:-}" == "wayland" && -n "\${WAYLAND_DISPLAY:-}" ]]; then
	exec env -u DISPLAY "${APP_BINARY}" "\$@"
else
	exec "${APP_BINARY}" "\$@"
fi
EOF
	chmod 0755 "${WRAPPER_PATH}"

	sed \
		-e "s|@EXEC@|${WRAPPER_PATH}|g" \
		-e "s|@ICON@|${APP_ID}|g" \
		-e "s|@STARTUP_WM_CLASS@|${APP_ID}|g" \
		"${DESKTOP_SOURCE}" >"${DESKTOP_ENTRY_PATH}"

	if command -v desktop-file-validate >/dev/null 2>&1; then
		desktop-file-validate "${DESKTOP_ENTRY_PATH}"
	fi

	if command -v update-desktop-database >/dev/null 2>&1; then
		update-desktop-database "${LOCAL_APPS_DIR}" || true
	fi

	if command -v gtk-update-icon-cache >/dev/null 2>&1; then
		gtk-update-icon-cache -f -t "${HOME}/.local/share/icons/hicolor" >/dev/null 2>&1 || true
	fi
}

main "$@"
