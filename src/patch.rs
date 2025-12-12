use crate::span::SourceSpan;

/// Apply a single textual replacement to `code` based on `span`.
///
/// The `span` offsets are byte indices into `code`.
pub fn apply_span_patch(code: &str, span: &SourceSpan, replacement: &str) -> String {
    let start = span.start as usize;
    let end = span.end as usize;

    debug_assert!(
        start <= end && end <= code.len(),
        "span [{start}, {end}) is out of bounds for code length {}",
        code.len()
    );

    let mut out = String::with_capacity(
        code.len() + replacement.len().saturating_sub(end.saturating_sub(start)),
    );

    out.push_str(&code[..start]);
    out.push_str(replacement);
    out.push_str(&code[end..]);

    out
}

/// Apply a replacement and, in debug builds, verify that the original slice matches `expected_original`.
///
/// This is a helper that is useful together with discovered mutants, where the span is expected
/// to cover a specific operator or snippet.
pub fn apply_checked_patch(
    code: &str,
    span: &SourceSpan,
    expected_original: &str,
    replacement: &str,
) -> String {
    let start = span.start as usize;
    let end = span.end as usize;

    debug_assert_eq!(
        &code[start..end],
        expected_original,
        "span [{start}, {end}) does not match expected original snippet"
    );

    apply_span_patch(code, span, replacement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn span_for_substr(code: &str, needle: &str) -> SourceSpan {
        let start = code
            .find(needle)
            .unwrap_or_else(|| panic!("needle {:?} not found in {:?}", needle, code));
        let end = start + needle.len();
        SourceSpan {
            file: PathBuf::from("dummy.nr"),
            start: start as u32,
            end: end as u32,
        }
    }

    #[test]
    fn patch_middle_of_string() {
        let code = "assert(x == 0);";
        let span = span_for_substr(code, "==");

        let patched = apply_span_patch(code, &span, "!=");
        assert_eq!(patched, "assert(x != 0);");
    }

    #[test]
    fn patch_at_start() {
        let code = "== x";
        let span = span_for_substr(code, "==");

        let patched = apply_span_patch(code, &span, "!=");
        assert_eq!(patched, "!= x");
    }

    #[test]
    fn patch_at_end() {
        let code = "x ==";
        let span = span_for_substr(code, "==");

        let patched = apply_span_patch(code, &span, "!=");
        assert_eq!(patched, "x !=");
    }

    #[test]
    fn checked_patch_verifies_original_slice() {
        let code = "constrain x < y;";
        let span = span_for_substr(code, "<");

        let patched = apply_checked_patch(code, &span, "<", ">=");
        assert_eq!(patched, "constrain x >= y;");
    }
}
