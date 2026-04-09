#!/usr/bin/env sh
set -eu

APP_NAME="vpn-desktop"
HELPER_NAME="vpn-desktopd"
GROUP_NAME="vpn-desktop"

PREFIX="${PREFIX:-/usr}"
BIN_DIR="${BIN_DIR:-$PREFIX/bin}"
LIBEXEC_DIR="${LIBEXEC_DIR:-$PREFIX/libexec}"
SYSTEMD_DIR_ETC="${SYSTEMD_DIR_ETC:-/etc/systemd/system}"
SYSTEMD_DIR_LIB="${SYSTEMD_DIR_LIB:-/lib/systemd/system}"
SYSTEMD_DIR_USR_LIB="${SYSTEMD_DIR_USR_LIB:-/usr/lib/systemd/system}"
DESKTOP_DIR="${DESKTOP_DIR:-$PREFIX/share/applications}"
ICON_DIR_SCALABLE="${ICON_DIR_SCALABLE:-$PREFIX/share/icons/hicolor/scalable/apps}"
ICON_DIR_PNG="${ICON_DIR_PNG:-$PREFIX/share/icons/hicolor/256x256/apps}"

GUI_BIN="$BIN_DIR/$APP_NAME"
HELPER_BIN="$LIBEXEC_DIR/$HELPER_NAME"
DESKTOP_FILE="$DESKTOP_DIR/$APP_NAME.desktop"
ICON_SVG="$ICON_DIR_SCALABLE/$APP_NAME.svg"
ICON_PNG="$ICON_DIR_PNG/$APP_NAME.png"
SOCKET_PATH="/run/$HELPER_NAME.sock"
SOCKET_UNIT="$HELPER_NAME.socket"
SERVICE_UNIT="$HELPER_NAME.service"

EXIT_CODE=0

log() {
    printf '%s\n' "[check] $*"
}

ok() {
    printf '%s\n' "[ok] $*"
}

warn() {
    printf '%s\n' "[warn] $*"
}

fail() {
    printf '%s\n' "[fail] $*"
    EXIT_CODE=1
}

has_cmd() {
    command -v "$1" >/dev/null 2>&1
}

file_exists_any() {
    for path in "$@"; do
        if [ -e "$path" ] || [ -L "$path" ]; then
            return 0
        fi
    done
    return 1
}

report_header() {
    printf '%s\n' ""
    printf '%s\n' "========================================"
    printf '%s\n' " Verificación de instalación de VPN Desktop"
    printf '%s\n' "========================================"
    printf '%s\n' ""
}

check_file_executable() {
    label="$1"
    path="$2"

    if [ -x "$path" ]; then
        ok "$label presente y ejecutable: $path"
    elif [ -e "$path" ]; then
        fail "$label existe pero no es ejecutable: $path"
    else
        fail "$label no encontrado: $path"
    fi
}

check_file_readable() {
    label="$1"
    path="$2"

    if [ -r "$path" ]; then
        ok "$label presente: $path"
    elif [ -e "$path" ]; then
        fail "$label existe pero no es legible: $path"
    else
        fail "$label no encontrado: $path"
    fi
}

check_systemd_unit_file() {
    unit="$1"

    if file_exists_any \
        "$SYSTEMD_DIR_ETC/$unit" \
        "$SYSTEMD_DIR_LIB/$unit" \
        "$SYSTEMD_DIR_USR_LIB/$unit"
    then
        ok "unit de systemd encontrada: $unit"
    else
        fail "unit de systemd no encontrada: $unit"
    fi
}

check_group_exists() {
    if has_cmd getent; then
        if getent group "$GROUP_NAME" >/dev/null 2>&1; then
            ok "grupo del sistema presente: $GROUP_NAME"
        else
            fail "grupo del sistema no encontrado: $GROUP_NAME"
        fi
    else
        warn "no se encontró 'getent'; no se pudo comprobar el grupo '$GROUP_NAME'"
    fi
}

check_user_group_membership() {
    current_user="${SUDO_USER:-$(id -un 2>/dev/null || printf '')}"

    if [ -z "$current_user" ]; then
        warn "no se pudo determinar el usuario actual para comprobar el grupo '$GROUP_NAME'"
        return
    fi

    if has_cmd id; then
        if id -nG "$current_user" 2>/dev/null | tr ' ' '\n' | grep -Fx "$GROUP_NAME" >/dev/null 2>&1; then
            ok "el usuario '$current_user' pertenece al grupo '$GROUP_NAME'"
        else
            warn "el usuario '$current_user' NO pertenece al grupo '$GROUP_NAME'"
            warn "si quieres usar la app sin errores de permisos, ejecuta:"
            warn "  sudo usermod -a -G $GROUP_NAME $current_user"
            warn "y luego cierra sesión y vuelve a entrar"
        fi
    else
        warn "no se encontró 'id'; no se pudo comprobar pertenencia al grupo"
    fi
}

check_socket_path() {
    if [ -S "$SOCKET_PATH" ]; then
        ok "socket runtime presente: $SOCKET_PATH"

        if has_cmd stat; then
            mode="$(stat -c '%a' "$SOCKET_PATH" 2>/dev/null || printf '')"
            owner="$(stat -c '%U' "$SOCKET_PATH" 2>/dev/null || printf '')"
            group="$(stat -c '%G' "$SOCKET_PATH" 2>/dev/null || printf '')"

            [ -n "$mode" ] && log "socket mode: $mode"
            [ -n "$owner" ] && log "socket owner: $owner"
            [ -n "$group" ] && log "socket group: $group"

            if [ "$group" = "$GROUP_NAME" ]; then
                ok "grupo del socket correcto: $group"
            elif [ -n "$group" ]; then
                warn "grupo del socket inesperado: $group (esperado: $GROUP_NAME)"
            fi
        fi
    elif [ -e "$SOCKET_PATH" ]; then
        fail "la ruta existe, pero no es un socket Unix: $SOCKET_PATH"
    else
        warn "socket runtime no encontrado: $SOCKET_PATH"
        warn "esto puede ser normal si el socket aún no ha sido activado o si systemd no lo ha arrancado"
    fi
}

check_systemd_status() {
    if ! has_cmd systemctl; then
        warn "systemctl no está disponible; no se puede comprobar el estado de las units"
        return
    fi

    if systemctl list-unit-files "$SOCKET_UNIT" >/dev/null 2>&1; then
        socket_enabled="$(systemctl is-enabled "$SOCKET_UNIT" 2>/dev/null || true)"
        socket_active="$(systemctl is-active "$SOCKET_UNIT" 2>/dev/null || true)"

        log "$SOCKET_UNIT enabled: ${socket_enabled:-desconocido}"
        log "$SOCKET_UNIT active: ${socket_active:-desconocido}"

        case "$socket_enabled" in
            enabled|static|indirect)
                ok "estado razonable de habilitación para $SOCKET_UNIT: $socket_enabled"
                ;;
            *)
                warn "$SOCKET_UNIT no aparece habilitado: ${socket_enabled:-desconocido}"
                ;;
        esac

        case "$socket_active" in
            active)
                ok "$SOCKET_UNIT está activo"
                ;;
            *)
                warn "$SOCKET_UNIT no está activo: ${socket_active:-desconocido}"
                ;;
        esac
    else
        warn "systemd no reconoce la unit $SOCKET_UNIT"
    fi

    if systemctl list-unit-files "$SERVICE_UNIT" >/dev/null 2>&1; then
        service_enabled="$(systemctl is-enabled "$SERVICE_UNIT" 2>/dev/null || true)"
        service_active="$(systemctl is-active "$SERVICE_UNIT" 2>/dev/null || true)"

        log "$SERVICE_UNIT enabled: ${service_enabled:-desconocido}"
        log "$SERVICE_UNIT active: ${service_active:-desconocido}"

        case "$service_active" in
            active|inactive)
                ok "estado del servicio observado: $service_active"
                ;;
            failed)
                warn "$SERVICE_UNIT está en estado failed"
                ;;
            *)
                warn "$SERVICE_UNIT devuelve estado: ${service_active:-desconocido}"
                ;;
        esac

        case "$service_enabled" in
            enabled|disabled|static|indirect)
                ok "estado de habilitación del servicio observado: $service_enabled"
                ;;
            *)
                warn "$SERVICE_UNIT devuelve un estado de habilitación no esperado: ${service_enabled:-desconocido}"
                ;;
        esac
    else
        warn "systemd no reconoce la unit $SERVICE_UNIT"
    fi
}

check_openvpn() {
    if ! has_cmd openvpn; then
        fail "no se encontró 'openvpn' en el sistema"
        return
    fi

    version_line="$(openvpn --version 2>/dev/null | sed -n '1p' || true)"
    if [ -n "$version_line" ]; then
        ok "openvpn está instalado"
        log "$version_line"
    else
        warn "openvpn existe, pero no se pudo obtener su versión"
    fi
}

check_desktop_entry_contents() {
    if [ ! -r "$DESKTOP_FILE" ]; then
        return
    fi

    if grep -Eq '^Exec=.*/vpn-desktop$|^Exec=vpn-desktop$' "$DESKTOP_FILE"; then
        ok "desktop entry contiene Exec coherente"
    else
        warn "desktop entry no parece contener un Exec esperado"
    fi

    if grep -Eq '^Icon=vpn-desktop$' "$DESKTOP_FILE"; then
        ok "desktop entry contiene Icon=vpn-desktop"
    else
        warn "desktop entry no parece referenciar el icono esperado"
    fi
}

check_icon_assets() {
    if [ -r "$ICON_SVG" ]; then
        ok "icono SVG presente: $ICON_SVG"
    elif [ -r "$ICON_PNG" ]; then
        ok "icono PNG presente: $ICON_PNG"
    else
        warn "no se encontró ni icono SVG ni PNG instalado"
    fi
}

print_next_steps() {
    printf '%s\n' ""
    printf '%s\n' "Siguientes pasos recomendados:"
    printf '%s\n' "  1. Verifica el socket:"
    printf '%s\n' "       systemctl status $SOCKET_UNIT"
    printf '%s\n' "  2. Verifica el servicio:"
    printf '%s\n' "       systemctl status $SERVICE_UNIT"
    printf '%s\n' "  3. Comprueba el socket runtime:"
    printf '%s\n' "       ls -l $SOCKET_PATH"
    printf '%s\n' "  4. Comprueba OpenVPN:"
    printf '%s\n' "       openvpn --version"
    printf '%s\n' "  5. Si tu usuario fue añadido al grupo '$GROUP_NAME', cierra sesión y vuelve a entrar."
}

main() {
    report_header

    log "Comprobando binarios instalados..."
    check_file_executable "GUI" "$GUI_BIN"
    check_file_executable "helper" "$HELPER_BIN"

    printf '%s\n' ""
    log "Comprobando assets de escritorio..."
    check_file_readable "desktop entry" "$DESKTOP_FILE"
    check_desktop_entry_contents
    check_icon_assets

    printf '%s\n' ""
    log "Comprobando integración con systemd..."
    check_systemd_unit_file "$SOCKET_UNIT"
    check_systemd_unit_file "$SERVICE_UNIT"
    check_systemd_status

    printf '%s\n' ""
    log "Comprobando permisos y grupo..."
    check_group_exists
    check_user_group_membership
    check_socket_path

    printf '%s\n' ""
    log "Comprobando dependencias de runtime..."
    check_openvpn

    print_next_steps

    printf '%s\n' ""
    if [ "$EXIT_CODE" -eq 0 ]; then
        ok "Verificación completada sin errores críticos."
    else
        fail "La verificación detectó problemas que conviene corregir antes de probar la app."
    fi

    exit "$EXIT_CODE"
}

main "$@"
