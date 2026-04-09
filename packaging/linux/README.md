# Linux packaging guide

Esta carpeta contiene los recursos necesarios para instalar `vpn-desktop` en un sistema Linux con una arquitectura basada en:

- GUI sin privilegios
- helper privilegiado `vpn-desktopd`
- comunicación por socket Unix
- activación y gestión mediante `systemd`

## Contenido

- `install.sh`: instala la GUI, el helper, las units de `systemd`, el `.desktop` y el icono.
- `uninstall.sh`: elimina los archivos instalados y desactiva las units.
- `systemd/vpn-desktopd.service`: servicio del helper privilegiado.
- `systemd/vpn-desktopd.socket`: socket Unix para activar el helper bajo demanda.
- `vpn-desktop.desktop`: lanzador de escritorio.
- `icons/vpn-desktop.svg`: icono SVG de la aplicación.

## Requisitos

Antes de instalar, asegúrate de tener:

- Linux con `systemd`
- `openvpn` instalado en el sistema
- permisos de administrador para ejecutar la instalación
- Rust y Cargo si quieres que el script compile los binarios automáticamente cuando no existan en `target/release`

## Instalación rápida

Desde la raíz del proyecto:

```/dev/null/install.sh#L1-3
cd vpn-desktop
sudo ./packaging/linux/install.sh
```

El instalador hace lo siguiente:

1. Compila en `release` si los binarios no existen.
2. Instala la GUI en `/usr/bin/vpn-desktop`.
3. Instala el helper en `/usr/libexec/vpn-desktopd`.
4. Instala:
   - `vpn-desktopd.service`
   - `vpn-desktopd.socket`
5. Instala el `.desktop`.
6. Instala el icono SVG y, si hay herramientas disponibles, también puede generar un PNG.
7. Crea el grupo `vpn-desktop` si no existe.
8. Añade al usuario objetivo al grupo `vpn-desktop`.
9. Ejecuta `systemctl daemon-reload`.
10. Habilita y arranca `vpn-desktopd.socket`.

## Flujo de uso

Después de la instalación:

- la GUI se ejecuta como usuario normal
- la GUI habla con el helper a través de `/run/vpn-desktopd.sock`
- el helper arranca mediante `systemd` cuando hace falta
- no debería ser necesario pedir contraseña en cada conexión diaria

## Grupo y permisos

El socket se instala para acceso por grupo:

- grupo: `vpn-desktop`
- modo esperado del socket: `0660`

Si el instalador añade tu usuario a ese grupo, normalmente tendrás que cerrar sesión y volver a entrar para que el cambio tenga efecto.

## Verificación básica

Puedes comprobar que el socket está habilitado con:

```/dev/null/systemctl.txt#L1-2
systemctl status vpn-desktopd.socket
systemctl status vpn-desktopd.service
```

Y puedes verificar que el socket existe con:

```/dev/null/socket.txt#L1-1
ls -l /run/vpn-desktopd.sock
```

## Verificación postinstalación

Después de instalar la app, conviene hacer una comprobación rápida del entorno para validar helper, socket, permisos y OpenVPN.

### Script sugerido de verificación

Puedes usar una verificación manual como esta:

```/dev/null/vpn-desktop-verify.sh#L1-18
#!/usr/bin/env sh
set -eu

echo "==> Comprobando socket del helper"
ls -l /run/vpn-desktopd.sock

echo
echo "==> Comprobando units de systemd"
systemctl status vpn-desktopd.socket --no-pager || true
systemctl status vpn-desktopd.service --no-pager || true

echo
echo "==> Comprobando OpenVPN"
openvpn --version | head -n 1

echo
echo "==> Comprobando grupo del usuario actual"
id
```

### Qué deberías comprobar

- que exista `/run/vpn-desktopd.sock`
- que `vpn-desktopd.socket` esté habilitado o activo
- que `openvpn` esté instalado
- que tu usuario pertenezca al grupo `vpn-desktop`

Si el socket existe pero no puedes usarlo desde la GUI, normalmente el problema está en los permisos del grupo o en que todavía no has cerrado sesión después de añadir el usuario al grupo.

## Desinstalación

Para eliminar la instalación del sistema:

```/dev/null/uninstall.sh#L1-2
cd vpn-desktop
sudo ./packaging/linux/uninstall.sh
```

La desinstalación:

- deshabilita y detiene `vpn-desktopd.socket`
- deshabilita y detiene `vpn-desktopd.service`
- elimina binarios, units, `.desktop` e iconos instalados
- no elimina automáticamente:
  - `~/.config/vpn-desktop`
  - secretos del keyring del usuario
  - el grupo del sistema `vpn-desktop`

## Rutas habituales

Por defecto, la instalación usa estas rutas:

- GUI: `/usr/bin/vpn-desktop`
- helper: `/usr/libexec/vpn-desktopd`
- service: `/etc/systemd/system/vpn-desktopd.service`
- socket: `/etc/systemd/system/vpn-desktopd.socket`
- socket runtime: `/run/vpn-desktopd.sock`
- desktop entry: `/usr/share/applications/vpn-desktop.desktop`
- icono SVG: `/usr/share/icons/hicolor/scalable/apps/vpn-desktop.svg`

## Construcción de un paquete `.deb`

Además del instalador manual, esta carpeta ya incluye un flujo funcional para generar un paquete `.deb` local para Debian o Ubuntu.

### Script disponible

Puedes construir el paquete con:

```/dev/null/build-deb.sh#L1-2
cd vpn-desktop
sh ./packaging/linux/build-deb.sh
```

El script:

1. compila los binarios en modo `release` si todavía no existen
2. crea un directorio temporal de staging
3. copia al staging:
   - `/usr/bin/vpn-desktop`
   - `/usr/libexec/vpn-desktopd`
   - `/usr/share/applications/vpn-desktop.desktop`
   - `/usr/share/icons/hicolor/scalable/apps/vpn-desktop.svg`
   - `/lib/systemd/system/vpn-desktopd.service`
   - `/lib/systemd/system/vpn-desktopd.socket`
4. genera scripts de mantenedor Debian
5. construye el `.deb` final con `dpkg-deb`

### Resultado esperado

Por defecto, el paquete se genera en una ruta como:

- `target/deb-build/vpn-desktop_0.1.0-1_amd64.deb`

Y puede instalarse con:

```/dev/null/dpkg-install.sh#L1-1
sudo dpkg -i target/deb-build/vpn-desktop_0.1.0-1_amd64.deb
```

### Contenido del paquete

El paquete Debian básico generado incluye:

- binario GUI
- helper root
- archivo `.desktop`
- icono SVG
- unit `vpn-desktopd.service`
- unit `vpn-desktopd.socket`
- licencia/atribución del icono en documentación
- scripts `postinst`, `prerm` y `postrm`

### Comportamiento de instalación del paquete

En la versión actual, el `postinst` del paquete:

1. crea el grupo `vpn-desktop` si aún no existe
2. recarga `systemd`
3. habilita y arranca `vpn-desktopd.socket`
4. actualiza cachés de escritorio e iconos cuando las utilidades están disponibles
5. intenta añadir el usuario objetivo al grupo `vpn-desktop` si puede determinarlo
6. muestra un aviso para reloguear si se añadió el usuario al grupo

### Comportamiento de desinstalación del paquete

En la versión actual:

- `prerm` deshabilita y detiene:
  - `vpn-desktopd.socket`
  - `vpn-desktopd.service`
- `postrm`:
  - recarga `systemd`
  - actualiza cachés de escritorio e iconos si procede

Por defecto, la desinstalación conserva:

- la configuración del usuario
- los secretos del keyring
- el grupo `vpn-desktop`

### Dependencias y requisitos

A nivel de paquete, la base actual contempla especialmente:

- `openvpn`
- `systemd`

Además, en el sistema objetivo seguirán siendo necesarias las bibliotecas gráficas que requiera el binario compilado.

## Notas

- Esta arquitectura es preferible a lanzar `sudo` o `pkexec` desde la GUI para cada conexión.
- El helper debe mantenerse pequeño y centrado en operaciones de OpenVPN.
- Las credenciales no deben guardarse en texto plano en el config TOML.
- Esta carpeta ya incluye la base para un futuro paquete `.deb`.