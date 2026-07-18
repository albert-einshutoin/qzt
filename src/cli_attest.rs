use std::fmt::Write as _;

use qzt::{Checksum, QztInfo, VerifyReport};

use crate::cli_json;

/// Data covered by one canonical QZT attestation.
pub struct Attestation<'a> {
    pub info: &'a QztInfo,
    pub original_checksum: &'a Checksum,
    pub container_checksum: Option<&'a Checksum>,
    pub final_file_size: u64,
    pub verify_report: &'a VerifyReport,
}

impl Attestation<'_> {
    /// Renders the frozen canonical JSON representation.
    #[must_use]
    pub fn render(&self) -> String {
        let mut output = String::with_capacity(512);
        let container_checksum = self
            .container_checksum
            .map_or_else(|| "null".to_owned(), format_checksum);

        // Why hand-assemble this JSON: attestation bytes are signed and compared
        // later, so every key below must remain in lexicographic order regardless
        // of serializer or map implementation changes.
        let _ = writeln!(
            output,
            concat!(
                "{{\"chunk_count\":{chunk_count},",
                "\"container_checksum\":{container_checksum},",
                "\"container_id\":\"{container_id}\",",
                "\"final_file_size\":{final_file_size},",
                "\"format\":\"qzt-0.1\",",
                "\"line_count\":{line_count},",
                "\"original_checksum\":{original_checksum},",
                "\"original_size\":{original_size},",
                "\"verify\":{{\"checked_chunks\":{checked_chunks},",
                "\"decoded_bytes\":{decoded_bytes},\"level\":\"{level}\"}}}}"
            ),
            chunk_count = self.info.chunk_count,
            container_checksum = container_checksum,
            container_id = cli_json::hex(&self.info.container_id),
            final_file_size = self.final_file_size,
            line_count = self.info.line_count,
            original_checksum = format_checksum(self.original_checksum),
            original_size = self.info.original_size,
            checked_chunks = self.verify_report.checked_chunks,
            decoded_bytes = self.verify_report.decoded_bytes,
            level = super::verify_level_as_str(self.verify_report.level),
        );
        output
    }
}

fn format_checksum(checksum: &Checksum) -> String {
    format!(
        "{{\"algorithm\":\"{}\",\"value\":\"{}\"}}",
        cli_json::escape(&checksum.algorithm),
        cli_json::hex(&checksum.value)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use qzt::VerifyLevel;

    #[test]
    fn absent_container_checksum_is_canonical_null() {
        let info = QztInfo {
            container_id: [0; 16],
            original_size: 0,
            chunk_count: 0,
            line_count: 0,
        };
        let checksum = Checksum::blake3(&[]);
        let report = VerifyReport {
            level: VerifyLevel::Deep,
            checked_chunks: 0,
            decoded_bytes: 0,
        };
        let rendered = Attestation {
            info: &info,
            original_checksum: &checksum,
            container_checksum: None,
            final_file_size: 0,
            verify_report: &report,
        }
        .render();

        assert!(rendered.contains("\"container_checksum\":null"));
        assert!(rendered.ends_with('\n'));
        assert_eq!(rendered.lines().count(), 1);
    }
}
