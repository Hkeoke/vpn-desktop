use anyhow::{Context, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

// ─── Método de autenticación del proxy ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum ProxyAuthMethod {
    #[default]
    None,
    Basic,
    Ntlm,
}

impl ProxyAuthMethod {
    /// Valor que se pasa directamente a OpenVPN (--http-proxy ... <method>)
    pub fn as_openvpn_arg(&self) -> &'static str {
        match self {
            ProxyAuthMethod::None => "none",
            ProxyAuthMethod::Basic => "basic",
            ProxyAuthMethod::Ntlm => "ntlm",
        }
    }

    /// Nombre legible para la UI
    pub fn display_name(&self) -> &'static str {
        match self {
            ProxyAuthMethod::None => "Sin autenticación",
            ProxyAuthMethod::Basic => "Basic",
            ProxyAuthMethod::Ntlm => "NTLM",
        }
    }

    /// Requiere un fichero de credenciales
    pub fn needs_auth_file(&self) -> bool {
        !matches!(self, ProxyAuthMethod::None)
    }

    pub fn all() -> &'static [ProxyAuthMethod] {
        &[
            ProxyAuthMethod::None,
            ProxyAuthMethod::Basic,
            ProxyAuthMethod::Ntlm,
        ]
    }
}

// ─── Perfil VPN ──────────────────────────────────────────────────────────────

/// Un perfil VPN agrupa el fichero de configuración OpenVPN (.ovpn) y sus
/// credenciales de acceso. Un perfil es independiente del proxy que se use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VpnProfile {
    /// Identificador único generado automáticamente
    pub id: String,
    /// Nombre visible en la UI
    pub name: String,
    /// Ruta absoluta al fichero .ovpn
    pub config_file: String,
    /// Usuario de la VPN.
    /// La aplicación generará un fichero temporal para pasarlo a OpenVPN mediante --auth-user-pass.
    pub username: String,
    /// Contraseña de la VPN.
    /// Se carga y guarda en el keyring del sistema, no en config.toml.
    #[serde(skip, default)]
    pub password: String,
    /// Si está activo, se añaden:
    ///   --script-security 2
    ///   --up    /etc/openvpn/update-resolv-conf
    ///   --down  /etc/openvpn/update-resolv-conf
    /// Necesario para que el DNS funcione correctamente en Linux.
    pub use_update_resolv_conf: bool,
}

impl Default for VpnProfile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            config_file: String::new(),
            username: String::new(),
            password: String::new(),
            use_update_resolv_conf: true,
        }
    }
}

impl VpnProfile {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validación mínima antes de intentar conectar
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("El nombre del perfil no puede estar vacío.".into());
        }
        if self.config_file.trim().is_empty() {
            return Err("Debes seleccionar un fichero .ovpn.".into());
        }
        if !std::path::Path::new(&self.config_file).exists() {
            return Err(format!("El fichero .ovpn no existe:\n{}", self.config_file));
        }
        if self.username.trim().is_empty() {
            return Err("El usuario de la VPN no puede estar vacío.".into());
        }
        if self.password.trim().is_empty() {
            return Err("La contraseña de la VPN no puede estar vacía.".into());
        }
        Ok(())
    }
}

// ─── Configuración de proxy ──────────────────────────────────────────────────

/// Una configuración de proxy define el servidor HTTP/HTTPS que OpenVPN usará
/// para tunelizar la conexión. Se puede combinar con cualquier perfil VPN.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    /// Identificador único generado automáticamente
    pub id: String,
    /// Nombre visible en la UI
    pub name: String,
    /// Dirección IP o hostname del servidor proxy
    pub host: String,
    /// Puerto del servidor proxy
    pub port: u16,
    /// Método de autenticación
    pub auth_method: ProxyAuthMethod,
    /// Usuario del proxy.
    /// Ignorado si auth_method es None.
    pub username: String,
    /// Contraseña del proxy.
    /// Se carga y guarda en el keyring del sistema, no en config.toml.
    #[serde(skip, default)]
    pub password: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: String::new(),
            host: String::new(),
            port: 3128,
            auth_method: ProxyAuthMethod::None,
            username: String::new(),
            password: String::new(),
        }
    }
}

impl ProxyConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validación mínima
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("El nombre del proxy no puede estar vacío.".into());
        }
        if self.host.trim().is_empty() {
            return Err("La dirección del servidor proxy no puede estar vacía.".into());
        }
        if self.port == 0 {
            return Err("El puerto debe ser mayor que 0.".into());
        }
        if self.auth_method.needs_auth_file() {
            if self.username.trim().is_empty() {
                return Err(format!(
                    "El método '{}' requiere un usuario de proxy.",
                    self.auth_method.display_name()
                ));
            }
            if self.password.trim().is_empty() {
                return Err(format!(
                    "El método '{}' requiere una contraseña de proxy.",
                    self.auth_method.display_name()
                ));
            }
        }
        Ok(())
    }
}

// ─── Ajustes de la aplicación ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {}

// ─── Configuración completa de la aplicación ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub settings: AppSettings,

    /// ID del perfil VPN seleccionado actualmente por el usuario.
    /// Se persiste para que la selección sobreviva reinicios de la app.
    #[serde(default)]
    pub selected_profile_id: Option<String>,

    /// ID del proxy seleccionado actualmente por el usuario.
    /// Se persiste para que la selección sobreviva reinicios de la app.
    #[serde(default)]
    pub selected_proxy_id: Option<String>,

    #[serde(default)]
    pub vpn_profiles: Vec<VpnProfile>,

    #[serde(default)]
    pub proxy_configs: Vec<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SecretsFile {
    #[serde(default)]
    vpn_passwords: HashMap<String, String>,
    #[serde(default)]
    proxy_passwords: HashMap<String, String>,
}

impl AppConfig {
    const KEYRING_SERVICE: &'static str = "vpn-desktop";

    // ── Persistencia ──────────────────────────────────────────────────────────

    /// Carga la configuración desde `~/.config/vpn-desktop/config.toml`.
    /// Si el fichero no existe o está corrupto, devuelve la configuración por defecto.
    pub fn load() -> Self {
        let path = Self::config_path();

        let mut config = match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<AppConfig>(&content) {
                Ok(cfg) => cfg,
                Err(e) => {
                    eprintln!(
                        "[vpn-desktop] Advertencia: no se pudo parsear la configuración \
                         ({}). Usando valores por defecto.",
                        e
                    );
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        };

        config.load_secrets_from_keyring();
        config.load_secrets_from_fallback_file();
        config
    }

    /// Guarda la configuración en disco.
    pub fn save(&self) -> Result<()> {
        self.store_secrets_in_keyring();
        self.store_secrets_in_fallback_file()?;

        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("No se pudo crear el directorio '{}'", dir.display()))?;

        let content =
            toml::to_string_pretty(self).context("No se pudo serializar la configuración")?;

        let path = Self::config_path();
        std::fs::write(&path, content)
            .with_context(|| format!("No se pudo escribir '{}'", path.display()))?;

        Ok(())
    }

    // ── Directorios ───────────────────────────────────────────────────────────

    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from(std::env::var("HOME").unwrap_or_default()))
            .join("vpn-desktop")
    }

    pub fn config_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    pub fn profiles_dir() -> PathBuf {
        Self::config_dir().join("profiles")
    }

    fn secrets_path() -> PathBuf {
        Self::config_dir().join("secrets.toml")
    }

    pub fn managed_profile_config_path(profile_id: &str, source_path: impl AsRef<Path>) -> PathBuf {
        let source_path = source_path.as_ref();
        let extension = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| !ext.trim().is_empty())
            .unwrap_or("ovpn");

        Self::profiles_dir().join(format!("{}.{}", profile_id, extension))
    }

    pub fn import_profile_config(
        profile_id: &str,
        source_path: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        let source_path = source_path.as_ref();
        let target_path = Self::managed_profile_config_path(profile_id, source_path);

        std::fs::create_dir_all(Self::profiles_dir()).with_context(|| {
            format!(
                "No se pudo crear el directorio de perfiles '{}'",
                Self::profiles_dir().display()
            )
        })?;

        if source_path == target_path {
            return Ok(target_path);
        }

        std::fs::copy(source_path, &target_path).with_context(|| {
            format!(
                "No se pudo importar el fichero OpenVPN '{}' a '{}'",
                source_path.display(),
                target_path.display()
            )
        })?;

        Ok(target_path)
    }

    pub fn is_managed_profile_config(config_file: &str) -> bool {
        let config_path = Path::new(config_file);
        let managed_dir = Self::profiles_dir();

        config_path.starts_with(&managed_dir)
    }

    pub fn delete_managed_profile_config(config_file: &str) -> Result<()> {
        if config_file.trim().is_empty() || !Self::is_managed_profile_config(config_file) {
            return Ok(());
        }

        let config_path = Path::new(config_file);

        if !config_path.exists() {
            return Ok(());
        }

        std::fs::remove_file(config_path).with_context(|| {
            format!(
                "No se pudo eliminar el fichero OpenVPN gestionado '{}'",
                config_path.display()
            )
        })?;

        Ok(())
    }

    pub fn cleanup_profile_assets(profile: &VpnProfile) -> Result<()> {
        Self::delete_managed_profile_config(&profile.config_file)?;
        Self::delete_vpn_password(profile)
    }

    fn vpn_password_key(profile_id: &str) -> String {
        format!("vpn-profile:{}", profile_id)
    }

    fn proxy_password_key(proxy_id: &str) -> String {
        format!("proxy-config:{}", proxy_id)
    }

    fn keyring_entry(key: &str) -> Result<Entry> {
        Entry::new(Self::KEYRING_SERVICE, key)
            .map_err(|e| anyhow::anyhow!("No se pudo abrir el keyring para '{}': {}", key, e))
    }

    fn load_secrets_from_keyring(&mut self) {
        for profile in &mut self.vpn_profiles {
            let key = Self::vpn_password_key(&profile.id);
            match Self::keyring_entry(&key) {
                Ok(entry) => match entry.get_password() {
                    Ok(password) => {
                        profile.password = password;
                    }
                    Err(keyring::Error::NoEntry) => {}
                    Err(err) => {
                        eprintln!(
                            "[vpn-desktop] Advertencia: no se pudo leer la contraseña VPN '{}' del keyring: {}",
                            profile.name, err
                        );
                    }
                },
                Err(err) => {
                    eprintln!(
                        "[vpn-desktop] Advertencia: no se pudo abrir el keyring para el perfil VPN '{}' ({}).",
                        profile.name, err
                    );
                }
            }
        }

        for proxy in &mut self.proxy_configs {
            let key = Self::proxy_password_key(&proxy.id);
            match Self::keyring_entry(&key) {
                Ok(entry) => match entry.get_password() {
                    Ok(password) => {
                        proxy.password = password;
                    }
                    Err(keyring::Error::NoEntry) => {}
                    Err(err) => {
                        eprintln!(
                            "[vpn-desktop] Advertencia: no se pudo leer la contraseña del proxy '{}' del keyring: {}",
                            proxy.name, err
                        );
                    }
                },
                Err(err) => {
                    eprintln!(
                        "[vpn-desktop] Advertencia: no se pudo abrir el keyring para el proxy '{}' ({}).",
                        proxy.name, err
                    );
                }
            }
        }
    }

    fn load_secrets_from_fallback_file(&mut self) {
        let path = Self::secrets_path();
        let secrets = match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str::<SecretsFile>(&content) {
                Ok(secrets) => secrets,
                Err(err) => {
                    eprintln!(
                        "[vpn-desktop] Advertencia: no se pudo parsear el fallback local de secretos '{}': {}",
                        path.display(),
                        err
                    );
                    return;
                }
            },
            Err(_) => return,
        };

        for profile in &mut self.vpn_profiles {
            if profile.password.trim().is_empty() {
                if let Some(password) = secrets.vpn_passwords.get(&profile.id) {
                    profile.password = password.clone();
                }
            }
        }

        for proxy in &mut self.proxy_configs {
            if proxy.password.trim().is_empty() {
                if let Some(password) = secrets.proxy_passwords.get(&proxy.id) {
                    proxy.password = password.clone();
                }
            }
        }
    }

    fn store_secrets_in_keyring(&self) {
        for profile in &self.vpn_profiles {
            if !profile.password.trim().is_empty() {
                if let Err(err) = Self::store_vpn_password(profile) {
                    eprintln!(
                        "[vpn-desktop] Advertencia: no se pudo guardar la contraseña VPN '{}' en keyring: {}",
                        profile.name, err
                    );
                }
            }
        }

        for proxy in &self.proxy_configs {
            if !proxy.password.trim().is_empty() {
                if let Err(err) = Self::store_proxy_password(proxy) {
                    eprintln!(
                        "[vpn-desktop] Advertencia: no se pudo guardar la contraseña del proxy '{}' en keyring: {}",
                        proxy.name, err
                    );
                }
            }
        }
    }

    fn store_secrets_in_fallback_file(&self) -> Result<()> {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("No se pudo crear el directorio '{}'", dir.display()))?;

        let mut secrets = SecretsFile::default();

        for profile in &self.vpn_profiles {
            if !profile.password.trim().is_empty() {
                secrets
                    .vpn_passwords
                    .insert(profile.id.clone(), profile.password.clone());
            }
        }

        for proxy in &self.proxy_configs {
            if !proxy.password.trim().is_empty() {
                secrets
                    .proxy_passwords
                    .insert(proxy.id.clone(), proxy.password.clone());
            }
        }

        let content = toml::to_string_pretty(&secrets)
            .context("No se pudo serializar el fallback local de secretos")?;

        let path = Self::secrets_path();
        std::fs::write(&path, content)
            .with_context(|| format!("No se pudo escribir '{}'", path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = std::fs::Permissions::from_mode(0o600);
            std::fs::set_permissions(&path, permissions).with_context(|| {
                format!(
                    "No se pudieron ajustar permisos seguros en '{}'",
                    path.display()
                )
            })?;
        }

        Ok(())
    }

    pub fn store_vpn_password(profile: &VpnProfile) -> Result<()> {
        let key = Self::vpn_password_key(&profile.id);
        let entry = Self::keyring_entry(&key)?;

        entry
            .set_password(&profile.password)
            .map_err(|e| anyhow::anyhow!("No se pudo guardar la contraseña VPN en keyring: {}", e))
    }

    pub fn store_proxy_password(proxy: &ProxyConfig) -> Result<()> {
        let key = Self::proxy_password_key(&proxy.id);
        let entry = Self::keyring_entry(&key)?;

        entry.set_password(&proxy.password).map_err(|e| {
            anyhow::anyhow!(
                "No se pudo guardar la contraseña del proxy en keyring: {}",
                e
            )
        })
    }

    pub fn delete_vpn_password(profile: &VpnProfile) -> Result<()> {
        let key = Self::vpn_password_key(&profile.id);
        let entry = Self::keyring_entry(&key)?;

        match entry.delete_credential() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(
                "No se pudo eliminar la contraseña VPN del keyring: {}",
                e
            )),
        }
    }

    pub fn delete_proxy_password(proxy: &ProxyConfig) -> Result<()> {
        let key = Self::proxy_password_key(&proxy.id);
        let entry = Self::keyring_entry(&key)?;

        match entry.delete_credential() {
            Ok(_) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!(
                "No se pudo eliminar la contraseña del proxy del keyring: {}",
                e
            )),
        }
    }

    // ── Búsqueda ──────────────────────────────────────────────────────────────

    pub fn find_profile(&self, id: &str) -> Option<&VpnProfile> {
        self.vpn_profiles.iter().find(|p| p.id == id)
    }

    pub fn find_proxy(&self, id: &str) -> Option<&ProxyConfig> {
        self.proxy_configs.iter().find(|p| p.id == id)
    }

    // ── Mutación ─────────────────────────────────────────────────────────────

    pub fn upsert_profile(&mut self, mut profile: VpnProfile) {
        if let Some(existing) = self.vpn_profiles.iter_mut().find(|p| p.id == profile.id) {
            if profile.password.trim().is_empty() {
                profile.password = existing.password.clone();
            }
            *existing = profile;
        } else {
            self.vpn_profiles.push(profile);
        }
    }

    pub fn remove_profile(&mut self, id: &str) {
        self.vpn_profiles.retain(|p| p.id != id);
    }

    pub fn upsert_proxy(&mut self, mut proxy: ProxyConfig) {
        if let Some(existing) = self.proxy_configs.iter_mut().find(|p| p.id == proxy.id) {
            if proxy.password.trim().is_empty() {
                proxy.password = existing.password.clone();
            }
            *existing = proxy;
        } else {
            self.proxy_configs.push(proxy);
        }
    }

    pub fn remove_proxy(&mut self, id: &str) {
        self.proxy_configs.retain(|p| p.id != id);
    }
}
