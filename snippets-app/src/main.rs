use std::env;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const STORE_DIR: &str = "snips";

fn print_usage() {
    eprintln!(
        "Usage:
  echo \"code\" | snippets-app --name \"<name>\" [--lang <lang>]
  snippets-app --read \"<name>\"
  snippets-app --delete \"<name>\"
  snippets-app --list"
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

fn path_for(name: &str) -> PathBuf {
    let slug = to_kebab_slug(name);
    let mut p = PathBuf::from(STORE_DIR);
    p.push(format!("{slug}.txt"));
    p
}

fn ensure_dir() -> io::Result<()> {
    if !Path::new(STORE_DIR).exists() {
        fs::create_dir_all(STORE_DIR)?;
    }
    Ok(())
}

fn cmd_create(name: &str, lang: Option<&str>) -> io::Result<()> {
    ensure_dir()?;
    let path = path_for(name);
    if path.exists() {
        eprintln!("snippet already exists: {name}");
        return Ok(());
    }
    let mut buf = String::new();
    io::stdin().read_to_string(&mut buf)?;
    let mut f = File::create(path)?;
    writeln!(f, "name: {name}")?;
    writeln!(f, "lang: {}", lang.unwrap_or(""))?;
    writeln!(f, "----")?;
    f.write_all(buf.as_bytes())?;
    println!("Saved");
    Ok(())
}

fn cmd_read(name: &str) -> io::Result<()> {
    let path = path_for(name);
    if !path.exists() {
        eprintln!("not found: {name}");
        return Ok(());
    }
    let mut s = String::new();
    File::open(path)?.read_to_string(&mut s)?;
    if let Some(idx) = s.find("\n----\n") {
        print!("{}", &s[idx + 6..]);
    } else {
        print!("{s}");
    }
    Ok(())
}

fn cmd_delete(name: &str) -> io::Result<()> {
    let path = path_for(name);
    if !path.exists() {
        eprintln!("not found: {name}");
        return Ok(());
    }
    fs::remove_file(path)?;
    println!("Deleted");
    Ok(())
}

fn cmd_list() -> io::Result<()> {
    ensure_dir()?;
    let mut items = Vec::new();
    for entry in fs::read_dir(STORE_DIR)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|x| x.to_str()) == Some("txt") {
            let mut s = String::new();
            File::open(&p)?.read_to_string(&mut s)?;
            let name_line = s.lines().next().unwrap_or("");
            let name = name_line.strip_prefix("name: ").unwrap_or(name_line);
            items.push((name.to_string(), p));
        }
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, p) in items {
        println!("{}  ({})", name, p.file_name().unwrap().to_string_lossy());
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        print_usage();
        return Ok(());
    }

    match args[0].as_str() {
        "--name" => {
            if args.len() < 2 {
                eprintln!("need: --name \"<name>\" [--lang <lang>]");
                return Ok(());
            }
            let name = &args[1];
            let lang = if args.len() >= 4 && args[2].as_str() == "--lang" {
                Some(args[3].as_str())
            } else {
                None
            };
            cmd_create(name, lang)
        }
        "--read" => {
            if args.len() < 2 {
                eprintln!("need: --read \"<name>\"");
                return Ok(());
            }
            cmd_read(&args[1])
        }
        "--delete" => {
            if args.len() < 2 {
                eprintln!("need: --delete \"<name>\"");
                return Ok(());
            }
            cmd_delete(&args[1])
        }
        "--list" => cmd_list(),
        _ => {
            print_usage();
            Ok(())
        }
    }
}
