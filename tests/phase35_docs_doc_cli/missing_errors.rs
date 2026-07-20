use std::fs;

use super::{no_index_container, run, two_doc_container};

#[test]
fn doc_reports_missing_index_and_missing_id_separately() {
    let base = crate::support::secure_temp_root().join(format!(
        "qzt-35-doc-distinguish-missing-{}",
        std::process::id()
    ));
    let _ = fs::create_dir_all(&base);
    let no_index_path = base.join("noidx.qzt");
    let indexed_path = base.join("two.qzt");
    fs::write(&no_index_path, no_index_container()).expect("write no-index fixture");
    fs::write(&indexed_path, two_doc_container()).expect("write indexed fixture");

    let missing_index = run(&["doc", no_index_path.to_str().unwrap(), "some-id"]);
    let missing_doc = run(&["doc", indexed_path.to_str().unwrap(), "missing-doc-id"]);

    assert_eq!(
        missing_index.status.code(),
        Some(1),
        "missing Document Index must keep exit code 1"
    );
    assert_eq!(
        missing_doc.status.code(),
        Some(1),
        "missing doc_id must keep exit code 1"
    );

    let missing_index_stderr = String::from_utf8_lossy(&missing_index.stderr);
    let missing_doc_stderr = String::from_utf8_lossy(&missing_doc.stderr);
    assert!(
        missing_index_stderr.contains("no Document Index in this container"),
        "missing-index stderr must tell the user to add a Document Index: {missing_index_stderr}"
    );
    assert!(
        missing_doc_stderr.contains("doc_id not found"),
        "missing-doc stderr must name the missing doc_id: {missing_doc_stderr}"
    );
    assert_ne!(
        missing_index_stderr, missing_doc_stderr,
        "missing-index and missing-doc errors must not collapse to the same stderr"
    );

    let _ = fs::remove_dir_all(base);
}
