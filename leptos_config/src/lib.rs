#![forbid(unsafe_code)]

pub mod errors;

use crate::errors::LeptosConfigError;
use config::{Case, Config, File, FileFormat};
use regex::Regex;
use std::{
    env::VarError, fs, net::SocketAddr, path::Path, str::FromStr, sync::Arc,
};
use typed_builder::TypedBuilder;

/// A Struct to allow us to parse LeptosOptions from the file. Not really needed, most interactions should
/// occur with LeptosOptions
#[derive(Clone, Debug, serde::Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct ConfFile {
    pub leptos_options: LeptosOptions,
}

/// This struct serves as a convenient place to store details used for configuring Leptos.
/// It's used in our actix and axum integrations to generate the
/// correct path for WASM, JS, and Websockets, as well as other configuration tasks.
/// It shares keys with cargo-leptos, to allow for easy interoperability
#[derive(TypedBuilder, Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LeptosOptions {
    /// The name of the WASM and JS files generated by wasm-bindgen. Defaults to the crate name with underscores instead of dashes
    #[builder(setter(into), default=default_output_name())]
    pub output_name: Arc<str>,
    /// The path of the all the files generated by cargo-leptos. This defaults to '.' for convenience when integrating with other
    /// tools.
    #[builder(setter(into), default=default_site_root())]
    #[serde(default = "default_site_root")]
    pub site_root: Arc<str>,
    /// The path of the WASM and JS files generated by wasm-bindgen from the root of your app
    /// By default, wasm-bindgen puts them in `pkg`.
    #[builder(setter(into), default=default_site_pkg_dir())]
    #[serde(default = "default_site_pkg_dir")]
    pub site_pkg_dir: Arc<str>,
    /// Used to configure the running environment of Leptos. Can be used to load dev constants and keys v prod, or change
    /// things based on the deployment environment
    /// I recommend passing in the result of `env::var("LEPTOS_ENV")`
    #[builder(setter(into), default=default_env())]
    #[serde(default = "default_env")]
    pub env: Env,
    /// Provides a way to control the address leptos is served from.
    /// Using an env variable here would allow you to run the same code in dev and prod
    /// Defaults to `127.0.0.1:3000`
    #[builder(setter(into), default=default_site_addr())]
    #[serde(default = "default_site_addr")]
    pub site_addr: SocketAddr,
    /// The port the Websocket watcher listens on. Should match the `reload_port` in cargo-leptos(if using).
    /// Defaults to `3001`
    #[builder(default = default_reload_port())]
    #[serde(default = "default_reload_port")]
    pub reload_port: u32,
    /// The port the Websocket watcher listens on when on the client, e.g., when behind a reverse proxy.
    /// Defaults to match reload_port
    #[builder(default)]
    #[serde(default)]
    pub reload_external_port: Option<u32>,
    /// The protocol the Websocket watcher uses on the client: `ws` in most cases, `wss` when behind a reverse https proxy.
    /// Defaults to `ws`
    #[builder(default)]
    #[serde(default)]
    pub reload_ws_protocol: ReloadWSProtocol,
    /// The path of a custom 404 Not Found page to display when statically serving content, defaults to `site_root/404.html`
    #[builder(default = default_not_found_path())]
    #[serde(default = "default_not_found_path")]
    pub not_found_path: Arc<str>,
    /// The file name of the hash text file generated by cargo-leptos. Defaults to `hash.txt`.
    #[builder(default = default_hash_file_name())]
    #[serde(default = "default_hash_file_name")]
    pub hash_file: Arc<str>,
    /// If true, hashes will be generated for all files in the site_root and added to their file names.
    /// Defaults to `true`.
    #[builder(default = default_hash_files())]
    #[serde(default = "default_hash_files")]
    pub hash_files: bool,
}

impl LeptosOptions {
    fn try_from_env() -> Result<Self, LeptosConfigError> {
        let output_name = env_w_default(
            "LEPTOS_OUTPUT_NAME",
            std::option_env!("LEPTOS_OUTPUT_NAME",).unwrap_or_default(),
        )?;
        if output_name.is_empty() {
            eprintln!(
                "It looks like you're trying to compile Leptos without the \
                 LEPTOS_OUTPUT_NAME environment variable being set. There are \
                 two options\n 1. cargo-leptos is not being used, but \
                 get_configuration() is being passed None. This needs to be \
                 changed to Some(\"Cargo.toml\")\n 2. You are compiling \
                 Leptos without LEPTOS_OUTPUT_NAME being set with \
                 cargo-leptos. This shouldn't be possible!"
            );
        }
        Ok(LeptosOptions {
            output_name: output_name.into(),
            site_root: env_w_default("LEPTOS_SITE_ROOT", "target/site")?.into(),
            site_pkg_dir: env_w_default("LEPTOS_SITE_PKG_DIR", "pkg")?.into(),
            env: env_from_str(env_w_default("LEPTOS_ENV", "DEV")?.as_str())?,
            site_addr: env_w_default("LEPTOS_SITE_ADDR", "127.0.0.1:3000")?
                .parse()?,
            reload_port: env_w_default("LEPTOS_RELOAD_PORT", "3001")?
                .parse()?,
            reload_external_port: match env_wo_default(
                "LEPTOS_RELOAD_EXTERNAL_PORT",
            )? {
                Some(val) => Some(val.parse()?),
                None => None,
            },
            reload_ws_protocol: ws_from_str(
                env_w_default("LEPTOS_RELOAD_WS_PROTOCOL", "ws")?.as_str(),
            )?,
            not_found_path: env_w_default("LEPTOS_NOT_FOUND_PATH", "/404")?
                .into(),
            hash_file: env_w_default("LEPTOS_HASH_FILE_NAME", "hash.txt")?
                .into(),
            hash_files: env_w_default("LEPTOS_HASH_FILES", "false")?.parse()?,
        })
    }
}

impl Default for LeptosOptions {
    fn default() -> Self {
        LeptosOptions::builder().build()
    }
}

fn default_output_name() -> Arc<str> {
    env!("CARGO_CRATE_NAME").replace('-', "_").into()
}

fn default_site_root() -> Arc<str> {
    ".".into()
}

fn default_site_pkg_dir() -> Arc<str> {
    "pkg".into()
}

fn default_env() -> Env {
    Env::DEV
}

fn default_site_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 3000))
}

fn default_reload_port() -> u32 {
    3001
}

fn default_not_found_path() -> Arc<str> {
    "/404".into()
}

fn default_hash_file_name() -> Arc<str> {
    "hash.txt".into()
}

fn default_hash_files() -> bool {
    false
}

fn env_wo_default(key: &str) -> Result<Option<String>, LeptosConfigError> {
    match std::env::var(key) {
        Ok(val) => Ok(Some(val)),
        Err(VarError::NotPresent) => Ok(None),
        Err(e) => Err(LeptosConfigError::EnvVarError(format!("{key}: {e}"))),
    }
}
fn env_w_default(
    key: &str,
    default: &str,
) -> Result<String, LeptosConfigError> {
    match std::env::var(key) {
        Ok(val) => Ok(val),
        Err(VarError::NotPresent) => Ok(default.to_string()),
        Err(e) => Err(LeptosConfigError::EnvVarError(format!("{key}: {e}"))),
    }
}

/// An enum that can be used to define the environment Leptos is running in.
/// Setting this to the `PROD` variant will not include the WebSocket code for `cargo-leptos` watch mode.
/// Defaults to `DEV`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum Env {
    PROD,
    DEV,
}

impl Default for Env {
    fn default() -> Self {
        Self::DEV
    }
}

fn env_from_str(input: &str) -> Result<Env, LeptosConfigError> {
    let sanitized = input.to_lowercase();
    match sanitized.as_ref() {
        "dev" | "development" => Ok(Env::DEV),
        "prod" | "production" => Ok(Env::PROD),
        _ => Err(LeptosConfigError::EnvVarError(format!(
            "{input} is not a supported environment. Use either `dev` or \
             `production`.",
        ))),
    }
}

impl FromStr for Env {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        env_from_str(input).or_else(|_| Ok(Self::default()))
    }
}

impl From<&str> for Env {
    fn from(str: &str) -> Self {
        env_from_str(str).unwrap_or_else(|err| panic!("{}", err))
    }
}

impl From<&Result<String, VarError>> for Env {
    fn from(input: &Result<String, VarError>) -> Self {
        match input {
            Ok(str) => {
                env_from_str(str).unwrap_or_else(|err| panic!("{}", err))
            }
            Err(_) => Self::default(),
        }
    }
}

impl TryFrom<String> for Env {
    type Error = LeptosConfigError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        env_from_str(s.as_str())
    }
}

/// An enum that can be used to define the websocket protocol Leptos uses for hotreloading
/// Defaults to `ws`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum ReloadWSProtocol {
    WS,
    WSS,
}

impl Default for ReloadWSProtocol {
    fn default() -> Self {
        Self::WS
    }
}

fn ws_from_str(input: &str) -> Result<ReloadWSProtocol, LeptosConfigError> {
    let sanitized = input.to_lowercase();
    match sanitized.as_ref() {
        "ws" | "WS" => Ok(ReloadWSProtocol::WS),
        "wss" | "WSS" => Ok(ReloadWSProtocol::WSS),
        _ => Err(LeptosConfigError::EnvVarError(format!(
            "{input} is not a supported websocket protocol. Use only `ws` or \
             `wss`.",
        ))),
    }
}

impl FromStr for ReloadWSProtocol {
    type Err = ();
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        ws_from_str(input).or_else(|_| Ok(Self::default()))
    }
}

impl From<&str> for ReloadWSProtocol {
    fn from(str: &str) -> Self {
        ws_from_str(str).unwrap_or_else(|err| panic!("{}", err))
    }
}

impl From<&Result<String, VarError>> for ReloadWSProtocol {
    fn from(input: &Result<String, VarError>) -> Self {
        match input {
            Ok(str) => ws_from_str(str).unwrap_or_else(|err| panic!("{}", err)),
            Err(_) => Self::default(),
        }
    }
}

impl TryFrom<String> for ReloadWSProtocol {
    type Error = LeptosConfigError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        ws_from_str(s.as_str())
    }
}

/// Loads [LeptosOptions] from a Cargo.toml text content with layered overrides.
/// If an env var is specified, like `LEPTOS_ENV`, it will override a setting in the file.
pub fn get_config_from_str(
    text: &str,
) -> Result<LeptosOptions, LeptosConfigError> {
    let re: Regex = Regex::new(r"(?m)^\[package.metadata.leptos\]").unwrap();
    let re_workspace: Regex =
        Regex::new(r"(?m)^\[\[workspace.metadata.leptos\]\]").unwrap();

    let metadata_name;
    let start;
    match re.find(text) {
        Some(found) => {
            metadata_name = "[package.metadata.leptos]";
            start = found.start();
        }
        None => match re_workspace.find(text) {
            Some(found) => {
                metadata_name = "[[workspace.metadata.leptos]]";
                start = found.start();
            }
            None => return Err(LeptosConfigError::ConfigSectionNotFound),
        },
    };

    // so that serde error messages have right line number
    let newlines = text[..start].matches('\n').count();
    let input = "\n".repeat(newlines) + &text[start..];
    // so the settings will be interpreted as root level settings
    let toml = input.replace(metadata_name, "");
    let settings = Config::builder()
        // Read the "default" configuration file
        .add_source(File::from_str(&toml, FileFormat::Toml))
        // Layer on the environment-specific values.
        // Add in settings from environment variables (with a prefix of LEPTOS)
        // E.g. `LEPTOS_RELOAD_PORT=5001 would set `LeptosOptions.reload_port`
        .add_source(
            config::Environment::with_prefix("LEPTOS")
                .convert_case(Case::Kebab),
        )
        .build()?;

    settings
        .try_deserialize()
        .map_err(|e| LeptosConfigError::ConfigError(e.to_string()))
}

/// Loads [LeptosOptions] from a Cargo.toml with layered overrides. If an env var is specified, like `LEPTOS_ENV`,
/// it will override a setting in the file. It takes in an optional path to a Cargo.toml file. If None is provided,
/// you'll need to set the options as environment variables or rely on the defaults. This is the preferred
/// approach for cargo-leptos. If Some("./Cargo.toml") is provided, Leptos will read in the settings itself. This
/// option currently does not allow dashes in file or folder names, as all dashes become underscores
pub fn get_configuration(
    path: Option<&str>,
) -> Result<ConfFile, LeptosConfigError> {
    if let Some(path) = path {
        get_config_from_file(path)
    } else {
        get_config_from_env()
    }
}

/// Loads [LeptosOptions] from a Cargo.toml with layered overrides. Leptos will read in the settings itself. This
/// option currently does not allow dashes in file or folder names, as all dashes become underscores
pub fn get_config_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<ConfFile, LeptosConfigError> {
    let text = fs::read_to_string(path)
        .map_err(|_| LeptosConfigError::ConfigNotFound)?;
    let leptos_options = get_config_from_str(&text)?;
    Ok(ConfFile { leptos_options })
}

/// Loads [LeptosOptions] from environment variables or rely on the defaults
pub fn get_config_from_env() -> Result<ConfFile, LeptosConfigError> {
    Ok(ConfFile {
        leptos_options: LeptosOptions::try_from_env()?,
    })
}

#[path = "tests.rs"]
#[cfg(test)]
mod tests;
