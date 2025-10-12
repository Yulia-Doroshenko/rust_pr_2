use std::{
    env,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

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
        let f = File::open(&self.path)?;
        let v: JsonFile = serde_json::from_reader(f)?;
        Ok(v)
    }

    fn write_all(&self, data: &JsonFile) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let f = File::create(&self.path)?;
        serde_json::to_writer_pretty(f, data)?;
        Ok(())
    }
}

impl SnippetStorage for JsonStore {
    fn create(&self, snip: &Snippet) -> Result<bool> {
        let mut data = self.read_all()?;
        if data.snippets.iter().any(|s| s.name == snip.name) {
            return Ok(false);
        }
        data.snippets.push(snip.clone());
        self.write_all(&data)?;
        Ok(true)
    }

    fn read(&self, name: &str) -> Result<Option<Snippet>> {
        let data = self.read_all()?;
        Ok(data.snippets.into_iter().find(|s| s.name == name))
    }

    fn delete(&self, name: &str) -> Result<bool> {
        let mut data = self.read_all()?;
        let before = data.snippets.len();
        data.snippets.retain(|s| s.name != name);
        let changed = data.snippets.len() != before;
        if changed {
            self.write_all(&data)?;
        }
        Ok(changed)
    }

    fn list(&self) -> Result<Vec<Snippet>> {
        let data = self.read_all()?;
        let mut v = data.snippets;
        v.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(v)
    }
}

struct SqliteStore {
    conn: rusqlite::Connection,
}

impl SqliteStore {
    fn new(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = rusqlite::Connection::open(path)?;
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
        )?;
        Ok(Self { conn })
    }
}

impl SnippetStorage for SqliteStore {
    fn create(&self, snip: &Snippet) -> Result<bool> {
        let res = self.conn.execute(
            "INSERT OR IGNORE INTO snippets(name, lang, code, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                snip.name,
                snip.lang,
                snip.code,
                snip.created_at.to_rfc3339(),
            ],
        )?;
        Ok(res == 1)
    }

    fn read(&self, name: &str) -> Result<Option<Snippet>> {
        self.conn
            .query_row(
                "SELECT name, lang, code, created_at FROM snippets WHERE name = ?1",
                params![name],
                |row| {
                    let created_at: String = row.get(3)?;
                    let created_at = DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            3, rusqlite::types::Type::Text, Box::new(e),
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
            .map_err(Into::into)
    }

    fn delete(&self, name: &str) -> Result<bool> {
        let n = self
            .conn
            .execute("DELETE FROM snippets WHERE name = ?1", params![name])?;
        Ok(n == 1)
    }

    fn list(&self) -> Result<Vec<Snippet>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name, lang, code, created_at FROM snippets ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| {
            let created_at: String = row.get(3)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                    3, rusqlite::types::Type::Text, Box::new(e),
                ))?;
            Ok(Snippet {
                name: row.get(0)?,
                lang: row.get(1)?,
                code: row.get(2)?,
                created_at,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }
}
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

fn pick_provider_from_env() -> Provider {
    let raw = env::var("SNIPPETS_APP_STORAGE")
        .unwrap_or_else(|_| "JSON:snips/snippets.json".into());
    let (kind, path) = raw
        .split_once(':')
        .map(|(k, p)| (k.trim(), p.trim()))
        .unwrap_or(("JSON", "snips/snippets.json"));
    let pb = PathBuf::from(path);
    match kind.to_uppercase().as_str() {
        "SQLITE" | "SQLITE3" => Provider::Sqlite(pb),
        "JSON" => Provider::Json(pb),
        _ => Provider::Json(pb),
    }
}

fn open_storage() -> Result<Box<dyn SnippetStorage>> {
    Ok(match pick_provider_from_env() {
        Provider::Json(p) => {
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            Box::new(JsonStore::new(p))
        }
        Provider::Sqlite(p) => Box::new(SqliteStore::new(p)?),
    })
}

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    let store = open_storage()?;

    match args[0].as_str() {
        "--name" => {
            if args.len() < 2 {
                eprintln!("need: --name \"<name>\" [--lang <lang>]");
                return Ok(());
            }
            let mut name = args[1].clone();
            name = to_kebab_slug(&name);
            let lang = if args.len() >= 4 && args[2].as_str() == "--lang" {
                Some(args[3].as_str())
            } else {
                None
            };

            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            let snip = Snippet {
                name,
                lang: lang.unwrap_or("").to_string(),
                code: buf,
                created_at: Utc::now(),
            };
            let created = store.create(&snip)?;
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
            match store.read(&name)? {
                Some(s) => {
                    print!("{}", s.code);
                }
                None => eprintln!("not found: {}", name),
            }
        }

        "--delete" => {
            if args.len() < 2 {
                eprintln!("need: --delete \"<name>\"");
                return Ok(());
            }
            let name = to_kebab_slug(&args[1]);
            if store.delete(&name)? {
                println!("Deleted");
            } else {
                eprintln!("not found: {}", name);
            }
        }

        "--list" => {
            let mut items = store.list()?;
            items.sort_by(|a, b| a.name.cmp(&b.name));
            for s in items {
                println!("{}  [{}]  created_at={}", s.name, s.lang, s.created_at.to_rfc3339());
            }
        }

        _ => print_usage(),
    }

    Ok(())
}
