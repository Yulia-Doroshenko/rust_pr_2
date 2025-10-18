use std::{
    env,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, types::Type};
use serde::{Deserialize, Serialize};
use thiserror::Error;
// ---------- Domain types ----------

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Snippet {
    name: String,
    lang: String,
    code: String,
    created_at: DateTime<Utc>,
}

trait SnippetStorage {
    fn create(&self, snip: &Snippet) -> Result<bool>;
    fn read(&self, name: &str) -> Result<Option<Snippet>>;
    fn delete(&self, name: &str) -> Result<bool>;
    fn list(&self) -> Result<Vec<Snippet>>;
}

// ---------- Errors ----------

#[derive(Debug, Error)]
enum AppError {
    #[error("I/O error at {path:?}")]
    Io {
        #[source]
        source: io::Error,
        path: PathBuf,
    },

    #[error("failed to (de)serialize JSON at {path:?}")]
    SerdeJson {
        #[source]
        source: serde_json::Error,
        path: PathBuf,
    },

    #[error("SQLite error")]
    Sqlite(#[from] rusqlite::Error),

    #[error("failed to parse RFC3339 datetime from DB: {raw}")]
    ChronoParse {
        #[source]
        source: chrono::ParseError,
        raw: String,
    },

    #[error("invalid storage provider specification: {0}")]
    InvalidProvider(String),

    #[error("stdin read error")]
    Stdin(#[from] io::Error),
}

// ---------- JSON storage ----------

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct JsonFile {
    snippets: Vec<Snippet>,
}

struct JsonStore {
    path: PathBuf,
}

impl JsonStore {
    fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn read_all(&self) -> Result<JsonFile> {
        if !self.path.exists() {
            return Ok(JsonFile::default());
        }
        let f = File::open(&self.path).map_err(|e| AppError::Io {
            source: e,
            path: self.path.clone(),
        })?;
        let v: JsonFile = serde_json::from_reader(f).map_err(|e| AppError::SerdeJson {
            source: e,
            path: self.path.clone(),
        })?;
        Ok(v)
    }

    fn write_all(&self, data: &JsonFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Io {
                source: e,
                path: parent.to_path_buf(),
            })?;
        }
        let f = File::create(&self.path).map_err(|e| AppError::Io {
            source: e,
            path: self.path.clone(),
        })?;
        serde_json::to_writer_pretty(f, data).map_err(|e| AppError::SerdeJson {
            source: e,
            path: self.path.clone(),
        })?;
        Ok(())
    }
}

impl SnippetStorage for JsonStore {
    fn create(&self, snip: &Snippet) -> Result<bool> {
        let mut data = self
            .read_all()
            .with_context(|| format!("loading JSON store from {:?}", self.path))?;
        if data.snippets.iter().any(|s| s.name == snip.name) {
            return Ok(false);
        }
        data.snippets.push(snip.clone());
        self.write_all(&data)
            .with_context(|| format!("writing JSON store to {:?}", self.path))?;
        Ok(true)
    }

    fn read(&self, name: &str) -> Result<Option<Snippet>> {
        let data = self
            .read_all()
            .with_context(|| format!("loading JSON store from {:?}", self.path))?;
        Ok(data.snippets.into_iter().find(|s| s.name == name))
    }

    fn delete(&self, name: &str) -> Result<bool> {
        let mut data = self
            .read_all()
            .with_context(|| format!("loading JSON store from {:?}", self.path))?;
        let before = data.snippets.len();
        data.snippets.retain(|s| s.name != name);
        let changed = data.snippets.len() != before;
        if changed {
            self.write_all(&data)
                .with_context(|| format!("writing JSON store to {:?}", self.path))?;
        }
        Ok(changed)
    }

    fn list(&self) -> Result<Vec<Snippet>> {
        let mut v = self
            .read_all()
            .with_context(|| format!("loading JSON store from {:?}", self.path))?
            .snippets;
        v.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(v)
    }
}

// ---------- SQLite storage ----------

struct SqliteStore {
    conn: rusqlite::Connection,
}

impl SqliteStore {
    fn new(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::Io {
                source: e,
                path: parent.to_path_buf(),
            })?;
        }
        let conn = rusqlite::Connection::open(path.as_ref()).with_context(|| {
            format!("opening SQLite DB at {:?}", path.as_ref())
        })?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS snippets (
                name        TEXT PRIMARY KEY,
                lang        TEXT NOT NULL,
                code        TEXT NOT NULL,
                created_at  TEXT NOT NULL
            );
            "#,
        )
        .context("initializing SQLite schema")?;
        Ok(Self { conn })
    }
}

impl SnippetStorage for SqliteStore {
    fn create(&self, snip: &Snippet) -> Result<bool> {
        let res = self
            .conn
            .execute(
                "INSERT OR IGNORE INTO snippets(name, lang, code, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    snip.name,
                    snip.lang,
                    snip.code,
                    snip.created_at.to_rfc3339(),
                ],
            )
            .context("inserting snippet into SQLite")?;
        Ok(res == 1)
    }

fn read(&self, name: &str) -> Result<Option<Snippet>> {
    self.conn
        .query_row(
            "SELECT name, lang, code, created_at FROM snippets WHERE name = ?1",
            params![name],
            |row| {
                let raw: String = row.get(3)?;
                let created_at = DateTime::parse_from_rfc3339(&raw)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        3,
                        Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Snippet {
                    name: row.get(0)?,
                    lang: row.get(1)?,
                    code: row.get(2)?,
                    created_at,
                })
            },
        )
        .optional()
        .context("selecting snippet from SQLite")
    }

    fn delete(&self, name: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM snippets WHERE name = ?1", params![name])
            .context("deleting snippet from SQLite")?;
        Ok(n == 1)
    }

    fn list(&self) -> Result<Vec<Snippet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, lang, code, created_at FROM snippets ORDER BY name ASC")
            .context("preparing SQLite SELECT for list()")?;

        let rows = stmt
            .query_map([], |row| {
                let raw: String = row.get(3)?;
                let created_at = DateTime::parse_from_rfc3339(&raw)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                        3,
                        Type::Text,
                        Box::new(e),
                    ))?;

                Ok(Snippet {
                    name: row.get(0)?,
                    lang: row.get(1)?,
                    code: row.get(2)?,
                    created_at,
                })
            })
            .context("iterating SQLite rows for list()")?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r.context("mapping row into Snippet")?);
        }
        Ok(out)
    }
}

// ---------- CLI helpers ----------

fn print_usage() {
    eprintln!(
        "Usage:
  echo \"code\" | snippets-app --name \"<name>\" [--lang <lang>]
  snippets-app --read \"<name>\"
  snippets-app --delete \"<name>\"
  snippets-app --list

Storage selection (env):
  SNIPPETS_APP_STORAGE=\"JSON:/path/to/snippets.json\"
  SNIPPETS_APP_STORAGE=\"SQLITE:/path/to/snippets.sqlite\"
"
    );
}

fn to_kebab_slug(input: &str) -> String {
    let lower = input.trim().to_lowercase();
    let mut out = String::with_capacity(lower.len());
    for ch in lower.chars() {
        let ok = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == ' ';
        out.push(if ok { ch } else { ' ' });
    }
    out.split_whitespace().collect::<Vec<_>>().join("-")
}

enum Provider {
    Json(PathBuf),
    Sqlite(PathBuf),
}

fn pick_provider_from_env() -> Result<Provider> {
    let raw = env::var("SNIPPETS_APP_STORAGE").unwrap_or_else(|_| "JSON:snips/snippets.json".into());
    let (kind, path) = raw
        .split_once(':')
        .map(|(k, p)| (k.trim(), p.trim()))
        .unwrap_or(("JSON", "snips/snippets.json"));

    let pb = PathBuf::from(path);
    let prov = match kind.to_uppercase().as_str() {
        "SQLITE" | "SQLITE3" => Provider::Sqlite(pb),
        "JSON" => Provider::Json(pb),
        other => {
            return Err(AppError::InvalidProvider(format!(
                "unknown kind '{other}', expected JSON or SQLITE; raw={raw}"
            ))
            .into())
        }
    };
    Ok(prov)
}

fn open_storage() -> Result<Box<dyn SnippetStorage>> {
    match pick_provider_from_env().context("parsing SNIPPETS_APP_STORAGE")? {
        Provider::Json(p) => {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(|e| AppError::Io {
                    source: e,
                    path: parent.to_path_buf(),
                })?;
            }
            Ok(Box::new(JsonStore::new(p)))
        }
        Provider::Sqlite(p) => Ok(Box::new(
            SqliteStore::new(&p).with_context(|| format!("opening SQLite at {:?}", p))?,
        )),
    }
}

// ---------- Main ----------

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    let store = open_storage().context("initializing storage")?;

    match args[0].as_str() {
        "--name" => {
            if args.len() < 2 {
                eprintln!("need: --name \"<name>\" [--lang <lang>\"]");
                return Ok(());
            }
            let name = to_kebab_slug(&args[1]);
            if name.is_empty() {
                bail!("empty name after slug normalization");
            }

            let lang = if args.len() >= 4 && args[2].as_str() == "--lang" {
                Some(args[3].as_str())
            } else {
                None
            };

            let mut buf = String::new();
            io::stdin()
                .read_to_string(&mut buf)
                .context("reading snippet code from stdin")?;

            if buf.trim().is_empty() {
                bail!("stdin is empty; provide code via pipe or redirection");
            }

            let snip = Snippet {
                name,
                lang: lang.unwrap_or("").to_string(),
                code: buf,
                created_at: Utc::now(),
            };
            let created = store
                .create(&snip)
                .with_context(|| format!("creating snippet '{}'", snip.name))?;
            if created {
                println!("Saved");
            } else {
                eprintln!("snippet already exists");
            }
        }

        "--read" => {
            if args.len() < 2 {
                eprintln!("need: --read \"<name>\"");
                return Ok(());
            }
            let name = to_kebab_slug(&args[1]);
            match store
                .read(&name)
                .with_context(|| format!("reading snippet '{name}'"))?
            {
                Some(s) => print!("{}", s.code),
                None => eprintln!("not found: {}", name),
            }
        }

        "--delete" => {
            if args.len() < 2 {
                eprintln!("need: --delete \"<name>\"");
                return Ok(());
            }
            let name = to_kebab_slug(&args[1]);
            if store
                .delete(&name)
                .with_context(|| format!("deleting snippet '{name}'"))?
            {
                println!("Deleted");
            } else {
                eprintln!("not found: {}", name);
            }
        }

        "--list" => {
            let mut items = store.list().context("listing snippets")?;
            items.sort_by(|a, b| a.name.cmp(&b.name));
            for s in items {
                println!("{}  [{}]  created_at={}", s.name, s.lang, s.created_at.to_rfc3339());
            }
        }

        _ => print_usage(),
    }

    Ok(())
}
