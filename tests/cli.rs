//! Runs the command-line entry as a real process.

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ondine"))
}

fn run_with_stdin(args: &[&str], input: &str) -> Output {
    let mut child = command()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn ondine");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(input.as_bytes())
        .expect("write stdin");

    child.wait_with_output().expect("wait for ondine")
}

#[test]
fn renders_stdin_byte_for_byte() {
    let source = "flowchart LR\n  A --> B\n";
    let expected = ondine_compiler::render(source, "", 0.0)
        .expect("render expected")
        .svg;

    let output = run_with_stdin(&[], source);

    assert!(output.status.success());
    assert_eq!(output.stdout, expected.as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn renders_files_beside_the_source() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = "flowchart LR\n  A --> B\n";
    let input = directory.path().join("flow.mmd");
    fs::write(&input, source).expect("write input");

    let expected = ondine_compiler::render(source, "", 0.0)
        .expect("render expected")
        .svg;

    let output = command().arg(&input).output().expect("run ondine");

    assert!(output.status.success());
    let svg = fs::read_to_string(directory.path().join("flow.svg")).expect("read svg");
    assert_eq!(svg, expected);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("flow.mmd -> "));
}

#[test]
fn renders_into_an_output_directory() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("flow.mmd");
    fs::write(&input, "flowchart LR\n  A --> B\n").expect("write input");
    let out = directory.path().join("nested/assets");

    let output = command()
        .arg(&input)
        .arg("--out-dir")
        .arg(&out)
        .output()
        .expect("run ondine");

    assert!(output.status.success());
    assert!(out.join("flow.svg").exists());
}

#[test]
fn keeps_other_extensions_in_the_output_name() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("flow.txt");
    fs::write(&input, "flowchart LR\n  A --> B\n").expect("write input");

    let output = command().arg(&input).output().expect("run ondine");

    assert!(output.status.success());
    assert!(directory.path().join("flow.txt.svg").exists());
}

#[test]
fn continues_after_a_failed_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let good = directory.path().join("good.mmd");
    let bad = directory.path().join("bad.mmd");
    fs::write(&good, "flowchart LR\n  A --> B\n").expect("write good");
    fs::write(&bad, "flowchart LR\n  A -->\n").expect("write bad");

    let output = command().arg(&good).arg(&bad).output().expect("run ondine");

    assert!(!output.status.success());
    assert!(directory.path().join("good.svg").exists());
    assert!(!directory.path().join("bad.svg").exists());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bad.mmd:"));
    assert!(stderr.contains("Parse error"));
}

#[test]
fn reports_stdin_errors_with_a_location() {
    let output = run_with_stdin(&[], "flowchart LR\n  A -->\n");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("<stdin>:"));
    assert!(stderr.contains("Parse error"));
}
