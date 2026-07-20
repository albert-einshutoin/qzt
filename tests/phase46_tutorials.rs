const README_EN: &str = include_str!("../README.md");
const README_JA: &str = include_str!("../README.ja.md");
const SYSTEMD_SERVICE: &str = include_str!("../docs/guides/examples/qzt-verify.service");
const SYSTEMD_TIMER: &str = include_str!("../docs/guides/examples/qzt-verify.timer");
const SYSTEMD_ALERT: &str = include_str!("../docs/guides/examples/qzt-verify-alert@.service");
const ACTIONS_WORKFLOW: &str = include_str!("../docs/guides/examples/qzt-artifact-workflow.yml");

use serde_json::Value;
mod support;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct TutorialTempDir(PathBuf);

impl TutorialTempDir {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos();
        let path = crate::support::secure_temp_root()
            .join(format!("qzt-phase46-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).expect("tutorial temp directory must be created");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TutorialTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn qzt(directory: &Path, args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_qzt"));
    command.current_dir(directory).args(args);
    if stdin.is_some() {
        command.stdin(Stdio::piped());
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("tutorial command must start");
    if let Some(bytes) = stdin {
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(bytes)
            .expect("tutorial input must be written");
    }
    let output = child
        .wait_with_output()
        .expect("tutorial command must exit");
    assert!(
        output.status.success(),
        "qzt {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("tutorial command must emit valid JSON")
}

const GUIDES: [(&str, &str); 6] = [
    (
        "log-preservation.md",
        include_str!("../docs/guides/log-preservation.md"),
    ),
    (
        "log-preservation.ja.md",
        include_str!("../docs/guides/log-preservation.ja.md"),
    ),
    (
        "artifact-fixation.md",
        include_str!("../docs/guides/artifact-fixation.md"),
    ),
    (
        "artifact-fixation.ja.md",
        include_str!("../docs/guides/artifact-fixation.ja.md"),
    ),
    (
        "search-operations.md",
        include_str!("../docs/guides/search-operations.md"),
    ),
    (
        "search-operations.ja.md",
        include_str!("../docs/guides/search-operations.ja.md"),
    ),
];

#[test]
fn every_tutorial_declares_prerequisites_validation_and_limitations() {
    for (name, guide) in GUIDES {
        for required in [
            "qzt 0.1.0-pre.2",
            "15 minutes",
            "Limitations",
            "docs/CLI",
            "tutorial-validation.md",
        ] {
            assert!(guide.contains(required), "{name} is missing {required}");
        }
    }
}

#[test]
fn tutorials_close_the_three_real_product_journeys() {
    let logs = GUIDES[0].1;
    for required in [
        "qzt pack - -o daily.qzt",
        "qzt attest daily.qzt > \"$partial\"",
        "qzt verify daily.qzt --deep --format json",
        "qzt search daily.qzt",
        "qzt range daily.qzt --bytes",
        "OnFailure=qzt-verify-alert@%n.service",
        "log show --last 1d",
        "set -euo pipefail",
        "umask 077",
        "test ! -e \"$archive\" && test ! -e \"$partial\"",
        "test ! -e \"$attestation\" && test ! -e \"$partial\"",
        ".capped == false",
        ".incomplete_reason == null",
    ] {
        assert!(logs.contains(required), "log guide is missing {required}");
    }
    assert!(
        logs.contains("```sh\nset -euo pipefail\numask 077\nqzt verify daily.qzt"),
        "the standalone attestation publication block must fail closed"
    );
    assert!(
        logs.contains("```sh\nset -euo pipefail\nqzt sidecar-rebuild daily.qzt"),
        "the standalone search-to-range block must fail closed"
    );

    let artifacts = GUIDES[2].1;
    for required in [
        "qzt pack-docs report.txt metrics.csv run.log -o \"$archive_partial\"",
        "qzt attest \"$archive\" > \"$attestation_partial\"",
        "qzt docs run-1234.qzt --format json",
        "qzt doc run-1234.qzt report.txt -o restored-report.txt",
        "actions/upload-artifact",
        "persist-credentials: false",
        "run-1234.provenance.txt",
        "minisign -Vm run-1234.sha256 -p minisign.pub",
        "sha256sum -c run-1234.sha256",
        "workflow_run_attempt=$EXPECTED_RUN_ATTEMPT",
        "diff -u run-1234.attest.json \"$regenerated\"",
    ] {
        assert!(
            artifacts.contains(required),
            "artifact guide is missing {required}"
        );
    }

    let search = GUIDES[4].1;
    for required in [
        "qzt sidecar-rebuild archive.qzt --index token -o archive.token.qzi",
        "qzt sidecar-rebuild archive.qzt --index ngram --ngram 3 -o archive.ngram.qzi",
        "--max-candidates",
        "--max-decoded-bytes",
        "--max-results",
        "physical_decoded_bytes",
        "query_shorter_than_ngram_n",
        "co-occurrence",
        "ASCII case folding is implemented",
        "excludes QZI header/manifest overhead",
        "wc -c archive.token.qzi",
    ] {
        assert!(
            search.contains(required),
            "search guide is missing {required}"
        );
    }
}

#[test]
fn readme_use_cases_link_to_language_matching_tutorials() {
    for path in [
        "docs/guides/log-preservation.md",
        "docs/guides/artifact-fixation.md",
        "docs/guides/search-operations.md",
    ] {
        assert!(README_EN.contains(path), "English README is missing {path}");
    }
    for path in [
        "docs/guides/log-preservation.ja.md",
        "docs/guides/artifact-fixation.ja.md",
        "docs/guides/search-operations.ja.md",
    ] {
        assert!(
            README_JA.contains(path),
            "Japanese README is missing {path}"
        );
    }
}

#[test]
fn guides_do_not_claim_search_hits_are_physical_decode_ranges() {
    for (name, guide) in GUIDES {
        assert!(!guide.contains("decoded_bytes == byte_length"), "{name}");
        assert!(!guide.contains("only the hit bytes are decoded"), "{name}");
        assert!(!guide.contains("法的証明"), "{name}");
        assert!(
            !guide.contains("sidecar bytes divided by source bytes"),
            "{name}"
        );
    }
}

#[test]
fn embedded_operational_templates_match_validated_files() {
    for guide in [GUIDES[0].1, GUIDES[1].1] {
        for template in [SYSTEMD_SERVICE, SYSTEMD_TIMER, SYSTEMD_ALERT] {
            assert!(guide.contains(template.trim()), "systemd template drifted");
        }
    }
    for guide in [GUIDES[2].1, GUIDES[3].1] {
        assert!(
            guide.contains(ACTIONS_WORKFLOW.trim()),
            "GitHub Actions template drifted"
        );
    }

    for required in [
        "permissions:\n  contents: read",
        "actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10",
        "actions/upload-artifact@b7c566a772e6b6bfb58ed0dc250532a479d7789f",
        "jq -e '.verify.level == \"deep\" and",
        "ref: ${{ github.sha }}",
        "persist-credentials: false",
        "SOURCE_COMMIT: ${{ github.sha }}",
        "set -euo pipefail",
        "run-1234.qzt.partial",
        "run-1234.provenance.txt",
        "retention-days: 30",
    ] {
        assert!(ACTIONS_WORKFLOW.contains(required), "workflow: {required}");
    }
    assert!(SYSTEMD_SERVICE.contains("OnFailure=qzt-verify-alert@%n.service"));
    assert!(SYSTEMD_SERVICE.contains("qzt verify /archive/daily.qzt --deep --format json"));
    for hardening in [
        "TimeoutStartSec=30min",
        "MemoryMax=1G",
        "TasksMax=32",
        "CapabilityBoundingSet=",
        "PrivateNetwork=true",
        "RequiresMountsFor=/archive",
    ] {
        assert!(SYSTEMD_SERVICE.contains(hardening), "service: {hardening}");
    }
    assert!(SYSTEMD_TIMER.contains("OnCalendar=daily"));
    assert!(SYSTEMD_TIMER.contains("RandomizedDelaySec=5min"));
    assert!(SYSTEMD_ALERT.contains("ExecStart=/usr/local/sbin/qzt-verify-alert %i"));
    assert!(SYSTEMD_ALERT.contains("User=qzt-alert"));
    assert!(SYSTEMD_ALERT.contains("NoNewPrivileges=true"));
}

#[test]
fn published_validation_numbers_match_the_live_cli_journeys() {
    let directory = TutorialTempDir::new();
    let root = directory.path();

    // These bytes are the source of both language variants and the validation
    // record. Execute them here so documentation cannot drift into invented data.
    let daily = concat!(
        "2026-07-19T01:00:00Z INFO service=api request_id=req-001 status=200\n",
        "2026-07-19T01:01:00Z WARN service=api request_id=req-002 retry=1\n",
        "2026-07-19T01:02:00Z ERROR service=api request_id=req-003 incident=INC-4242 status=503\n",
        "2026-07-19T01:03:00Z INFO service=api request_id=req-004 status=200\n",
    );
    qzt(
        root,
        &["pack", "-", "-o", "daily.qzt"],
        Some(daily.as_bytes()),
    );
    let verify = json(&qzt(
        root,
        &["verify", "daily.qzt", "--deep", "--format", "json"],
        None,
    ));
    assert_eq!(verify["decoded_bytes"], 288);
    qzt(
        root,
        &[
            "sidecar-rebuild",
            "daily.qzt",
            "--index",
            "token",
            "-o",
            "daily.qzi",
        ],
        None,
    );
    let hit = json(&qzt(
        root,
        &[
            "search",
            "daily.qzt",
            "INC-4242",
            "--sidecar",
            "daily.qzi",
            "--format",
            "json",
        ],
        None,
    ));
    assert_eq!(hit["hits"][0]["logical_offset"], 200);
    assert_eq!(hit["hits"][0]["byte_length"], 8);
    assert_eq!(hit["metrics"]["decoded_bytes"], 87);
    assert_eq!(hit["metrics"]["physical_decoded_bytes"], 288);
    assert_eq!(
        qzt(root, &["range", "daily.qzt", "--bytes", "200:208"], None).stdout,
        b"INC-4242"
    );

    fs::write(
        root.join("report.txt"),
        "Pipeline run 1234\nResult: PASS\nDataset checksum recorded.\n",
    )
    .unwrap();
    fs::write(
        root.join("metrics.csv"),
        "metric,value\nrows,1200\nerrors,0\n",
    )
    .unwrap();
    fs::write(
        root.join("run.log"),
        concat!(
            "2026-07-19T02:00:00Z pipeline start\n",
            "2026-07-19T02:03:00Z pipeline complete status=PASS\n",
        ),
    )
    .unwrap();
    qzt(
        root,
        &[
            "pack-docs",
            "report.txt",
            "metrics.csv",
            "run.log",
            "-o",
            "run-1234.qzt",
        ],
        None,
    );
    let documents = json(&qzt(
        root,
        &["docs", "run-1234.qzt", "--format", "json"],
        None,
    ));
    let spans: Vec<(&str, u64, u64)> = documents["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| {
            (
                document["doc_id"].as_str().unwrap(),
                document["logical_offset"].as_u64().unwrap(),
                document["byte_length"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        spans,
        vec![
            ("report.txt", 0, 58),
            ("metrics.csv", 58, 32),
            ("run.log", 90, 87),
        ]
    );
    assert_eq!(
        qzt(root, &["doc", "run-1234.qzt", "report.txt"], None).stdout,
        fs::read(root.join("report.txt")).unwrap()
    );
    let attestation_a = qzt(root, &["attest", "run-1234.qzt"], None);
    let attestation_b = qzt(root, &["attest", "run-1234.qzt"], None);
    assert_eq!(attestation_a.stdout, attestation_b.stdout);

    let archive = concat!(
        "2026-07-19T03:00:00Z INFO tenant=alpha request_id=req-100 action=login status=200\n",
        "2026-07-19T03:00:01Z ERROR tenant=alpha request_id=req-101 action=checkout incident=INC-9001 status=503\n",
        "2026-07-19T03:00:02Z WARN tenant=beta request_id=req-102 action=checkout retry=1\n",
        "2026-07-19T03:00:03Z ERROR tenant=beta request_id=req-103 action=payment incident=INC-9002 status=500\n",
        "2026-07-19T03:00:04Z INFO tenant=alpha request_id=req-104 action=logout status=200\n",
    );
    qzt(
        root,
        &["pack", "-", "-o", "archive.qzt"],
        Some(archive.as_bytes()),
    );
    for (kind, output) in [
        ("token", "archive.token.qzi"),
        ("ngram", "archive.ngram.qzi"),
    ] {
        let mut args = vec!["sidecar-rebuild", "archive.qzt", "--index", kind];
        if kind == "ngram" {
            args.extend(["--ngram", "3"]);
        }
        args.extend(["-o", output]);
        qzt(root, &args, None);
    }
    let bounded = json(&qzt(
        root,
        &[
            "search",
            "archive.qzt",
            "ERROR",
            "--sidecar",
            "archive.token.qzi",
            "--max-candidates",
            "100",
            "--max-decoded-bytes",
            "16MiB",
            "--max-results",
            "1",
            "--format",
            "json",
        ],
        None,
    ));
    assert_eq!(bounded["capped"], true);
    assert_eq!(bounded["metrics"]["candidate_granules"], 2);
    assert_eq!(bounded["metrics"]["decoded_bytes"], 104);
    assert_eq!(bounded["metrics"]["physical_decoded_bytes"], 452);
    assert_eq!(bounded["metrics"]["verified_matches"], 1);
    let lowercase = json(&qzt(
        root,
        &[
            "search",
            "archive.qzt",
            "error",
            "--sidecar",
            "archive.token.qzi",
            "--format",
            "json",
        ],
        None,
    ));
    assert_eq!(lowercase["metrics"]["verified_matches"], 2);
    let payload_bytes = lowercase["metrics"]["index_size_bytes"].as_u64().unwrap();
    let on_disk_bytes = fs::metadata(root.join("archive.token.qzi")).unwrap().len();
    assert!(
        on_disk_bytes > payload_bytes,
        "QZI header/manifest bytes must not be described as index payload"
    );
    let incomplete = json(&qzt(
        root,
        &[
            "search",
            "archive.qzt",
            "IN",
            "--sidecar",
            "archive.ngram.qzi",
            "--format",
            "json",
        ],
        None,
    ));
    assert_eq!(
        incomplete["incomplete_reason"],
        "query_shorter_than_ngram_n"
    );
}
