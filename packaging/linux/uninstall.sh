#!/bin/sh
set -eu

APP_NAME="vpn-desktop"
HELPER_NAME="vpn-desktopd"
GROUP_NAME="vpn-desktop"

PREFIX="${PREFIX:-/usr}"
BIN_DIR="${BIN_DIR:-$PREFIX/bin}"
LIBEXEC_DIR="${LIBEXEC_DIR:-$PREFIX/libexec}"
SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"
DESKTOP_DIR="${DESKTOP_DIR:-$PREFIX/share/applications}"
ICON_BASE_DIR="${ICON_BASE_DIR:-$PREFIX/share/icons/hicolor}"

GUI_BIN="$BIN_DIR/$APP_NAME"
DAEMON_BIN="$LIBEXEC_DIR/$HELPER_NAME"
DESKTOP_FILE="$DESKTOP_DIR/$APP_NAME.desktop"
SERVICE_FILE="$SYSTEMD_DIR/$HELPER_NAME.service"
SOCKET_FILE="$SYSTEMD_DIR/$HELPER_NAME.socket"
SOCKET_PATH="/run/$HELPER_NAME.sock"
ICON_PNG_256="$ICON_BASE_DIR/256x256/apps/$APP_NAME.png"
ICON_SVG_SCALABLE="$ICON_BASE_DIR/scalable/apps/$APP_NAME.svg"

require_root() {
    if [ "$(id -u)" != "0" ]; then
        echo "Este script debe ejecutarse como root." >&2
        exit 1
    fi
}

remove_if_exists() {
    target="$1"
    if [ -e "$target" ] || [ -L "$target" ]; then
        echo "Eliminando: $target"
        rm -f "$target"
    else
        echo "No existe, se omite: $target"
    fi
}

stop_systemd_units() {
    if command -v systemctl >/dev/null 2>&1; then
        echo "Deteniendo y deshabilitando units de systemd..."
        systemctl disable --now vpn-desktopd.socket >/dev/null 2>&1 || true
        systemctl disable --now vpn-desktopd.service >/dev/null 2>&1 || true
        systemctl daemon-reload >/dev/null 2>&1 || true
    else
        echo "systemctl no está disponible; se omite gestión de systemd."
    fi
}

remove_runtime_socket() {
    if [ -S "$SOCKET_PATH" ] || [ -e "$SOCKET_PATH" ]; then
        echo "Eliminando socket runtime: $SOCKET_PATH"
        rm -f "$SOCKET_PATH"
    fi
}

maybe_remove_group() {
    if getent group "$GROUP_NAME" >/dev/null 2>&1; then
        echo
        echo "El grupo '$GROUP_NAME' sigue existiendo."
        echo "Si no lo necesitas, puedes borrarlo manualmente con:"
        echo "  groupdel $GROUP_NAME"
    fi
}

print_summary() {
    echo
    echo "Desinstalación completada."
    echo
    echo "Elementos eliminados:"
    echo "  - $GUI_BIN"
    echo "  - $DAEMON_BIN"
    echo "  - $DESKTOP_FILE"
    echo "  - $SERVICE_FILE"
    echo "  - $SOCKET_FILE"
    echo "  - $ICON_PNG_256"
    echo "  - $ICON_SVG_SCALABLE"
    echo
    echo "Elementos NO eliminados automáticamente:"
    echo "  - Configuración de usuario en ~/.config/vpn-desktop"
    echo "  - Secretos del keyring del usuario"
    echo "  - Grupo del sistema '$GROUP_NAME'"
    echo
    echo "Si quieres una limpieza total, revisa esos elementos manualmente."
}

main() {
    require_root

    echo "==> Desinstalando $APP_NAME"

    stop_systemd_units

    remove_if_exists "$GUI_BIN"
    remove_if_exists "$DAEMON_BIN"
    remove_if_exists "$DESKTOP_FILE"
    remove_if_exists "$SERVICE_FILE"
    remove_if_exists "$SOCKET_FILE"
    remove_if_exists "$ICON_PNG_256"
    remove_if_exists "$ICON_SVG_SCALABLE"

    remove_runtime_socket
    maybe_remove_group
    print_summary
}

main "$@"
