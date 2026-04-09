# VPN Desktop

cargo build --manifest-path vpn-desktop/Cargo.toml --release && sh vpn-desktop/packaging/linux/build-deb.sh

Aplicación desktop en Rust para Linux que gestiona perfiles OpenVPN con una interfaz gráfica, soporte de proxy y credenciales guardadas en el keyring del sistema.

## Estado del proyecto

El proyecto está migrando desde un enfoque basado en `sudo/pkexec` lanzados desde la GUI hacia una arquitectura más correcta para Linux:

- **GUI sin privilegios**
- **helper root persistente**
- **socket Unix para IPC**
- **servicio systemd**
- **instalación del sistema con assets dedicados**

Ese enfoque evita pedir la contraseña de root en cada conexión y deja la aplicación empaquetable de una forma profesional.

---

## Estructura del proyecto

```text
vpn-desktop/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── app/
│   ├── config.rs
│   ├── ipc.rs
│   ├── ipc_client.rs
│   ├── daemon.rs
│   ├── vpn.rs
│   └── bin/
│       └── vpn-desktopd.rs
└── packaging/
    └── linux/
        ├── systemd/
        │   ├── vpn-desktopd.service
        │   └── vpn-desktopd.socket
        ├── install.sh
        ├── uninstall.sh
        └── vpn-desktop.desktop
```

---

## Nuevo directorio `packaging`

Se añade una estructura dedicada para empaquetado e instalación:

```text
packaging/
└── linux/
    ├── icons/
    │   └── vpn-desktop.svg
    ├── systemd/
    │   ├── vpn-desktopd.service
    │   └── vpn-desktopd.socket
    ├── install.sh
    ├── uninstall.sh
    └── vpn-desktop.desktop
```

### Objetivo de esta carpeta

Centralizar todos los assets necesarios para instalar correctamente la app en Linux:

- unidades `systemd`
- socket de activación del helper root
- scripts de instalación/desinstalación
- archivo `.desktop`
- icono real de la aplicación
- metadatos y documentación de empaquetado

---

## Qué contendrá cada archivo

### `packaging/linux/systemd/vpn-desktopd.service`

Servicio root del helper de backend.

Responsabilidades:

- arrancar el helper `vpn-desktopd`
- gestionar el ciclo de vida del proceso root
- integrarse con `systemd`
- servir peticiones IPC de la GUI

Ejemplo esperado:

- correr como `root`
- ligado al socket `vpn-desktopd.socket`
- con `ExecStart=/usr/libexec/vpn-desktopd` o ruta equivalente

---

### `packaging/linux/systemd/vpn-desktopd.socket`

Socket Unix expuesto al frontend.

Responsabilidades:

- crear `/run/vpn-desktopd.sock`
- definir permisos y grupo
- activar el servicio bajo demanda
- evitar que la GUI tenga que lanzar procesos privilegiados por sí sola

Configuración recomendada:

- `SocketMode=0660`
- `SocketUser=root`
- `SocketGroup=vpn-desktop`

---

### `packaging/linux/install.sh`

Script de instalación del sistema.

Responsabilidades previstas:

- instalar binarios
- crear el grupo `vpn-desktop`
- copiar units de `systemd`
- activar el socket con `systemctl enable --now`
- instalar el `.desktop`
- instalar el icono SVG de la aplicación si está disponible
- mostrar si hace falta relogin del usuario

---

### `packaging/linux/uninstall.sh`

Script de desinstalación.

Responsabilidades previstas:

- parar el servicio/socket
- eliminar units instaladas
- eliminar binarios instalados
- borrar `.desktop`
- opcionalmente dejar o limpiar configuración de usuario

---

### `packaging/linux/vpn-desktop.desktop`

Entrada para menú/aplicaciones del entorno gráfico.

Responsabilidades:

- aparecer en el launcher
- definir nombre, icono y categoría
- lanzar la GUI correctamente

Ejemplo esperado:

- `Name=VPN Desktop`
- `Exec=/usr/bin/vpn-desktop`
- `Icon=vpn-desktop`
- `Type=Application`
- `Categories=Network;Utility;`

---

## Flujo de instalación esperado

### 1. Instalar la app como administrador
Durante instalación se hace una única elevación de privilegios para copiar archivos del sistema.

### 2. Registrar helper y socket
Se instalan:

- `vpn-desktopd.service`
- `vpn-desktopd.socket`

### 3. Configurar permisos
Se usa un grupo del sistema, por ejemplo:

- `vpn-desktop`

y se añade el usuario autorizado a ese grupo.

### 4. Uso diario sin prompts constantes
Después de eso, la GUI debería poder hablar con el helper por el socket Unix sin pedir la contraseña en cada conexión.

### 5. Integración visual en el escritorio
El instalador también puede dejar registrado el icono de la app para que el entorno gráfico la muestre correctamente en el launcher, el menú de aplicaciones y el conmutador de ventanas.

---

## Ventajas de este enfoque

- más profesional para Linux
- más seguro que usar `sudo` repetidamente desde la interfaz
- mejor separación entre frontend y backend
- empaquetado claro
- integración real con `systemd`
- mejor UX

---

## Notas

- La GUI debe seguir corriendo como usuario normal.
- El helper root debe ser pequeño, controlado y limitado a operaciones de VPN.
- Las contraseñas deben seguir en el keyring del usuario, no en texto plano.
- El directorio `packaging` es la base para un futuro `.deb` o instalador más completo.
- El icono de la aplicación puede instalarse como SVG en `hicolor/scalable/apps/` para integrarse con el escritorio Linux.

---

## Estado actual de la migración

A día de hoy, la base principal ya está planteada y parcialmente implementada:

1. Existe un helper dedicado `vpn-desktopd`
2. Existe una capa IPC basada en socket Unix
3. La GUI ya está orientada al uso del helper en vez de lanzar OpenVPN directamente
4. Hay scripts de instalación y desinstalación para Linux
5. Hay units de `systemd` para servicio y socket
6. El `.desktop` ya puede referenciar un icono por nombre
7. El empaquetado Linux ya contempla un icono SVG real en `packaging/linux/icons/vpn-desktop.svg`

## Icono de la aplicación

La app ya dispone de un icono SVG real para integración de escritorio Linux:

- Archivo previsto: `packaging/linux/icons/vpn-desktop.svg`
- Nombre de icono usado por el `.desktop`: `vpn-desktop`

Este icono está pensado para instalarse en una ruta como:

- `/usr/share/icons/hicolor/scalable/apps/vpn-desktop.svg`

Así, el archivo `.desktop` puede resolverlo mediante:

- `Icon=vpn-desktop`

## Helper root e instalación Linux

La arquitectura actual recomendada para Linux queda así:

- `vpn-desktop`: interfaz gráfica sin privilegios
- `vpn-desktopd`: helper privilegiado
- `/run/vpn-desktopd.sock`: socket Unix para IPC
- `vpn-desktopd.socket`: activación por `systemd`
- `vpn-desktopd.service`: ejecución controlada del helper root

Esto permite que el uso diario no dependa de lanzar `sudo` o `pkexec` desde la GUI en cada conexión.

## Próximos pasos recomendados

1. Añadir documentación de empaquetado `.deb`
2. Documentar de forma más precisa la instalación en Debian/Ubuntu
3. Añadir política opcional de `polkit` si se decide usar además de `systemd`
4. Refinar la detección de estado del helper desde la UI
5. Completar distribución de iconos adicionales si más adelante se quiere PNG además de SVG
