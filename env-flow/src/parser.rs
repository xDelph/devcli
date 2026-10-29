/// Robust `.env` file parser.
///
/// Handles:
/// - `KEY=value`, `export KEY=value`
/// - Single-quoted (no interpolation), double-quoted (escape sequences)
/// - Unquoted values with inline `# comment` (only when preceded by whitespace)
/// - `URL=https://x.com#frag` — `#` without leading space is NOT a comment
/// - Split on FIRST `=` only (`DATABASE_URL=postgres://h/db?a=1&b=2`)
/// - Escape sequences in double-quoted values (`\n`, `\t`, `\r`, `\\`, `\"`)
/// - Multiline quoted values, and `# comment` after a closing quote
/// - Quotes only matter at the start of a value (`PW=ab"cd` is literal)
/// - CRLF → LF normalisation, UTF-8 BOM stripping
/// - Strict mode: malformed lines → `Error::Parse`; lenient mode: skip with `tracing::warn!`
use std::path::Path;

use crate::{types::ParsedEntry, Error, Result};

pub struct ParsedFile {
    pub entries: Vec<ParsedEntry>,
}

/// Parse the contents of a `.env` file.
///
/// `path` is used only for error messages.
/// `strict` controls whether malformed lines produce errors (true) or warnings (false).
pub fn parse(content: &str, path: &Path, strict: bool) -> Result<ParsedFile> {
    // Strip UTF-8 BOM if present
    let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    // Normalise CRLF → LF
    let normalised: String = content.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalised.split('\n').collect();

    let malformed = |line: usize, message: String| -> Result<()> {
        if strict {
            Err(Error::Parse {
                path: path.to_path_buf(),
                line,
                message,
            })
        } else {
            tracing::warn!(path = %path.display(), line, "{}", message);
            Ok(())
        }
    };

    let mut entries = Vec::new();
    let mut next = 0usize;

    while next < lines.len() {
        let line_number = next + 1;
        let line = lines[next];
        next += 1;

        let trimmed = line.trim();

        // Skip empty lines and comments. A comment is never the start of a
        // multi-line value, whatever quotes it contains.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Strip optional `export ` prefix
        let trimmed = trimmed
            .strip_prefix("export ")
            .map(|s| s.trim_start())
            .unwrap_or(trimmed);

        // Find first `=`
        let Some(eq_pos) = trimmed.find('=') else {
            malformed(line_number, format!("missing '=' in line: {trimmed}"))?;
            continue;
        };

        let key = trimmed[..eq_pos].trim();
        if key.is_empty() {
            malformed(line_number, format!("empty key in line: {trimmed}"))?;
            continue;
        }

        let rest = trimmed[eq_pos + 1..].trim_start();

        let (raw_value, interpolate) = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                match read_quoted(quote, &rest[1..], &lines, &mut next) {
                    Some(quoted) => quoted,
                    None => {
                        // No closing quote anywhere: do not swallow the rest of
                        // the file, read this line as a plain value.
                        malformed(line_number, format!("unterminated {quote} quote"))?;
                        let value = strip_inline_comment(&rest[1..]).trim_end().to_string();
                        (value, quote == '"')
                    }
                }
            }
            _ => {
                // Unquoted: strip inline comment (only ` #` counts as a marker)
                let value = strip_inline_comment(rest);
                (value.trim_end().to_string(), true)
            }
        };

        entries.push(ParsedEntry {
            key: key.to_string(),
            raw_value,
            line_number,
            interpolate,
        });
    }

    Ok(ParsedFile { entries })
}

/// Read a quoted value whose opening `quote` has been consumed; `first` is the
/// rest of the line after it. The value may continue on following lines, which
/// are then consumed (`next` advances). Anything after the closing quote must
/// be a comment or whitespace and is dropped.
///
/// Returns `(value, should_interpolate)`, or `None` (with `next` untouched) when
/// the quote is never closed.
fn read_quoted(
    quote: char,
    first: &str,
    lines: &[&str],
    next: &mut usize,
) -> Option<(String, bool)> {
    let mut body = first.to_string();
    let mut cursor = *next;
    loop {
        if let Some(end) = closing_quote(&body, quote) {
            *next = cursor;
            let inner = &body[..end];
            return Some(if quote == '"' {
                (process_double_quote_escapes(inner), true)
            } else {
                (inner.to_string(), false)
            });
        }
        let line = lines.get(cursor)?;
        cursor += 1;
        body.push('\n');
        body.push_str(line);
    }
}

/// Byte offset of the quote closing `body`. Inside double quotes a quote
/// preceded by a backslash is escaped; single quotes have no escapes.
fn closing_quote(body: &str, quote: char) -> Option<usize> {
    let mut escaped = false;
    for (i, ch) in body.char_indices() {
        if quote == '"' && ch == '\\' && !escaped {
            escaped = true;
            continue;
        }
        if ch == quote && !escaped {
            return Some(i);
        }
        escaped = false;
    }
    None
}

/// Process `\n`, `\t`, `\r`, `\\`, `\"` escape sequences inside double-quoted strings.
fn process_double_quote_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some('"') => out.push('"'),
                Some('$') => out.push('$'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Strip an inline `# comment`.
/// Only strips if `#` is preceded by at least one whitespace character.
/// `URL=https://x.com#frag` → `https://x.com#frag` (no whitespace before `#`)
/// `KEY=value # comment`   → `value`
fn strip_inline_comment(s: &str) -> &str {
    // Walk backwards looking for ` #` or `\t#`
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && i > 0 && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return s[..i].trim_end();
        }
        i += 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn p() -> &'static Path {
        Path::new(".env")
    }

    fn parse_strict(s: &str) -> ParsedFile {
        parse(s, p(), true).expect("parse failed")
    }

    fn parse_lenient(s: &str) -> ParsedFile {
        parse(s, p(), false).expect("parse failed")
    }

    fn first(pf: &ParsedFile) -> &ParsedEntry {
        pf.entries.first().expect("no entries")
    }

    #[test]
    fn basic_key_value() {
        let pf = parse_strict("KEY=value\n");
        assert_eq!(first(&pf).key, "KEY");
        assert_eq!(first(&pf).raw_value, "value");
    }

    #[test]
    fn export_prefix() {
        let pf = parse_strict("export KEY=value\n");
        assert_eq!(first(&pf).key, "KEY");
        assert_eq!(first(&pf).raw_value, "value");
    }

    #[test]
    fn empty_value() {
        let pf = parse_strict("KEY=\n");
        assert_eq!(first(&pf).raw_value, "");
    }

    #[test]
    fn double_quoted() {
        let pf = parse_strict("KEY=\"hello world\"\n");
        assert_eq!(first(&pf).raw_value, "hello world");
        assert!(first(&pf).interpolate);
    }

    #[test]
    fn single_quoted_no_interpolation() {
        let pf = parse_strict("KEY='no ${escape}'\n");
        assert_eq!(first(&pf).raw_value, "no ${escape}");
        assert!(!first(&pf).interpolate);
    }

    #[test]
    fn escape_sequences_in_double_quotes() {
        let pf = parse_strict("KEY=\"line1\\nline2\"\n");
        assert_eq!(first(&pf).raw_value, "line1\nline2");
    }

    #[test]
    fn first_equals_only_split() {
        let pf = parse_strict("URL=postgres://h/db?a=1&b=2\n");
        assert_eq!(first(&pf).raw_value, "postgres://h/db?a=1&b=2");
    }

    #[test]
    fn inline_comment_stripped() {
        let pf = parse_strict("KEY=value # comment\n");
        assert_eq!(first(&pf).raw_value, "value");
    }

    #[test]
    fn hash_in_url_not_comment() {
        let pf = parse_strict("URL=https://x.com#frag\n");
        assert_eq!(first(&pf).raw_value, "https://x.com#frag");
    }

    #[test]
    fn comment_line_skipped() {
        let pf = parse_strict("# this is a comment\nKEY=val\n");
        assert_eq!(pf.entries.len(), 1);
        assert_eq!(first(&pf).key, "KEY");
    }

    #[test]
    fn empty_lines_skipped() {
        let pf = parse_strict("\n\nKEY=val\n\n");
        assert_eq!(pf.entries.len(), 1);
    }

    #[test]
    fn utf8_bom_stripped() {
        let content = "\u{FEFF}KEY=value\n";
        let pf = parse_strict(content);
        assert_eq!(first(&pf).key, "KEY");
    }

    #[test]
    fn crlf_normalised() {
        let pf = parse_strict("KEY=value\r\n");
        assert_eq!(first(&pf).raw_value, "value");
    }

    #[test]
    fn unicode_value() {
        let pf = parse_strict("GREETING=héllo wörld\n");
        assert_eq!(first(&pf).raw_value, "héllo wörld");
    }

    #[test]
    fn multiple_entries() {
        let pf = parse_strict("A=1\nB=2\nC=3\n");
        assert_eq!(pf.entries.len(), 3);
        assert_eq!(pf.entries[1].key, "B");
    }

    #[test]
    fn missing_equals_lenient_skips() {
        let pf = parse_lenient("BADLINE\nGOOD=ok\n");
        assert_eq!(pf.entries.len(), 1);
        assert_eq!(first(&pf).key, "GOOD");
    }

    #[test]
    fn missing_equals_strict_errors() {
        let result = parse("BADLINE\n", p(), true);
        assert!(result.is_err());
    }

    #[test]
    fn line_numbers_tracked() {
        let pf = parse_strict("A=1\nB=2\nC=3\n");
        assert_eq!(pf.entries[0].line_number, 1);
        assert_eq!(pf.entries[1].line_number, 2);
        assert_eq!(pf.entries[2].line_number, 3);
    }

    #[test]
    fn multiline_double_quoted() {
        let content = "CERT=\"-----BEGIN-----\nMIIE\n-----END-----\"\nOTHER=val\n";
        let pf = parse_strict(content);
        assert_eq!(pf.entries.len(), 2);
        assert!(pf.entries[0].raw_value.contains('\n'));
        assert_eq!(pf.entries[1].key, "OTHER");
    }

    fn value_of<'a>(pf: &'a ParsedFile, key: &str) -> &'a str {
        &pf.entries
            .iter()
            .find(|e| e.key == key)
            .unwrap_or_else(|| panic!("no {key}"))
            .raw_value
    }

    #[test]
    fn quoted_value_followed_by_inline_comment() {
        let pf = parse_strict("A=\"hello world\" # greeting\nB='raw $X'   # literal\n");
        assert_eq!(value_of(&pf, "A"), "hello world");
        assert_eq!(value_of(&pf, "B"), "raw $X");
    }

    #[test]
    fn quote_in_a_comment_does_not_swallow_following_lines() {
        let pf = parse_strict("# a 5\" screen\nA=1\nB=2\n");
        assert_eq!(pf.entries.len(), 2);
        assert_eq!(value_of(&pf, "A"), "1");
    }

    #[test]
    fn quote_inside_an_unquoted_value_is_literal() {
        let pf = parse_strict("PW=ab\"cd\nNEXT=1\n");
        assert_eq!(value_of(&pf, "PW"), "ab\"cd");
        assert_eq!(value_of(&pf, "NEXT"), "1");
    }

    #[test]
    fn escaped_quote_does_not_close_a_double_quoted_value() {
        let pf = parse_strict("A=\"say \\\"hi\\\"\" # c\n");
        assert_eq!(value_of(&pf, "A"), "say \"hi\"");
    }

    #[test]
    fn multiline_single_quoted_value() {
        let pf = parse_strict("KEY='line1\nline2'\nNEXT=1\n");
        assert_eq!(value_of(&pf, "KEY"), "line1\nline2");
        assert_eq!(value_of(&pf, "NEXT"), "1");
    }

    #[test]
    fn unterminated_quote_reads_one_line_in_lenient_mode() {
        let pf = parse_lenient("A=\"abc\nB=2\n");
        assert_eq!(value_of(&pf, "A"), "abc");
        assert_eq!(value_of(&pf, "B"), "2");
        assert!(parse("A=\"abc\nB=2\n", p(), true).is_err());
    }
}
