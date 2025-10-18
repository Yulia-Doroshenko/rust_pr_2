use clap::{ArgAction, Parser};
use figment::{
    providers::{Env, Serialized, Toml},
    Figment,
};
use figment::providers::Format; // додає Toml::file(...)
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::Path, process::ExitCode};

#[derive(Parser, Debug)]
#[command(
    name = "task_3_9",
    version,
    about = "Prints its configuration to STDOUT",
    disable_help_subcommand = true
)]
struct Cli {
    #[arg(short = 'd', long = "debug", action = ArgAction::SetTrue)]
    debug: bool,

    #[arg(
        short = 'c',
        long = "conf",
        value_name = "CONF",
        default_value = "config.toml",
        env = "CONF_FILE"
    )]
    conf: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    debug: bool,
    #[serde(default)]
    server: Server,
    #[serde(default)]
    db: Db,
    #[serde(default)]
    log: Log,
    #[serde(default)]
    background: Background,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Server {
    #[serde(default = "def_external_url")]
    external_url: String,
    #[serde(default = "def_http_port")]
    http_port: u16,
    #[serde(default = "def_grpc_port")]
    grpc_port: u16,
    #[serde(default = "def_healthz_port")]
    healthz_port: u16,
    #[serde(default = "def_metrics_port")]
    metrics_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Db {
    #[serde(default)]
    mysql: MySql,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MySql {
    #[serde(default = "def_mysql_host")]
    host: String,
    #[serde(default = "def_mysql_port")]
    port: u16,
    #[serde(default = "def_mysql_dating")]
    dating: String,
    #[serde(default = "def_mysql_user")]
    user: String,
    #[serde(default)]
    pass: String,
    #[serde(default)]
    connections: Connections,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Connections {
    #[serde(default = "def_connections_max_idle")]
    max_idle: u32,
    #[serde(default = "def_connections_max_open")]
    max_open: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Log {
    #[serde(default)]
    app: AppLog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppLog {
    #[serde(default)]
    level: LogLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Background {
    #[serde(default)]
    watchdog: Watchdog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Watchdog {
    #[serde(default = "def_watchdog_period")]
    period: String, // напр. "5s"
    #[serde(default = "def_watchdog_limit")]
    limit: u32,
    #[serde(default = "def_watchdog_lock_timeout")]
    lock_timeout: String, // напр. "4s"
}

impl Default for Config {
    fn default() -> Self {
        Self {
            debug: false,
            server: Server::default(),
            db: Db::default(),
            log: Log::default(),
            background: Background::default(),
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self {
            external_url: def_external_url(),
            http_port: def_http_port(),
            grpc_port: def_grpc_port(),
            healthz_port: def_healthz_port(),
            metrics_port: def_metrics_port(),
        }
    }
}

impl Default for Db {
    fn default() -> Self {
        Self { mysql: MySql::default() }
    }
}

impl Default for MySql {
    fn default() -> Self {
        Self {
            host: def_mysql_host(),
            port: def_mysql_port(),
            dating: def_mysql_dating(),
            user: def_mysql_user(),
            pass: String::new(),
            connections: Connections::default(),
        }
    }
}

impl Default for Connections {
    fn default() -> Self {
        Self {
            max_idle: def_connections_max_idle(),
            max_open: def_connections_max_open(),
        }
    }
}

impl Default for Log {
    fn default() -> Self {
        Self { app: AppLog::default() }
    }
}

impl Default for AppLog {
    fn default() -> Self {
        Self { level: LogLevel::Info }
    }
}

impl Default for Background {
    fn default() -> Self {
        Self { watchdog: Watchdog::default() }
    }
}

impl Default for Watchdog {
    fn default() -> Self {
        Self {
            period: def_watchdog_period(),
            limit: def_watchdog_limit(),
            lock_timeout: def_watchdog_lock_timeout(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        };
        f.write_str(s)
    }
}

fn def_external_url() -> String { "http://127.0.0.1".into() }
fn def_http_port() -> u16 { 8081 }
fn def_grpc_port() -> u16 { 8082 }
fn def_healthz_port() -> u16 { 10025 }
fn def_metrics_port() -> u16 { 9199 }

fn def_mysql_host() -> String { "127.0.0.1".into() }
fn def_mysql_port() -> u16 { 3306 }
fn def_mysql_dating() -> String { "default".into() }
fn def_mysql_user() -> String { "root".into() }

fn def_connections_max_idle() -> u32 { 30 }
fn def_connections_max_open() -> u32 { 30 }

fn def_watchdog_period() -> String { "5s".into() }
fn def_watchdog_limit() -> u32 { 10 }
fn def_watchdog_lock_timeout() -> String { "4s".into() }

fn load_config(cli: &Cli) -> Result<Config, String> {
    let mut figment: Figment = Figment::from(Serialized::defaults(Config::default()));

    // 2) TOML файл (якщо існує)
    let path = Path::new(&cli.conf);
    if path.exists() {
        figment = figment.merge(Toml::file(path));
    }

    figment = figment.merge(
        Env::prefixed("CONF_")
            .split("__")
            .global(),
    );

    let mut cfg: Config = figment.extract().map_err(|e| e.to_string())?;

    if cli.debug {
        cfg.debug = true;
    }

    Ok(cfg)
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match load_config(&cli) {
        Ok(cfg) => {
            println!("{}", serde_json::to_string_pretty(&cfg).unwrap());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Failed to load configuration: {e}");
            ExitCode::from(1)
        }
    }
}
