# Guía de prueba final en Linux

Esta guía sirve para validar el funcionamiento real de `vpn-desktop` con la arquitectura actual:

- GUI sin privilegios
- helper root `vpn-desktopd`
- socket Unix en `/run/vpn-desktopd.sock`
- activación mediante `systemd`

El objetivo es comprobar que:

1. la aplicación instala correctamente sus componentes
2. el helper puede activarse
3. el socket queda accesible
4. la GUI arranca
5. una conexión OpenVPN puede iniciarse desde la app

---

## 1. Requisitos previos

Antes de probar, asegúrate de tener:

- Linux con `systemd`
- `openvpn` instalado
- permisos de administrador
- un usuario normal con sesión gráfica
- al menos un fichero `.ovpn` válido para pruebas

Comprobaciones rápidas:

```/dev/null/check-openvpn.txt#L1-2
which openvpn
openvpn --version
```

```/dev/null/check-systemd.txt#L1-1
systemctl --version
```

---

## 2. Opción A: probar instalando el paquete `.deb`

Si ya has generado el paquete, instala con:

```/dev/null/install-deb.txt#L1-1
sudo dpkg -i target/deb-build/vpn-desktop_0.1.0-1_amd64.deb
```

Si `dpkg` informa de dependencias pendientes, corrige con:

```/dev/null/fix-deps.txt#L1-1
sudo apt-get install -f
```

---

## 3. Opción B: probar con el instalador manual

Si prefieres instalar desde el árbol del proyecto:

```/dev/null/install-manual.txt#L1-2
cd vpn-desktop
sudo ./packaging/linux/install.sh
```

---

## 4. Verificar que el helper quedó instalado

Comprueba que existen los binarios esperados:

```/dev/null/check-binaries.txt#L1-2
ls -l /usr/bin/vpn-desktop
ls -l /usr/libexec/vpn-desktopd
```

Comprueba también los assets del sistema:

```/dev/null/check-assets.txt#L1-4
ls -l /usr/share/applications/vpn-desktop.desktop
ls -l /usr/share/icons/hicolor/scalable/apps/vpn-desktop.svg
ls -l /lib/systemd/system/vpn-desktopd.service
ls -l /lib/systemd/system/vpn-desktopd.socket
```

Si en instalación manual se usó `/etc/systemd/system`, revisa ahí en vez de `/lib/systemd/system`.

---

## 5. Verificar `systemd`

Recarga units si hace falta:

```/dev/null/reload-systemd.txt#L1-1
sudo systemctl daemon-reload
```

Comprueba el estado del socket:

```/dev/null/socket-status.txt#L1-1
systemctl status vpn-desktopd.socket
```

Comprueba el estado del servicio:

```/dev/null/service-status.txt#L1-1
systemctl status vpn-desktopd.service
```

Estados esperables:

- `vpn-desktopd.socket` debería aparecer como `active (listening)` o equivalente
- `vpn-desktopd.service` puede aparecer inactivo hasta que una GUI se conecte, porque el socket puede activarlo bajo demanda

Si quieres confirmar que el socket está habilitado:

```/dev/null/is-enabled.txt#L1-2
systemctl is-enabled vpn-desktopd.socket
systemctl is-enabled vpn-desktopd.service
```

---

## 6. Verificar el socket runtime

Comprueba que el socket existe:

```/dev/null/check-runtime-socket.txt#L1-1
ls -l /run/vpn-desktopd.sock
```

Lo esperado es algo parecido a:

- propietario root
- grupo `vpn-desktop`
- permisos `srw-rw----` o equivalentes a `0660`

Si no existe:

- revisa `systemctl status vpn-desktopd.socket`
- revisa si la instalación terminó correctamente
- revisa si el sistema realmente usa la unit instalada

---

## 7. Verificar grupo y permisos del usuario

Comprueba que el grupo existe:

```/dev/null/check-group.txt#L1-1
getent group vpn-desktop
```

Comprueba si tu usuario pertenece al grupo:

```/dev/null/check-user-group.txt#L1-1
id -nG
```

o:

```/dev/null/check-user-group-explicit.txt#L1-1
id -nG <tu_usuario>
```

Si no aparece `vpn-desktop`, añádelo:

```/dev/null/add-user-group.txt#L1-1
sudo usermod -a -G vpn-desktop <tu_usuario>
```

Después de añadirlo, normalmente debes:

- cerrar sesión
- volver a iniciar sesión

Si no haces eso, la GUI puede seguir sin permisos sobre el socket aunque el grupo ya exista.

---

## 8. Abrir la aplicación

Puedes abrirla desde el menú de aplicaciones o desde terminal:

```/dev/null/run-gui.txt#L1-1
/usr/bin/vpn-desktop
```

Comprobaciones mínimas al arrancar:

- la ventana abre sin cerrarse inmediatamente
- aparecen los tabs esperados:
  - `Conectar`
  - `Perfiles`
  - `Proxies`
- no se muestra un error inmediato de helper si todo está bien instalado

---

## 9. Probar un perfil OpenVPN real

### Crear/importar perfil
Dentro de la app:

1. ve a `Perfiles`
2. crea un perfil
3. selecciona tu fichero `.ovpn`
4. introduce usuario y contraseña
5. guarda

### Opcional: configurar proxy
Si tu VPN requiere proxy:

1. ve a `Proxies`
2. crea el proxy
3. configura host, puerto y autenticación
4. guarda

### Probar conexión
En `Conectar`:

1. selecciona perfil
2. selecciona proxy si aplica
3. pulsa `Conectar`

---

## 10. Qué deberías observar al conectar

Señales de que todo va bien:

- la app no pide contraseña root en cada conexión
- aparecen logs del helper o de OpenVPN
- el estado pasa por `Connecting`
- si la conexión completa, en logs debería aparecer algo similar a:
  - `Initialization Sequence Completed`

También puedes revisar si el servicio fue activado:

```/dev/null/check-activation.txt#L1-1
systemctl status vpn-desktopd.service
```

---

## 11. Si algo falla: diagnóstico rápido

### Caso A: la GUI arranca pero dice que no encuentra el helper
Posibles causas:

- `vpn-desktopd.socket` no está activo
- el socket `/run/vpn-desktopd.sock` no existe
- instalación incompleta

Comprueba:

```/dev/null/debug-missing-helper.txt#L1-3
systemctl status vpn-desktopd.socket
systemctl status vpn-desktopd.service
ls -l /run/vpn-desktopd.sock
```

---

### Caso B: error de permisos al usar el helper
Posibles causas:

- tu usuario no está en `vpn-desktop`
- aún no has reiniciado sesión después de añadirlo al grupo

Comprueba:

```/dev/null/debug-permissions.txt#L1-2
id -nG
ls -l /run/vpn-desktopd.sock
```

---

### Caso C: el helper arranca pero falla al conectar VPN
Posibles causas:

- `openvpn` no está instalado
- el `.ovpn` no es válido
- credenciales incorrectas
- falta `update-resolv-conf` en perfiles que lo necesiten

Comprueba:

```/dev/null/debug-openvpn.txt#L1-2
which openvpn
openvpn --version
```

Si tu perfil depende de `update-resolv-conf`:

```/dev/null/debug-resolv-conf.txt#L1-1
ls -l /etc/openvpn/update-resolv-conf
```

---

### Caso D: el servicio arranca y se cae
Revisa logs de `systemd`:

```/dev/null/debug-journal.txt#L1-2
journalctl -u vpn-desktopd.service -n 100 --no-pager
journalctl -u vpn-desktopd.socket -n 100 --no-pager
```

---

## 12. Prueba mínima de aceptación

Puedes considerar la prueba como satisfactoria si cumples esto:

- `vpn-desktop` abre correctamente
- `vpn-desktopd.socket` está activo
- `/run/vpn-desktopd.sock` existe
- tu usuario pertenece al grupo `vpn-desktop`
- la app puede iniciar una conexión sin pedir root cada vez
- OpenVPN arranca desde el helper
- los logs muestran actividad real de conexión

---

## 13. Prueba de desconexión

Con una VPN ya activa:

1. vuelve a la pestaña `Conectar`
2. pulsa `Desconectar`

Lo esperado:

- el helper envía la orden de parada
- la UI vuelve a estado desconectado
- no quedan errores extraños ni procesos colgados

Opcionalmente puedes comprobar si sigue habiendo un proceso `openvpn`:

```/dev/null/check-openvpn-process.txt#L1-1
ps aux | grep openvpn
```

---

## 14. Desinstalación de prueba

Si quieres verificar también el flujo de desinstalación:

### Si instalaste con `.deb`
```/dev/null/remove-deb.txt#L1-1
sudo dpkg -r vpn-desktop
```

### Si instalaste con instalador manual
```/dev/null/remove-manual.txt#L1-2
cd vpn-desktop
sudo ./packaging/linux/uninstall.sh
```

Luego comprueba que se han eliminado:

- binarios
- `.desktop`
- iconos
- units instaladas
- socket runtime

---

## 15. Resultado esperado final

El resultado ideal de esta prueba completa es:

- app instalada correctamente
- helper integrado con `systemd`
- socket operativo
- permisos correctos del usuario
- OpenVPN ejecutado por el helper
- conexión iniciada desde la GUI
- sin prompts de privilegios repetidos en el uso diario

---