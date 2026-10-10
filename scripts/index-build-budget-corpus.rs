//! Measurement-only adapter: compile the unchanged existing generator, not QZT.
mod error {
    #[derive(Debug)]
    pub enum QztError { ResourceLimitExceeded }
    pub type Result<T> = std::result::Result<T, QztError>;
}
#[path = "../src/corpus.rs"]
mod corpus;

fn main() {
    use std::io::Read;
    let mut input = String::new();
    std::io::stdin().take(4097).read_to_string(&mut input).expect("UTF-8 input");
    assert!(input.len() <= 4096, "input exceeds 4096 bytes");
    let fields: Vec<&str> = input.lines().collect();
    assert_eq!(fields.len(), 4, "stdin: KIND, BYTES, SEED, OUTPUT on four lines");
    let kind = match fields[0] {
        "C2" => corpus::CorpusKind::C2Logs,
        "C4" => corpus::CorpusKind::C4CodeStructured,
        "C5" => corpus::CorpusKind::C5Multilingual,
        _ => panic!("only C2/C4/C5 are in scope"),
    };
    let target_bytes: usize = fields[1].parse().expect("bytes");
    assert!((1..=104_857_600).contains(&target_bytes));
    let seed = fields[2].parse().expect("seed");
    let mut bytes = corpus::generate_validation_corpus(
        kind, corpus::ValidationCorpusOptions { seed, target_bytes },
    ).expect("corpus");
    bytes.extend_from_slice("\nqztpre6needleunique 東京pre6唯一😀\n".as_bytes());
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new().write(true).create_new(true)
        .open(fields[3]).expect("fresh output required");
    output.write_all(&bytes).expect("write corpus");
}
