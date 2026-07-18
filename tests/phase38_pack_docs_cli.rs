use std::fs;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qzt"))
        .args(args)
        .output()
        .expect("qzt command should run")
}

#[test]
fn pack_docs_round_trips_in_argument_order_with_prefixed_basenames() {
    let base = std::env::temp_dir().join(format!("qzt-38-pack-docs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("fixture directory");
    let first = base.join("first.txt");
    let second = base.join("second.txt");
    let output = base.join("bundle.qzt");
    fs::write(&first, b"first\n").expect("write first");
    fs::write(&second, b"second\n").expect("write second");

    let packed = run(&[
        "pack-docs",
        second.to_str().unwrap(),
        first.to_str().unwrap(),
        "--doc-id-prefix",
        "logs/",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert!(
        packed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&packed.stderr)
    );

    let listed = run(&["docs", output.to_str().unwrap()]);
    assert!(listed.status.success());
    let listed = String::from_utf8(listed.stdout).expect("docs output utf-8");
    let rows: Vec<&str> = listed.lines().collect();
    assert!(
        rows[1].starts_with("logs/second.txt\t0\t7\t1\t1\t"),
        "{listed}"
    );
    assert!(
        rows[2].starts_with("logs/first.txt\t7\t6\t2\t1\t"),
        "{listed}"
    );

    let extracted = run(&["doc", output.to_str().unwrap(), "logs/second.txt"]);
    assert!(extracted.status.success());
    assert_eq!(extracted.stdout, b"second\n");
    let verified = run(&["verify", output.to_str().unwrap(), "--deep"]);
    assert!(
        verified.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&verified.stderr)
    );

    let _ = fs::remove_dir_all(base);
}

#[test]
fn pack_docs_rejects_duplicate_basenames_without_leaving_output_artifacts() {
    let base = std::env::temp_dir().join(format!("qzt-38-pack-docs-dupe-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let one = base.join("one");
    let two = base.join("two");
    fs::create_dir_all(&one).expect("first directory");
    fs::create_dir_all(&two).expect("second directory");
    let first = one.join("same.txt");
    let second = two.join("same.txt");
    let output = base.join("bundle.qzt");
    fs::write(&first, b"one\n").expect("write first");
    fs::write(&second, b"two\n").expect("write second");

    let packed = run(&[
        "pack-docs",
        first.to_str().unwrap(),
        second.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(packed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&packed.stderr).contains("same.txt"));
    assert!(!output.exists());
    assert!(!std::path::PathBuf::from(format!("{}.tmp", output.display())).exists());

    let _ = fs::remove_dir_all(base);
}

#[test]
fn pack_docs_memory_profile_is_available_and_help_explains_memory_cost() {
    let base = std::env::temp_dir().join(format!("qzt-38-pack-docs-memory-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("fixture directory");
    let input = base.join("input.txt");
    let output = base.join("bundle.qzt");
    fs::write(&input, b"memory profile\n").expect("write input");

    let packed = run(&[
        "pack-docs",
        input.to_str().unwrap(),
        "--profile",
        "memory",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert!(
        packed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&packed.stderr)
    );
    assert!(
        run(&["verify", output.to_str().unwrap(), "--deep"])
            .status
            .success()
    );

    let help = run(&["pack-docs", "--help"]);
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("total input size"));
    let _ = fs::remove_dir_all(base);
}

#[test]
fn pack_docs_memory_uses_bounded_default_chunks_and_honors_explicit_sizes() {
    let base = std::env::temp_dir().join(format!("qzt-38-memory-chunks-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("fixture directory");
    let large = base.join("large.txt");
    let small = base.join("small.txt");
    let output = base.join("bundle.qzt");
    fs::write(&large, vec![b'x'; 10 * 1024 * 1024]).expect("write large input");
    fs::write(&small, vec![b'y'; 4 * 1024]).expect("write small input");

    let packed = run(&[
        "pack-docs",
        large.to_str().unwrap(),
        small.to_str().unwrap(),
        "--profile",
        "memory",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert!(
        packed.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&packed.stderr)
    );
    let details = qzt::open_skeleton_details(&fs::read(&output).expect("read container"))
        .expect("container details");
    assert_eq!(details.metadata.target_chunk_size, 256 * 1024);
    assert_eq!(details.metadata.max_chunk_size, 2 * 1024 * 1024);
    assert!(
        details
            .chunk_entries
            .iter()
            .all(|entry| entry.uncompressed_size <= 2 * 1024 * 1024)
    );
    let range = run(&["range", output.to_str().unwrap(), "--bytes", "4096:8192"]);
    assert!(range.status.success());
    assert_eq!(range.stdout, vec![b'x'; 4 * 1024]);

    let explicit_output = base.join("explicit.qzt");
    let explicit = run(&[
        "pack-docs",
        small.to_str().unwrap(),
        "--profile",
        "memory",
        "--chunk-size",
        "4096",
        "--max-chunk-size",
        "8192",
        "-o",
        explicit_output.to_str().unwrap(),
    ]);
    assert!(
        explicit.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&explicit.stderr)
    );
    let explicit_details =
        qzt::open_skeleton_details(&fs::read(&explicit_output).expect("read container"))
            .expect("explicit container details");
    assert_eq!(explicit_details.metadata.target_chunk_size, 4096);
    assert_eq!(explicit_details.metadata.max_chunk_size, 8192);

    let max_only_output = base.join("max-only.qzt");
    let max_only = run(&[
        "pack-docs",
        small.to_str().unwrap(),
        "--profile",
        "memory",
        "--max-chunk-size",
        "131072",
        "-o",
        max_only_output.to_str().unwrap(),
    ]);
    assert!(
        max_only.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&max_only.stderr)
    );
    let max_only_details =
        qzt::open_skeleton_details(&fs::read(&max_only_output).expect("read max-only container"))
            .expect("max-only container details");
    assert_eq!(max_only_details.metadata.target_chunk_size, 131_072);
    assert_eq!(max_only_details.metadata.max_chunk_size, 131_072);

    let target_only_output = base.join("target-only.qzt");
    let target_only = run(&[
        "pack-docs",
        small.to_str().unwrap(),
        "--profile",
        "memory",
        "--chunk-size",
        "4194304",
        "-o",
        target_only_output.to_str().unwrap(),
    ]);
    assert!(
        target_only.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&target_only.stderr)
    );
    let target_only_details = qzt::open_skeleton_details(
        &fs::read(&target_only_output).expect("read target-only container"),
    )
    .expect("target-only container details");
    assert_eq!(target_only_details.metadata.target_chunk_size, 4_194_304);
    assert_eq!(target_only_details.metadata.max_chunk_size, 4_194_304);

    let _ = fs::remove_dir_all(base);
}

#[test]
fn pack_memory_profile_error_explains_the_document_index_requirement() {
    let base = std::env::temp_dir().join(format!("qzt-110-memory-message-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).expect("fixture directory");
    let input = base.join("input.txt");
    let output = base.join("output.qzt");
    fs::write(&input, b"memory profile\n").expect("write input");

    let result = run(&[
        "pack",
        input.to_str().unwrap(),
        "--profile",
        "memory",
        "-o",
        output.to_str().unwrap(),
    ]);
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("DocumentIndex"), "{stderr}");
    assert!(
        stderr.contains("pack_bytes_with_memory_profile"),
        "{stderr}"
    );
    let _ = fs::remove_dir_all(base);
}
