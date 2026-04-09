#!/usr/bin/env sh
set -eu

APP_NAME="vpn-desktop"
HELPER_NAME="vpn-desktopd"
GROUP_NAME="vpn-desktop"

PREFIX="${PREFIX:-/usr}"
BIN_DIR="${BIN_DIR:-$PREFIX/bin}"
LIBEXEC_DIR="${LIBEXEC_DIR:-$PREFIX/libexec}"
SYSTEMD_DIR="${SYSTEMD_DIR:-/etc/systemd/system}"
DESKTOP_DIR="${DESKTOP_DIR:-$PREFIX/share/applications}"
ICON_DIR="${ICON_DIR:-$PREFIX/share/icons/hicolor/scalable/apps}"
ICON_PNG_DIR="${ICON_PNG_DIR:-$PREFIX/share/icons/hicolor/256x256/apps}"

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)

GUI_SOURCE="${GUI_SOURCE:-$PROJECT_DIR/target/release/$APP_NAME}"
HELPER_SOURCE="${HELPER_SOURCE:-$PROJECT_DIR/target/release/$HELPER_NAME}"

SERVICE_SRC="$SCRIPT_DIR/systemd/${HELPER_NAME}.service"
SOCKET_SRC="$SCRIPT_DIR/systemd/${HELPER_NAME}.socket"
DESKTOP_SRC="$SCRIPT_DIR/${APP_NAME}.desktop"

SERVICE_DST="$SYSTEMD_DIR/${HELPER_NAME}.service"
SOCKET_DST="$SYSTEMD_DIR/${HELPER_NAME}.socket"
DESKTOP_DST="$DESKTOP_DIR/${APP_NAME}.desktop"
ICON_SRC="$SCRIPT_DIR/icons/${APP_NAME}.svg"
ICON_DST="$ICON_DIR/${APP_NAME}.svg"
ICON_PNG_DST="$ICON_PNG_DIR/${APP_NAME}.png"

GUI_DST="$BIN_DIR/$APP_NAME"
HELPER_DST="$LIBEXEC_DIR/$HELPER_NAME"

log() {
    printf '%s\n' "[install] $*"
}

warn() {
    printf '%s\n' "[install][warn] $*" >&2
}

die() {
    printf '%s\n' "[install][error] $*" >&2
    exit 1
}

need_root() {
    if [ "$(id -u)" -ne 0 ]; then
        die "este script debe ejecutarse como root"
    fi
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "comando requerido no encontrado: $1"
}

have_cmd() {
    command -v "$1" >/dev/null 2>&1
}

install_file() {
    src="$1"
    dst="$2"
    mode="$3"

    [ -f "$src" ] || die "no existe el archivo requerido: $src"

    dst_dir=$(dirname "$dst")
    mkdir -p "$dst_dir"
    install -m "$mode" "$src" "$dst"
}

replace_exec_path_in_desktop() {
    file="$1"
    tmp="${file}.tmp"
    sed "s|^Exec=.*$|Exec=$GUI_DST|g" "$file" > "$tmp"
    mv "$tmp" "$file"
}

ensure_group() {
    if getent group "$GROUP_NAME" >/dev/null 2>&1; then
        log "grupo '$GROUP_NAME' ya existe"
    else
        log "creando grupo '$GROUP_NAME'"
        groupadd --system "$GROUP_NAME"
    fi
}

ensure_user_in_group() {
    target_user="${SUDO_USER:-${PKEXEC_UID:-}}"

    if [ -n "$target_user" ] && [ "$target_user" != "0" ]; then
        if printf '%s' "$target_user" | grep -Eq '^[0-9]+$'; then
            user_name=$(getent passwd "$target_user" | cut -d: -f1 || true)
        else
            user_name="$target_user"
        fi

        if [ -n "${user_name:-}" ] && id "$user_name" >/dev/null 2>&1; then
            if id -nG "$user_name" | tr ' ' '\n' | grep -Fx "$GROUP_NAME" >/dev/null 2>&1; then
                log "usuario '$user_name' ya pertenece al grupo '$GROUP_NAME'"
            else
                log "añadiendo usuario '$user_name' al grupo '$GROUP_NAME'"
                usermod -a -G "$GROUP_NAME" "$user_name"
                warn "el usuario '$user_name' debe cerrar sesión y volver a entrar para aplicar el grupo"
            fi
        fi
    elif [ -n "${INSTALL_USER:-}" ]; then
        if id "$INSTALL_USER" >/dev/null 2>&1; then
            log "añadiendo usuario '$INSTALL_USER' al grupo '$GROUP_NAME'"
            usermod -a -G "$GROUP_NAME" "$INSTALL_USER"
            warn "el usuario '$INSTALL_USER' debe cerrar sesión y volver a entrar para aplicar el grupo"
        else
            warn "INSTALL_USER está definido pero el usuario no existe: $INSTALL_USER"
        fi
    else
        warn "no se pudo detectar automáticamente el usuario final; usa INSTALL_USER=usuario si quieres añadirlo al grupo '$GROUP_NAME'"
    fi
}

reload_systemd() {
    if command -v systemctl >/dev/null 2>&1; then
        log "recargando systemd"
        systemctl daemon-reload
    else
        warn "systemctl no está disponible; recarga systemd manualmente si es necesario"
    fi
}

enable_units() {
    if command -v systemctl >/dev/null 2>&1; then
        log "habilitando y arrancando socket ${HELPER_NAME}.socket"
        systemctl enable --now "${HELPER_NAME}.socket"
    else
        warn "systemctl no está disponible; habilita manualmente ${HELPER_NAME}.socket"
    fi
}

build_if_missing() {
    if [ ! -x "$GUI_SOURCE" ] || [ ! -x "$HELPER_SOURCE" ]; then
        need_cmd cargo
        log "binarios no encontrados en target/release; compilando en release"
        cargo build --manifest-path "$PROJECT_DIR/Cargo.toml" --release
    fi
}

install_optional_png_icon() {
    src="$1"
    dst="$2"

    mkdir -p "$(dirname "$dst")"

    if have_cmd rsvg-convert; then
        log "generando icono PNG 256x256 con rsvg-convert en $dst"
        rsvg-convert -w 256 -h 256 "$src" -o "$dst"
    elif have_cmd inkscape; then
        log "generando icono PNG 256x256 con inkscape en $dst"
        inkscape "$src" --export-type=png --export-filename="$dst" -w 256 -h 256 >/dev/null 2>&1
    else
        warn "no se encontró rsvg-convert ni inkscape; se omite el icono PNG opcional"
        return 0
    fi
}

main() {
    need_root
    need_cmd install
    need_cmd getent
    need_cmd sed

    [ -f "$SERVICE_SRC" ] || die "falta el unit file: $SERVICE_SRC"
    [ -f "$SOCKET_SRC" ] || die "falta el socket file: $SOCKET_SRC"
    [ -f "$DESKTOP_SRC" ] || die "falta el desktop file: $DESKTOP_SRC"

    build_if_missing

    [ -x "$GUI_SOURCE" ] || die "binario GUI no encontrado o no ejecutable: $GUI_SOURCE"
    [ -x "$HELPER_SOURCE" ] || die "binario helper no encontrado o no ejecutable: $HELPER_SOURCE"

    log "creando directorios de instalación"
    mkdir -p "$BIN_DIR" "$LIBEXEC_DIR" "$SYSTEMD_DIR" "$DESKTOP_DIR" "$ICON_DIR" "$ICON_PNG_DIR"

    ensure_group
    ensure_user_in_group

    log "instalando binario GUI en $GUI_DST"
    install -m 0755 "$GUI_SOURCE" "$GUI_DST"

    log "instalando helper root en $HELPER_DST"
    install -m 0755 "$HELPER_SOURCE" "$HELPER_DST"

    log "instalando unit files de systemd"
    install_file "$SERVICE_SRC" "$SERVICE_DST" 0644
    install_file "$SOCKET_SRC" "$SOCKET_DST" 0644

    log "instalando archivo .desktop"
    install_file "$DESKTOP_SRC" "$DESKTOP_DST" 0644
    replace_exec_path_in_desktop "$DESKTOP_DST"

    if [ -f "$ICON_SRC" ]; then
        log "instalando icono SVG en $ICON_DST"
        install_file "$ICON_SRC" "$ICON_DST" 0644
        install_optional_png_icon "$ICON_SRC" "$ICON_PNG_DST"
    else
        warn "no se encontró icono SVG en $ICON_SRC; se omite la instalación del icono"
    fi

    if have_cmd gtk-update-icon-cache; then
        log "actualizando caché de iconos"
        gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
    fi

    if have_cmd update-desktop-database; then
        log "actualizando base de datos de aplicaciones"
        update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
    fi

    reload_systemd
    enable_units

    log "instalación completada"
    printf '%s\n' ""
    printf '%s\n' "Resumen:"
    printf '%s\n' "  GUI:        $GUI_DST"
    printf '%s\n' "  Helper:     $HELPER_DST"
    printf '%s\n' "  Service:    $SERVICE_DST"
    printf '%s\n' "  Socket:     $SOCKET_DST"
    printf '%s\n' "  Desktop:    $DESKTOP_DST"
    if [ -f "$ICON_DST" ]; then
        printf '%s\n' "  Icon SVG:   $ICON_DST"
    fi
    if [ -f "$ICON_PNG_DST" ]; then
        printf '%s\n' "  Icon PNG:   $ICON_PNG_DST"
    fi
    printf '%s\n' ""
    printf '%s\n' "Recuerda:"
    printf '%s\n' "  - Si se añadió tu usuario al grupo '$GROUP_NAME', cierra sesión y vuelve a entrar."
    printf '%s\n' "  - Asegúrate de tener 'openvpn' instalado en el sistema."
}

main "$@"
