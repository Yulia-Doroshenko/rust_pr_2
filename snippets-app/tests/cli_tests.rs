
use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use predicates::prelude::*;
use std::process::Command;
use std::io::Write;
use regex::Regex;
use tiny_http::{Server, Response};

fn bin() -> Command {
    Command::cargo_bin("snippets-app").expect("binary 'snippets-app'")
}

#[test]
fn list_on_empty_repo_prints_no_snippets() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.child("snippets.json");

    bin()
        .arg("--json-path").arg(json_path.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("(no snippets yet)"));
}

#[test]
fn add_via_stdin_and_list_then_remove() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.child("snippets.json");

    let mut cmd = bin();
    cmd.arg("--json-path").arg(json_path.path())
        .arg("add")
        .arg("--name").arg("hello")
        .arg("--lang").arg("rust");
    let mut child = cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"fn main(){println!(\"hi\");}\n").unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "add via stdin failed: {:?}", out);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("Saved 'hello'"), "unexpected stdout: {}", s);

    let output = bin()
        .arg("--json-path").arg(json_path.path())
        .arg("list")
        .output()
        .unwrap();
    assert!(output.status.success());
    let list = String::from_utf8_lossy(&output.stdout);
    let re = Regex::new(r"^hello \[rust\] @\d{4}-\d{2}-\d{2}T").unwrap();
    assert!(list.lines().any(|l| re.is_match(l)), "unexpected list output: {}", list);

    bin()
        .arg("--json-path").arg(json_path.path())
        .arg("remove")
        .arg("--name").arg("hello")
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed 'hello'"));

    bin()
        .arg("--json-path").arg(json_path.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("(no snippets yet)"));
}

#[test]
fn remove_missing_prints_message_and_succeeds() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.child("snippets.json");

    bin()
        .arg("--json-path").arg(json_path.path())
        .arg("remove").arg("--name").arg("nope")
        .assert()
        .success()
        .stdout(predicate::str::contains("No snippet named 'nope'"));
}

#[test]
fn add_without_stdin_or_download_fails() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.child("snippets.json");

    bin()
        .arg("--json-path").arg(json_path.path())
        .arg("add").arg("--name").arg("x")
        .arg("--lang").arg("text")
        .assert()
        .failure()
        .stderr(predicate::str::contains("STDIN is empty; pass --download <URL> or pipe content"));
}

#[test]
fn add_via_download_from_local_http_server() {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = server.server_addr();
    let url = format!("http://{}/snippet", addr);

    // background thread to serve one request
    let handle = std::thread::spawn(move || {
        if let Ok(req) = server.recv() {
            let _ = req.respond(Response::from_string("println!(\"from http\");"));
        }
    });

    let tmp = assert_fs::TempDir::new().unwrap();
    let json_path = tmp.child("snippets.json");

    bin()
        .arg("--json-path").arg(json_path.path())
        .arg("add")
        .arg("--name").arg("net")
        .arg("--lang").arg("rust")
        .arg("--download").arg(&url)
        .assert()
        .success()
        .stdout(predicate::str::contains("Saved 'net'"));

    let out = bin()
        .arg("--json-path").arg(json_path.path())
        .arg("list")
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("net [rust] @"), "list didn't contain net: {}", s);

    let _ = handle.join();
}

#[test]
fn respects_logging_env_vars_even_if_file_path_invalid() {
    let tmp = assert_fs::TempDir::new().unwrap();
    let log_path = tmp.child("logs/app.log");
    let json_path = tmp.child("snippets.json");

    let mut cmd = bin();
    cmd.env("SNIPPETS_APP_LOG_LEVEL", "debug")
       .env("SNIPPETS_APP_LOG_PATH", log_path.path())
       .arg("--json-path").arg(json_path.path())
       .arg("list");
    cmd.assert().success();
    log_path.assert(predicates::path::exists());
}
