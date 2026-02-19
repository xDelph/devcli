/// Robust `.env` file parser.
///
/// Handles:
/// - `KEY=value`, `export KEY=value`
/// - Single-quoted (no interpolation), double-quoted (escape sequences)
/// - Unquoted values with inline `# comment` (only when preceded by whitespace)
/// - `URL=https://x.com#frag` — `#` without leading space is NOT a comment
/// - Split on FIRST `=` only (`DATABASE_URL=postgres://h/db?a=1&b=2`)
/// - Escape sequences in double-quoted values (`\n`, `\t`, `\r`, `\\`, `\"`)
/// - Multiline double-quoted values
/// - CRLF → LF normalisation, UTF-8 BOM stripping
/// - Strict mode: malformed lines → `Error::Parse`; lenient mode: skip with `tracing::warn!`
use std::path::Path;

use crate::{
    Error, Result,
    types::ParsedEntry,
};

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

    let mut entries = Vec::new();
    let mut chars = normalised.chars().peekable();
    let mut line_number: usize = 1;
    let mut line_start_line: usize; // line number at beginning of current statement

    while chars.peek().is_some() {
        line_start_line = line_number;
        // Collect one logical line (may span multiple physical lines inside "")
        let (raw_line, lines_consumed) = collect_logical_line(&mut chars);
        line_number += lines_consumed;

        let trimmed = raw_line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Strip optional `export ` prefix
        let trimmed = trimmed
            .strip_prefix("export ")
            .map(|s| s.trim_start())
            .unwrap_or(trimmed);

        // Find first `=`
        let eq_pos = match trimmed.find('=') {
            Some(p) => p,
            None => {
                let msg = format!("missing '=' in line: {trimmed}");
                if strict {
                    return Err(Error::Parse {
                        path: path.to_path_buf(),
                        line: line_start_line,
                        message: msg,
                    });
                } else {
                    tracing::warn!(path = %path.display(), line = line_start_line, "{}", msg);
                    continue;
                }
            }
        };

        let key = trimmed[..eq_pos].trim();
        if key.is_empty() {
            let msg = format!("empty key in line: {trimmed}");
            if strict {
                return Err(Error::Parse {
                    path: path.to_path_buf(),
                    line: line_start_line,
                    message: msg,
                });
            } else {
                tracing::warn!(path = %path.display(), line = line_start_line, "{}", msg);
                continue;
            }
        }

        let rest = &trimmed[eq_pos + 1..];

        let (raw_value, interpolate) = parse_value(rest);

        entries.push(ParsedEntry {
            key: key.to_string(),
            raw_value,
            line_number: line_start_line,
            interpolate,
        });
    }

    Ok(ParsedFile { entries })
}

/// Collect one logical "statement" from a character iterator.
/// Logical lines end at `\n` unless we are inside a double-quoted string.
/// Returns (line_text, number_of_newlines_consumed).
fn collect_logical_line(chars: &mut std::iter::Peekable<std::str::Chars>) -> (String, usize) {
    let mut buf = String::new();
    let mut newlines = 0usize;
    let mut in_double_quote = false;
    let mut escaped = false;

    while let Some(&ch) = chars.peek() {
        chars.next();
        match ch {
            '\n' if !in_double_quote => {
                newlines += 1;
                break;
            }
            '\n' => {
                newlines += 1;
                buf.push('\n');
            }
            '"' if !escaped => {
                in_double_quote = !in_double_quote;
                buf.push('"');
            }
            '\\' if in_double_quote && !escaped => {
                escaped = true;
                buf.push('\\');
            }
            _ => {
                escaped = false;
                buf.push(ch);
            }
        }
    }

    (buf, newlines)
}

/// Parse the value portion (everything after the first `=`).
/// Returns `(resolved_string, should_interpolate)`.
fn parse_value(raw: &str) -> (String, bool) {
    let raw = raw.trim_start();

    if raw.starts_with('\'') {
        // Single-quoted: take until matching `'`, no escapes, no interpolation
        let inner = raw
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .unwrap_or_else(|| raw.trim_matches('\''));
        (inner.to_string(), false)
    } else if raw.starts_with('"') {
        // Double-quoted: process escape sequences
        let inner = raw
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or_else(|| raw.strip_prefix('"').unwrap_or(raw));
        (process_double_quote_escapes(inner), true)
    } else {
        // Unquoted: strip inline comment (only `# ` or ` #` counts as comment marker)
        let value = strip_inline_comment(raw);
        (value.trim_end().to_string(), true)
    }
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
}
