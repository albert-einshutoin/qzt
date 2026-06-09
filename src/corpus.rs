use crate::error::{QztError, Result};

/// Validation corpus class from `docs/QZT_v0.1_Validation_Corpus.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum CorpusKind {
    /// C1 conversation transcripts.
    C1Conversation,
    /// C2 application and server logs.
    C2Logs,
    /// C3 prose documents and knowledge base.
    C3Prose,
    /// C4 source code and structured data.
    C4CodeStructured,
    /// C5 multilingual / CJK / emoji.
    C5Multilingual,
    /// C6 adversarial / pathological.
    C6Adversarial,
}

impl CorpusKind {
    /// Returns all validation corpus kinds in stable order.
    #[must_use]
    pub const fn all() -> [Self; 6] {
        [
            Self::C1Conversation,
            Self::C2Logs,
            Self::C3Prose,
            Self::C4CodeStructured,
            Self::C5Multilingual,
            Self::C6Adversarial,
        ]
    }

    /// Stable corpus identifier.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::C1Conversation => "C1",
            Self::C2Logs => "C2",
            Self::C3Prose => "C3",
            Self::C4CodeStructured => "C4",
            Self::C5Multilingual => "C5",
            Self::C6Adversarial => "C6",
        }
    }
}

/// Deterministic corpus generator options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationCorpusOptions {
    /// Seed controlling generated values.
    pub seed: u64,
    /// Approximate output size in bytes.
    pub target_bytes: usize,
}

impl Default for ValidationCorpusOptions {
    fn default() -> Self {
        Self {
            seed: 0x515a_5401,
            target_bytes: 32 * 1024,
        }
    }
}

/// Generates a deterministic validation corpus.
pub fn generate_validation_corpus(
    kind: CorpusKind,
    options: ValidationCorpusOptions,
) -> Result<Vec<u8>> {
    if options.target_bytes == 0 {
        return Err(QztError::ResourceLimitExceeded);
    }

    let mut rng = DeterministicRng::new(options.seed ^ kind_seed(kind));
    let mut output = Vec::with_capacity(options.target_bytes);
    while output.len() < options.target_bytes {
        match kind {
            CorpusKind::C1Conversation => push_conversation(&mut output, &mut rng),
            CorpusKind::C2Logs => push_log(&mut output, &mut rng),
            CorpusKind::C3Prose => push_prose(&mut output, &mut rng),
            CorpusKind::C4CodeStructured => push_code_structured(&mut output, &mut rng),
            CorpusKind::C5Multilingual => push_multilingual(&mut output, &mut rng),
            CorpusKind::C6Adversarial => push_adversarial(&mut output, &mut rng),
        }
    }
    output.truncate(options.target_bytes);
    while std::str::from_utf8(&output).is_err() {
        output.pop();
    }
    if output.is_empty() {
        output.extend_from_slice(b"\n");
    }
    Ok(output)
}

fn push_conversation(output: &mut Vec<u8>, rng: &mut DeterministicRng) {
    let turn = rng.next_u32() % 10_000;
    output.extend_from_slice(
        format!(
            "## conversation-{turn}\nuser: explain qzt evidence ref {turn}\nassistant: restore byte ranges, verify blake3, cite doc_id.\n```rust\nlet range = {turn}..{};\n```\n",
            turn + 128
        )
        .as_bytes(),
    );
}

fn push_log(output: &mut Vec<u8>, rng: &mut DeterministicRng) {
    let index = rng.next_u32() % 1_000_000;
    let level = if index.is_multiple_of(257) {
        "error"
    } else {
        "info"
    };
    output.extend_from_slice(
        format!(
            "2026-06-08T12:{:02}:{:02}Z level={level} service=qzt component=reader request_id=req-{index:06} message=bounded range restore rare-token-{}\n",
            index % 60,
            (index / 60) % 60,
            index.is_multiple_of(997)
        )
        .as_bytes(),
    );
}

fn push_prose(output: &mut Vec<u8>, rng: &mut DeterministicRng) {
    let variants = [
        "Cold evidence containers preserve original bytes while allowing small verified slices to be restored.",
        "A memory system can cite a document without trusting a mutable search index.",
        "The format separates immutable core data from rebuildable sidecars.",
    ];
    output.extend_from_slice(variants[rng.index(variants.len())].as_bytes());
    output.extend_from_slice(b"\n\n");
}

fn push_code_structured(output: &mut Vec<u8>, rng: &mut DeterministicRng) {
    let id = rng.next_u32() % 10_000;
    output.extend_from_slice(
        format!(
            "fn case_{id}() -> &'static str {{\n    r#\"{{\"id\":{id},\"kind\":\"qzt\",\"ok\":true}}\"#\n}}\n"
        )
        .as_bytes(),
    );
}

fn push_multilingual(output: &mut Vec<u8>, rng: &mut DeterministicRng) {
    let lines = [
        "東京の証跡を復元します。😀\n",
        "证据范围必须按字节验证。\n",
        "라인 단위 검색과 범위 복원이 함께 동작합니다。\n",
        "emoji 🔎📄✅ keep UTF-8 boundaries intact.\n",
    ];
    output.extend_from_slice(lines[rng.index(lines.len())].as_bytes());
}

fn push_adversarial(output: &mut Vec<u8>, rng: &mut DeterministicRng) {
    match rng.next_u32() % 4 {
        0 => output.extend(std::iter::repeat_n(b'a', 1024)),
        1 => output.extend_from_slice(b"\r\nmixed\nnewline\r\n"),
        2 => {
            for _ in 0..128 {
                output.push(33 + (rng.next_u32() % 90) as u8);
            }
            output.push(b'\n');
        }
        _ => output.extend_from_slice(b"single-line-without-newline-"),
    }
}

fn kind_seed(kind: CorpusKind) -> u64 {
    match kind {
        CorpusKind::C1Conversation => 0xC1,
        CorpusKind::C2Logs => 0xC2,
        CorpusKind::C3Prose => 0xC3,
        CorpusKind::C4CodeStructured => 0xC4,
        CorpusKind::C5Multilingual => 0xC5,
        CorpusKind::C6Adversarial => 0xC6,
    }
}

struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed | 1 }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn index(&mut self, len: usize) -> usize {
        (self.next_u32() as usize) % len
    }
}
