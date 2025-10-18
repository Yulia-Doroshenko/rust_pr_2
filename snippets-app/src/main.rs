use std::{
    collections::BTreeMap,
    env,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, EnvFilter};

#[derive(Debug, Clone)]
struct Config {
    log_level: String,
    log_path: Option<PathBuf>,
    json_path: PathBuf,
}

impl Config {
    fn from_env_and_defaults(json_path: PathBuf) -> Self {
        let log_level = env::var("SNIPPETS_APP_LOG_LEVEL").unwrap_or_else(|_| "info".to_owned());
        let log_path = env::var("SNIPPETS_APP_LOG_PATH").ok().map(PathBuf::from);
        Self { log_level, log_path, json_path }
    }
}

fn init_tracing(cfg: &Config) -> Result<Option<WorkerGuard>> {
    let filter = EnvFilter::try_new(&cfg.log_level).unwrap_or_else(|_| EnvFilter::new("info"));
    if let Some(path) = cfg.log_path.as_ref() {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
        let file = std::fs::OpenOptions::new().create(true).append(true).open(path)
            .with_context(|| format!("opening log file '{}'", path.display()))?;
        let (nb, guard) = tracing_appender::non_blocking(file);
        fmt().with_env_filter(filter)
             .with_target(true)
             .with_level(true)
             .with_ansi(false)
             .with_writer(nb)
             .init();
        Ok(Some(guard))
    } else {
        fmt().with_env_filter(filter)
             .with_target(true)
             .with_level(true)
             .init();
        Ok(None)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Snippet {
    name: String,
    lang: String,
    code: String,
    created_at: DateTime<Utc>,
}

#[derive(Default, Serialize, Deserialize)]
struct Repo {
    items: BTreeMap<String, Snippet>,
}

impl Repo {
    fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let file = File::open(path).with_context(|| format!("open '{}'", path.display()))?;
        let repo: Repo = serde_json::from_reader(io::BufReader::new(file))
            .with_context(|| format!("parse JSON '{}'", path.display()))?;
        Ok(repo)
    }
    fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).ok(); }
        let file = File::create(path).with_context(|| format!("create '{}'", path.display()))?;
        serde_json::to_writer_pretty(io::BufWriter::new(file), self)?;
        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(name = "snippets-app",
          version,
          about = "Store and manage code snippets; supports downloading via --download",
          disable_help_subcommand = true)]
struct Cli {
    #[arg(long = "json-path", value_name = "PATH", default_value = "snippets.json")]
    json_path: PathBuf,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Add(AddCmd),
    List,
    Remove(RemoveCmd),
}

#[derive(Args, Debug)]
struct AddCmd {
    #[arg(short = 'n', long = "name")]
    name: String,

    #[arg(short = 'l', long = "lang", default_value = "text")]
    lang: String,

    #[arg(long = "download", value_name = "URL")]
    download: Option<String>,
}

#[derive(Args, Debug)]
struct RemoveCmd {
    #[arg(short = 'n', long = "name")]
    name: String,
}

fn read_stdin_text() -> Result<String> {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).context("read from STDIN")?;
    if buf.is_empty() {
        return Err(anyhow!("STDIN is empty; pass --download <URL> or pipe content"));
    }
    Ok(String::from_utf8_lossy(&buf).to_string())
}

fn fetch_url(url: &str) -> Result<String> {
    info!(target: "snippets", url = url, "downloading snippet");
    let resp = reqwest::blocking::get(url).with_context(|| format!("GET {}", url))?;
    let st = resp.status();
    if !st.is_success() {
        return Err(anyhow!("HTTP {}", st));
    }
    let text = resp.text().context("read response body")?;
    Ok(text)
}

fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();
    let cfg = Config::from_env_and_defaults(cli.json_path.clone());
    let _guard = init_tracing(&cfg)?;

    debug!("Starting with json_path='{}'", cfg.json_path.display());

    let mut repo = Repo::load(&cfg.json_path)?;

    match cli.cmd {
        Command::List => {
            if repo.items.is_empty() {
                println!("(no snippets yet)");
                return Ok(());
            }
            for sn in repo.items.values() {
                println!("{} [{}] @{}", sn.name, sn.lang, sn.created_at.to_rfc3339());
            }
            info!(count = repo.items.len(), "listed snippets");
        }
        Command::Remove(RemoveCmd { name }) => {
            let existed = repo.items.remove(&name).is_some();
            repo.save(&cfg.json_path)?;
            if existed {
                println!("Removed '{name}'");
                info!(name = %name, "removed");
            } else {
                println!("No snippet named '{name}'");
                warn!(name = %name, "remove requested but not found");
            }
        }
        Command::Add(AddCmd { name, lang, download }) => {
            let code = if let Some(url) = download.as_deref() {
                fetch_url(url)?
            } else {
                read_stdin_text()?
            };

            let sn = Snippet {
                name: name.clone(),
                lang,
                code,
                created_at: Utc::now(),
            };
            repo.items.insert(name.clone(), sn);
            repo.save(&cfg.json_path)?;
            println!("Saved '{name}'");
            info!(name = %name, "saved/updated");
        }
    }

    Ok(())
}
