use std::fs;
use std::path::{Path, PathBuf};

fn rust_test_files() -> Vec<PathBuf> {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    fs::read_dir(tests)
        .expect("tests directory should be readable")
        .map(|entry| entry.expect("test entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect()
}

#[test]
fn reusable_test_helpers_have_one_implementation() {
    let forbidden_local_definitions = [
        "fn small_chunk_options()",
        "fn output_success(",
        "fn assert_success(",
        "struct CountingReadAt",
        "fn pack(input: &[u8])",
        "fn pack(input: &[u8], target: usize, max: usize)",
    ];

    for path in rust_test_files() {
        if path
            .file_name()
            .is_some_and(|name| name == "test_support_consolidation.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path).expect("Rust test source should be readable");
        for definition in forbidden_local_definitions {
            assert!(
                !source.contains(definition),
                "{} must use tests/support/mod.rs instead of defining {definition}",
                path.display()
            );
        }
    }
}
