//! Runs the command-line entry as a real process.

use std::fs;
use std::io::Write;
use std::process::{Command, Output, Stdio};

const FLOW_ASCII: &str = r#"+----+    +----+
|    |    |    |
|  A |---->  B |
+----+    +----+
"#;

const SEQUENCE_ASCII: &str = r#"+--------+   +------+
|  Alice |   | Bob  |
|        |   |      |
+----+---+   +--+---+
     |          |
     +--Hello--->
     |          |
     |    Hi    |
     <----------+
+--------+   +------+
|  Alice |   | Bob  |
|        |   |      |
+--------+   +------+
"#;

const CLASS_ASCII: &str = r#"+---------+
|  Animal |
|         |
+---------+
| +int age|
+---------+
     ^
     |
     |
  +-----+
  | Duck|
  +-----+
  +-----+
"#;

const STATE_ASCII: &str = r#"     o
     |
     |
     v
+--------+
|  Still |
+--------+
     |
     |
     v
     o
"#;

const WIDE_ASCII: &str = r#"+---------+    +----+
|         |    |    |
|  日本語 |---->  B |
+---------+    +----+
"#;

const DIAMOND_ASCII: &str = r#"      +--------+
      |        |
      | Start  |
      +---+----+
          |
          |
          v
       /     \
    +   Ready?   +
      \        /
        \   /
      +---+---+
    +-+       +-+
   Yes          No
    |           |
+---v---+   +---v---+
|       |   |       |
|  Ship |   |  Wait |
+-------+   +-------+
"#;

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

#[test]
fn prints_stdin_as_ascii() {
    let output = run_with_stdin(&["--ascii"], "flowchart LR\n  A --> B\n");

    assert!(output.status.success());
    assert_eq!(output.stdout, FLOW_ASCII.as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn prints_sequence_diagrams_as_ascii() {
    let output = run_with_stdin(
        &["--ascii"],
        "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi\n",
    );

    assert!(output.status.success());
    assert_eq!(output.stdout, SEQUENCE_ASCII.as_bytes());
}

#[test]
fn prints_class_diagrams_as_ascii() {
    let output = run_with_stdin(
        &["--ascii"],
        "classDiagram\n  Animal <|-- Duck\n  Animal : +int age\n",
    );

    assert!(output.status.success());
    assert_eq!(output.stdout, CLASS_ASCII.as_bytes());
}

#[test]
fn prints_state_diagrams_as_ascii() {
    let output = run_with_stdin(
        &["--ascii"],
        "stateDiagram-v2\n  [*] --> Still\n  Still --> [*]\n",
    );

    assert!(output.status.success());
    assert_eq!(output.stdout, STATE_ASCII.as_bytes());
}

#[test]
fn prints_wide_characters_as_ascii() {
    let output = run_with_stdin(&["--ascii"], "flowchart LR\n  A[日本語] --> B\n");

    assert!(output.status.success());
    assert_eq!(output.stdout, WIDE_ASCII.as_bytes());
}

#[test]
fn prints_edge_labels_as_ascii() {
    let output = run_with_stdin(
        &["--ascii"],
        "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|Yes| C[Ship]\n  B -->|No| D[Wait]\n",
    );

    assert!(output.status.success());
    assert_eq!(output.stdout, DIAMOND_ASCII.as_bytes());
}

#[test]
fn prints_files_as_ascii_without_writing_svg() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("flow.mmd");
    fs::write(&input, "flowchart LR\n  A --> B\n").expect("write input");

    let output = command()
        .arg("--ascii")
        .arg(&input)
        .output()
        .expect("run ondine");

    assert!(output.status.success());
    assert_eq!(output.stdout, FLOW_ASCII.as_bytes());
    assert!(output.stderr.is_empty());
    assert!(!directory.path().join("flow.svg").exists());
}

#[test]
fn concatenates_ascii_for_multiple_files() {
    let directory = tempfile::tempdir().expect("tempdir");
    let flow = directory.path().join("flow.mmd");
    let sequence = directory.path().join("sequence.mmd");
    fs::write(&flow, "flowchart LR\n  A --> B\n").expect("write flow");
    fs::write(
        &sequence,
        "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi\n",
    )
    .expect("write sequence");

    let output = command()
        .arg("--ascii")
        .arg(&flow)
        .arg(&sequence)
        .output()
        .expect("run ondine");

    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        format!("{FLOW_ASCII}{SEQUENCE_ASCII}").as_bytes()
    );
    assert!(!directory.path().join("flow.svg").exists());
    assert!(!directory.path().join("sequence.svg").exists());
}

#[test]
fn ignores_out_dir_in_ascii_mode() {
    let directory = tempfile::tempdir().expect("tempdir");
    let input = directory.path().join("flow.mmd");
    let out = directory.path().join("nested/assets");
    fs::write(&input, "flowchart LR\n  A --> B\n").expect("write input");

    let output = command()
        .arg("--ascii")
        .arg(&input)
        .arg("--out-dir")
        .arg(&out)
        .output()
        .expect("run ondine");

    assert!(output.status.success());
    assert_eq!(output.stdout, FLOW_ASCII.as_bytes());
    assert!(!out.exists());
    assert!(!directory.path().join("flow.svg").exists());
}

#[test]
fn accepts_short_ascii_flag() {
    let output = run_with_stdin(&["-a"], "flowchart LR\n  A --> B\n");

    assert!(output.status.success());
    assert_eq!(output.stdout, FLOW_ASCII.as_bytes());
}

#[test]
fn reports_ascii_render_errors() {
    let output = run_with_stdin(&["--ascii"], "flowchart LR\n  A -->\n");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.starts_with("<stdin>:"));
    assert!(stderr.contains("Parse error"));
}
