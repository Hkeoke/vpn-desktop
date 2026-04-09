#!/usr/bin/env sh
set -eu

APP_NAME="vpn-desktop"
HELPER_NAME="vpn-desktopd"
PACKAGE_NAME="vpn-desktop"
PACKAGE_VERSION="${PACKAGE_VERSION:-0.1.0}"
PACKAGE_RELEASE="${PACKAGE_RELEASE:-1}"
MAINTAINER="${MAINTAINER:-VPN Desktop <noreply@example.com>}"
DESCRIPTION="${DESCRIPTION:-Desktop OpenVPN manager for Linux with privileged helper daemon}"
SECTION="${SECTION:-net}"
PRIORITY="${PRIORITY:-optional}"
ARCH="${ARCH:-$(dpkg --print-architecture)}"

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PROJECT_DIR=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)

BUILD_ROOT="${BUILD_ROOT:-$PROJECT_DIR/target/deb-build}"
STAGING_DIR="$BUILD_ROOT/${PACKAGE_NAME}_${PACKAGE_VERSION}-${PACKAGE_RELEASE}_${ARCH}"
DEBIAN_DIR="$STAGING_DIR/DEBIAN"

BIN_SRC="${BIN_SRC:-$PROJECT_DIR/target/release/$APP_NAME}"
HELPER_SRC="${HELPER_SRC:-$PROJECT_DIR/target/release/$HELPER_NAME}"

DESKTOP_SRC="$SCRIPT_DIR/vpn-desktop.desktop"
ICON_SRC="$SCRIPT_DIR/icons/vpn-desktop.svg"
SERVICE_SRC="$SCRIPT_DIR/systemd/${HELPER_NAME}.service"
SOCKET_SRC="$SCRIPT_DIR/systemd/${HELPER_NAME}.socket"
ICON_LICENSE_SRC="$SCRIPT_DIR/icons/LICENSE.txt"

BIN_DST="$STAGING_DIR/usr/bin/$APP_NAME"
HELPER_DST="$STAGING_DIR/usr/libexec/$HELPER_NAME"
DESKTOP_DST="$STAGING_DIR/usr/share/applications/$APP_NAME.desktop"
ICON_SVG_DST="$STAGING_DIR/usr/share/icons/hicolor/scalable/apps/$APP_NAME.svg"
SERVICE_DST="$STAGING_DIR/lib/systemd/system/${HELPER_NAME}.service"
SOCKET_DST="$STAGING_DIR/lib/systemd/system/${HELPER_NAME}.socket"
DOC_DIR="$STAGING_DIR/usr/share/doc/$PACKAGE_NAME"
ICON_LICENSE_DST="$DOC_DIR/icon-license.txt"

PACKAGE_FILE="$BUILD_ROOT/${PACKAGE_NAME}_${PACKAGE_VERSION}-${PACKAGE_RELEASE}_${ARCH}.deb"

log() {
    printf '%s\n' "[build-deb] $*"
}

warn() {
    printf '%s\n' "[build-deb][warn] $*" >&2
}

die() {
    printf '%s\n' "[build-deb][error] $*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "comando requerido no encontrado: $1"
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
    sed "s|^Exec=.*$|Exec=/usr/bin/$APP_NAME|g" "$file" > "$tmp"
    mv "$tmp" "$file"
}

replace_helper_path_in_service() {
    file="$1"
    tmp="${file}.tmp"
    sed "s|^ExecStart=.*$|ExecStart=/usr/libexec/$HELPER_NAME|g" "$file" > "$tmp"
    mv "$tmp" "$file"
}

cleanup_previous_build() {
    if [ -d "$STAGING_DIR" ]; then
        log "eliminando staging anterior: $STAGING_DIR"
        rm -rf "$STAGING_DIR"
    fi
    mkdir -p "$DEBIAN_DIR"
}

build_binaries_if_missing() {
    if [ ! -x "$BIN_SRC" ] || [ ! -x "$HELPER_SRC" ]; then
        need_cmd cargo
        log "binarios release no encontrados; compilando"
        cargo build --manifest-path "$PROJECT_DIR/Cargo.toml" --release
    fi

    [ -x "$BIN_SRC" ] || die "binario GUI no encontrado o no ejecutable: $BIN_SRC"
    [ -x "$HELPER_SRC" ] || die "binario helper no encontrado o no ejecutable: $HELPER_SRC"
}

write_control() {
    cat > "$DEBIAN_DIR/control" <<EOF
Package: $PACKAGE_NAME
Version: ${PACKAGE_VERSION}-${PACKAGE_RELEASE}
Section: $SECTION
Priority: $PRIORITY
Architecture: $ARCH
Maintainer: $MAINTAINER
Depends: openvpn, systemd
Description: $DESCRIPTION
 GUI Rust para gestionar perfiles OpenVPN en Linux.
 Incluye un helper privilegiado, activación por socket con systemd
 y assets de escritorio para integración en el sistema.
EOF
    chmod 0644 "$DEBIAN_DIR/control"
}

write_postinst() {
    cat > "$DEBIAN_DIR/postinst" <<'EOF'
#!/usr/bin/env sh
set -eu

GROUP_NAME="vpn-desktop"
HELPER_NAME="vpn-desktopd"

log() {
    printf '%s\n' "[postinst] $*"
}

warn() {
    printf '%s\n' "[postinst][warn] $*" >&2
}

ensure_group() {
    if getent group "$GROUP_NAME" >/dev/null 2>&1; then
        log "grupo '$GROUP_NAME' ya existe"
    else
        log "creando grupo '$GROUP_NAME'"
        groupadd --system "$GROUP_NAME"
    fi
}

maybe_add_user_to_group() {
    target_user="${SUDO_USER:-}"

    if [ -z "$target_user" ] && [ -n "${INSTALL_USER:-}" ]; then
        target_user="$INSTALL_USER"
    fi

    if [ -n "$target_user" ] && id "$target_user" >/dev/null 2>&1; then
        if id -nG "$target_user" | tr ' ' '\n' | grep -Fx "$GROUP_NAME" >/dev/null 2>&1; then
            log "usuario '$target_user' ya pertenece al grupo '$GROUP_NAME'"
        else
            log "añadiendo usuario '$target_user' al grupo '$GROUP_NAME'"
            usermod -a -G "$GROUP_NAME" "$target_user"
            warn "el usuario '$target_user' debe cerrar sesión y volver a entrar para aplicar el grupo"
        fi
    else
        warn "no se pudo determinar el usuario final para añadir al grupo '$GROUP_NAME'"
    fi
}

reload_systemd() {
    if command -v systemctl >/dev/null 2>&1; then
        systemctl daemon-reload || true
    fi
}

enable_socket() {
    if command -v systemctl >/dev/null 2>&1; then
        log "habilitando y arrancando ${HELPER_NAME}.socket"
        systemctl enable --now "${HELPER_NAME}.socket" || true
    else
        warn "systemctl no está disponible; habilita manualmente ${HELPER_NAME}.socket"
    fi
}

update_desktop_db() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
    fi
}

update_icon_cache() {
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q /usr/share/icons/hicolor >/dev/null 2>&1 || true
    fi
}

case "${1:-configure}" in
    configure|triggered)
        ensure_group
        maybe_add_user_to_group
        reload_systemd
        enable_socket
        update_desktop_db
        update_icon_cache
        ;;
esac

exit 0
EOF
    chmod 0755 "$DEBIAN_DIR/postinst"
}

write_prerm() {
    cat > "$DEBIAN_DIR/prerm" <<'EOF'
#!/usr/bin/env sh
set -eu

HELPER_NAME="vpn-desktopd"

if command -v systemctl >/dev/null 2>&1; then
    systemctl disable --now "${HELPER_NAME}.socket" >/dev/null 2>&1 || true
    systemctl disable --now "${HELPER_NAME}.service" >/dev/null 2>&1 || true
fi

exit 0
EOF
    chmod 0755 "$DEBIAN_DIR/prerm"
}

write_postrm() {
    cat > "$DEBIAN_DIR/postrm" <<'EOF'
#!/usr/bin/env sh
set -eu

HELPER_NAME="vpn-desktopd"

reload_systemd() {
    if command -v systemctl >/dev/null 2>&1; then
        systemctl daemon-reload >/dev/null 2>&1 || true
    fi
}

update_desktop_db() {
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database /usr/share/applications >/dev/null 2>&1 || true
    fi
}

update_icon_cache() {
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -q /usr/share/icons/hicolor >/dev/null 2>&1 || true
    fi
}

case "${1:-remove}" in
    remove|purge)
        reload_systemd
        update_desktop_db
        update_icon_cache
        ;;
esac

exit 0
EOF
    chmod 0755 "$DEBIAN_DIR/postrm"
}

copy_payload() {
    log "instalando binarios en staging"
    install_file "$BIN_SRC" "$BIN_DST" 0755
    install_file "$HELPER_SRC" "$HELPER_DST" 0755

    log "instalando desktop entry"
    install_file "$DESKTOP_SRC" "$DESKTOP_DST" 0644
    replace_exec_path_in_desktop "$DESKTOP_DST"

    log "instalando units de systemd"
    install_file "$SERVICE_SRC" "$SERVICE_DST" 0644
    install_file "$SOCKET_SRC" "$SOCKET_DST" 0644
    replace_helper_path_in_service "$SERVICE_DST"

    if [ -f "$ICON_SRC" ]; then
        log "instalando icono SVG"
        install_file "$ICON_SRC" "$ICON_SVG_DST" 0644
    else
        warn "no se encontró icono SVG: $ICON_SRC"
    fi

    mkdir -p "$DOC_DIR"
    if [ -f "$ICON_LICENSE_SRC" ]; then
        install_file "$ICON_LICENSE_SRC" "$ICON_LICENSE_DST" 0644
    fi
}

build_deb() {
    need_cmd dpkg-deb
    log "construyendo paquete .deb"
    dpkg-deb --build --root-owner-group "$STAGING_DIR" "$PACKAGE_FILE"
}

print_summary() {
    printf '%s\n' ""
    printf '%s\n' "Paquete generado:"
    printf '%s\n' "  $PACKAGE_FILE"
    printf '%s\n' ""
    printf '%s\n' "Instalación sugerida:"
    printf '%s\n' "  sudo dpkg -i $PACKAGE_FILE"
    printf '%s\n' ""
    printf '%s\n' "Después de instalar:"
    printf '%s\n' "  - Asegúrate de tener 'openvpn' instalado."
    printf '%s\n' "  - Revisa el estado de ${HELPER_NAME}.socket con systemctl."
    printf '%s\n' "  - Si tu usuario fue añadido al grupo 'vpn-desktop', cierra sesión y vuelve a entrar."
}

main() {
    need_cmd install
    need_cmd sed
    need_cmd dpkg

    [ -f "$DESKTOP_SRC" ] || die "falta el archivo desktop: $DESKTOP_SRC"
    [ -f "$SERVICE_SRC" ] || die "falta el service file: $SERVICE_SRC"
    [ -f "$SOCKET_SRC" ] || die "falta el socket file: $SOCKET_SRC"

    cleanup_previous_build
    build_binaries_if_missing
    write_control
    write_postinst
    write_prerm
    write_postrm
    copy_payload
    build_deb
    print_summary
}

main "$@"
