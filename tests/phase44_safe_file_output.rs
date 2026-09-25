use std::fs;
use std::path::Path;
use std::process::{Command, Output};

mod support;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(args)
        .output()
        .expect("run qzt")
}

fn run_in(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .current_dir(directory)
        .args(args)
        .output()
        .expect("run qzt")
}

fn path(path: &Path) -> &str {
    path.to_str().expect("temporary path is UTF-8")
}

fn assert_rejected_unchanged(args: &[&str], protected: &Path, before: &[u8]) {
    let result = run(args);
    assert!(
        !result.status.success(),
        "command unexpectedly succeeded: {args:?}"
    );
    assert!(fs::read(protected).unwrap() == before, "changed: {args:?}");
}

#[test]
fn cli_rejects_self_overwrite_before_writing() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("source.txt");
    let packed = temp.path().join("source.qzt");
    fs::write(&source, b"one document\n").unwrap();
    assert!(
        run(&["pack", path(&source), "-o", path(&packed)])
            .status
            .success()
    );
    let before = fs::read(&packed).unwrap();

    assert_rejected_unchanged(
        &["export", path(&packed), "-o", path(&packed)],
        &packed,
        &before,
    );
    assert_rejected_unchanged(
        &["sidecar-rebuild", path(&packed), "-o", path(&packed)],
        &packed,
        &before,
    );
    assert_rejected_unchanged(
        &["pack", path(&packed), "-o", path(&packed)],
        &packed,
        &before,
    );
    assert_rejected_unchanged(
        &[
            "pack-docs",
            path(&source),
            path(&packed),
            "-o",
            path(&packed),
        ],
        &packed,
        &before,
    );

    let docs = temp.path().join("docs.qzt");
    assert!(
        run(&["pack-docs", path(&source), "-o", path(&docs)])
            .status
            .success()
    );
    let before_docs = fs::read(&docs).unwrap();
    assert_rejected_unchanged(
        &["doc", path(&docs), "source.txt", "-o", path(&docs)],
        &docs,
        &before_docs,
    );
}

#[test]
fn corrupt_export_keeps_existing_output_and_source() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("bad.qzt");
    let output = temp.path().join("existing.txt");
    fs::write(&source, b"not a QZT").unwrap();
    fs::write(&output, b"existing data").unwrap();
    let before_source = fs::read(&source).unwrap();
    let before_output = fs::read(&output).unwrap();
    assert_rejected_unchanged(
        &["export", path(&source), "-o", path(&output)],
        &output,
        &before_output,
    );
    assert_eq!(fs::read(&source).unwrap(), before_source);
}

#[test]
fn aliases_and_symlink_output_cannot_replace_an_input() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("source.txt");
    let packed = temp.path().join("source.qzt");
    fs::write(&source, b"alias fixture\n").unwrap();
    assert!(
        run(&["pack", path(&source), "-o", path(&packed)])
            .status
            .success()
    );
    let original = fs::read(&packed).unwrap();

    let relative = run_in(temp.path(), &["export", "source.qzt", "-o", "./source.qzt"]);
    assert!(!relative.status.success());
    assert!(String::from_utf8_lossy(&relative.stderr).contains("same file"));
    assert_eq!(fs::read(&packed).unwrap(), original);

    let hard_link = temp.path().join("hard-link.qzt");
    fs::hard_link(&packed, &hard_link).unwrap();
    let hard = run(&["export", path(&packed), "-o", path(&hard_link)]);
    assert!(!hard.status.success());
    assert!(String::from_utf8_lossy(&hard.stderr).contains("same file"));
    assert_eq!(fs::read(&packed).unwrap(), original);
    assert_eq!(fs::read(&hard_link).unwrap(), original);

    let symlink = temp.path().join("symlink.qzt");
    #[cfg(unix)]
    let symlink_result = std::os::unix::fs::symlink(&packed, &symlink);
    #[cfg(windows)]
    let symlink_result = std::os::windows::fs::symlink_file(&packed, &symlink);
    if let Err(error) = symlink_result {
        eprintln!("symlink case skipped: OS denied symlink creation: {error}");
    } else {
        let link = run(&["export", path(&packed), "-o", path(&symlink)]);
        assert!(!link.status.success());
        assert!(String::from_utf8_lossy(&link.stderr).contains("symlink"));
        assert!(
            fs::symlink_metadata(&symlink)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&packed).unwrap(), original);
    }

    let different_case = temp.path().join("SOURCE.QZT");
    if fs::metadata(&different_case).is_ok() {
        let case = run(&["export", path(&packed), "-o", path(&different_case)]);
        assert!(!case.status.success());
        assert!(String::from_utf8_lossy(&case.stderr).contains("same file"));
        assert_eq!(fs::read(&packed).unwrap(), original);
    } else {
        eprintln!("case-insensitive alias skipped: test filesystem distinguishes filename case");
    }
}

#[test]
fn later_chunk_decode_failure_keeps_existing_output_and_cleans_temporary_file() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("source.txt");
    let packed = temp.path().join("corrupt.qzt");
    let output = temp.path().join("restored.txt");
    let mut text = Vec::new();
    for _ in 0..4 {
        text.extend(std::iter::repeat_n(b'x', 9_000));
        text.push(b'\n');
    }
    fs::write(&source, &text).unwrap();
    assert!(
        run(&[
            "pack",
            path(&source),
            "--chunk-size",
            "10000",
            "--max-chunk-size",
            "12000",
            "-o",
            path(&packed),
        ])
        .status
        .success()
    );
    let mut bytes = fs::read(&packed).unwrap();
    let details = qzt::open_skeleton_details(&bytes).unwrap();
    assert!(details.chunk_entries.len() > 1);
    let offset = usize::try_from(details.chunk_entries[1].physical_offset).unwrap();
    bytes[offset] ^= 0xff;
    fs::write(&packed, &bytes).unwrap();
    fs::write(&output, b"previous restoration").unwrap();

    let result = run(&["export", path(&packed), "-o", path(&output)]);
    assert!(!result.status.success());
    assert_eq!(fs::read(&packed).unwrap(), bytes);
    assert_eq!(fs::read(&output).unwrap(), b"previous restoration");
    assert!(!fs::read_dir(temp.path()).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("qzt-tmp")
    }));
}

#[test]
fn normal_file_and_stdout_output_round_trip_and_replace() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("source.txt");
    let packed = temp.path().join("packed.qzt");
    let packed_again = temp.path().join("packed-again.qzt");
    let restored = temp.path().join("restored.txt");
    fs::write(&source, b"first\nsecond\n").unwrap();
    assert!(
        run(&["pack", path(&source), "-o", path(&packed)])
            .status
            .success()
    );
    let packed_bytes = fs::read(&packed).unwrap();
    fs::write(&packed_again, b"old pack").unwrap();
    assert!(
        run(&["pack", path(&source), "-o", path(&packed_again)])
            .status
            .success()
    );
    assert_eq!(fs::read(&packed_again).unwrap(), packed_bytes);

    let stdout = run(&["export", path(&packed)]);
    assert!(stdout.status.success());
    assert_eq!(stdout.stdout, fs::read(&source).unwrap());
    assert!(
        run(&["export", path(&packed), "-o", path(&restored)])
            .status
            .success()
    );
    assert_eq!(fs::read(&restored).unwrap(), stdout.stdout);
    fs::write(&restored, b"old restoration").unwrap();
    assert!(
        run(&["export", path(&packed), "-o", path(&restored)])
            .status
            .success()
    );
    assert_eq!(fs::read(&restored).unwrap(), stdout.stdout);
    let relative = run_in(temp.path(), &["export", "packed.qzt", "-o", "relative.txt"]);
    assert!(
        relative.status.success(),
        "{}",
        String::from_utf8_lossy(&relative.stderr)
    );
    assert_eq!(
        fs::read(temp.path().join("relative.txt")).unwrap(),
        stdout.stdout
    );
}

#[test]
fn replacement_preserves_existing_output_acl() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("source.txt");
    let packed = temp.path().join("source.qzt");
    let output = temp.path().join("private.txt");
    fs::write(&source, b"replacement data\n").unwrap();
    assert!(
        run(&["pack", path(&source), "-o", path(&packed)])
            .status
            .success()
    );
    fs::write(&output, b"old private data").unwrap();
    if !set_acl(&output) {
        return;
    }
    let before = read_acl(&output);

    let result = run(&["export", path(&packed), "-o", path(&output)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&output).unwrap(), fs::read(&source).unwrap());
    assert_eq!(read_acl(&output), before);

    #[cfg(windows)]
    {
        let inheritance = Command::new("icacls")
            .arg(&output)
            .arg("/inheritance:e")
            .output()
            .unwrap();
        assert!(
            inheritance.status.success(),
            "{}",
            String::from_utf8_lossy(&inheritance.stderr)
        );
        let inherited_acl = read_acl(&output);
        let result = run(&["export", path(&packed), "-o", path(&output)]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read(&output).unwrap(), fs::read(&source).unwrap());
        assert_eq!(read_acl(&output), inherited_acl);
    }
}

#[cfg(windows)]
#[test]
fn readonly_existing_output_is_rejected_before_any_output() {
    let mut failures = Vec::new();
    for command in ["pack", "pack-docs", "export", "doc", "sidecar-rebuild"] {
        let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
        let source = temp.path().join("source.txt");
        let second = temp.path().join("second.txt");
        let packed = temp.path().join("source.qzt");
        let docs = temp.path().join("docs.qzt");
        let output = temp.path().join("existing-output");
        fs::write(&source, b"input original\n").unwrap();
        fs::write(&second, b"second original\n").unwrap();
        assert!(
            run(&["pack", path(&source), "-o", path(&packed)])
                .status
                .success()
        );
        assert!(
            run(&["pack-docs", path(&source), "-o", path(&docs)])
                .status
                .success()
        );
        fs::write(&output, b"existing output original").unwrap();
        assert!(set_acl(&output));
        let acl_before = read_acl(&output);
        let mut permissions = fs::metadata(&output).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&output, permissions).unwrap();

        let args = match command {
            "pack" => vec!["pack", path(&source), "-o", path(&output)],
            "pack-docs" => vec![
                "pack-docs",
                path(&source),
                path(&second),
                "-o",
                path(&output),
            ],
            "export" => vec!["export", path(&packed), "-o", path(&output)],
            "doc" => vec!["doc", path(&docs), "source.txt", "-o", path(&output)],
            "sidecar-rebuild" => vec!["sidecar-rebuild", path(&packed), "-o", path(&output)],
            _ => unreachable!(),
        };
        let before = [source.clone(), second.clone(), packed.clone(), docs.clone()]
            .map(|input| (input.clone(), fs::read(input).unwrap()));
        let result = run(&args);
        let stderr = String::from_utf8_lossy(&result.stderr);
        let leftovers: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("qzt-tmp"))
            .collect();
        let output_preserved = fs::read(&output).unwrap() == b"existing output original";
        let readonly_preserved = fs::metadata(&output).unwrap().permissions().readonly();
        let acl_preserved = read_acl(&output) == acl_before;
        let inputs_preserved = before
            .iter()
            .all(|(input, bytes)| fs::read(input).unwrap().as_slice() == bytes.as_slice());
        eprintln!(
            "{command}: exit={:?}, stderr={stderr:?}, output={output_preserved}, readonly={readonly_preserved}, acl={acl_preserved}, inputs={inputs_preserved}, temporary={leftovers:?}",
            result.status.code()
        );
        let rejected_safely = !result.status.success()
            && stderr.contains("read-only")
            && output_preserved
            && readonly_preserved
            && acl_preserved
            && inputs_preserved
            && leftovers.is_empty();
        if !rejected_safely {
            failures.push(format!(
                "{command}: readonly rejection did not preserve all state"
            ));
        }

        let mut permissions = fs::metadata(&output).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&output, permissions).unwrap();
        let writable = run(&args);
        if !writable.status.success()
            || fs::read(&output).unwrap() == b"existing output original"
            || read_acl(&output) != acl_before
            || !before
                .iter()
                .all(|(input, bytes)| fs::read(input).unwrap().as_slice() == bytes.as_slice())
        {
            failures.push(format!(
                "{command}: writable replacement failed: {}",
                String::from_utf8_lossy(&writable.stderr)
            ));
        }
        let new_output = temp.path().join("new-output");
        let mut new_args = args.clone();
        *new_args.last_mut().unwrap() = path(&new_output);
        let new_result = run(&new_args);
        if !new_result.status.success() || fs::read(&new_output).unwrap_or_default().is_empty() {
            failures.push(format!(
                "{command}: new output failed: {}",
                String::from_utf8_lossy(&new_result.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(target_os = "macos")]
fn set_acl(output: &Path) -> bool {
    let result = Command::new("chmod")
        .args(["+a", "everyone allow read"])
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    true
}

#[cfg(target_os = "macos")]
fn read_acl(output: &Path) -> String {
    let result = Command::new("ls").arg("-le").arg(output).output().unwrap();
    assert!(result.status.success());
    let listing = String::from_utf8(result.stdout).unwrap();
    let acl = listing
        .lines()
        .find(|line| line.contains("everyone allow read"))
        .unwrap();
    acl.trim().to_owned()
}

#[cfg(target_os = "linux")]
#[test]
fn replacement_does_not_inherit_new_parent_default_acl() {
    let temp = tempfile::tempdir_in(support::secure_temp_root()).unwrap();
    let source = temp.path().join("source.txt");
    let packed = temp.path().join("source.qzt");
    let output = temp.path().join("old.txt");
    fs::write(&source, b"new content\n").unwrap();
    assert!(
        run(&["pack", path(&source), "-o", path(&packed)])
            .status
            .success()
    );
    fs::write(&output, b"old content").unwrap();
    let result = match Command::new("setfacl")
        .args(["-m", "d:u:424242:r--"])
        .arg(temp.path())
        .output()
    {
        Ok(result) => result,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "Linux default ACL test skipped: setfacl is not installed; the dedicated CI job installs acl"
            );
            return;
        }
        Err(error) => panic!("run setfacl: {error}"),
    };
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let before = read_acl(&output);

    let result = run(&["export", path(&packed), "-o", path(&output)]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read(&output).unwrap(), fs::read(&source).unwrap());
    assert_eq!(read_acl(&output), before);
}

#[cfg(target_os = "linux")]
fn set_acl(output: &Path) -> bool {
    let result = match Command::new("setfacl")
        .args(["-m", "u:424242:r--"])
        .arg(output)
        .output()
    {
        Ok(result) => result,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "Linux ACL test skipped: setfacl is not installed; the dedicated CI job installs acl"
            );
            return false;
        }
        Err(error) => panic!("run setfacl: {error}"),
    };
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    true
}

#[cfg(target_os = "linux")]
fn read_acl(output: &Path) -> String {
    let result = Command::new("getfacl")
        .arg("-c")
        .arg(output)
        .output()
        .expect("install the acl package for the Linux ACL regression test");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[cfg(windows)]
fn set_acl(output: &Path) -> bool {
    let user = std::env::var("USERNAME").unwrap();
    let grant = format!("{user}:(F)");
    let result = Command::new("icacls")
        .arg(output)
        .args(["/inheritance:r", "/grant:r", &grant])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    true
}

#[cfg(windows)]
fn read_acl(output: &Path) -> String {
    let result = Command::new("icacls").arg(output).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}
