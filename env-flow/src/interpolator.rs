/// Variable interpolation for env values.
///
/// Supported syntax:
/// - `${VAR}`          → value of VAR (from EnvVars then std::env)
/// - `${VAR:-default}` → value of VAR, or "default" if unset/empty
/// - `\${VAR}`         → literal `${VAR}` (escaped)
/// - Single-quoted values are NEVER interpolated (bash semantics, handled in loader)
///
/// Cycle detection: DFS with an in-progress set.
/// Interpolation can see variables set by earlier layers (cross-layer references).
use std::collections::HashSet;

use indexmap::IndexMap;

use crate::{Error, Result, types::EnvEntry};

/// Interpolate all values in the map in-place.
///
/// Variables that are pending interpolation can reference each other
/// (forward references via `std::env`).
pub fn interpolate_all(map: &mut IndexMap<String, EnvEntry>) -> Result<()> {
    // Collect keys so we can iterate without borrow conflicts
    let keys: Vec<String> = map.keys().cloned().collect();

    for key in &keys {
        let (value, should_interpolate) = {
            let entry = map.get(key).unwrap();
            (entry.value.clone(), entry.interpolate)
        };
        if !should_interpolate {
            // Single-quoted values must never be expanded
            continue;
        }
        let mut in_progress = HashSet::new();
        let resolved = interpolate_value(&value, key, map, &mut in_progress)?;
        map.get_mut(key).unwrap().value = resolved;
    }

    Ok(())
}

/// Expand `${VAR}` / `${VAR:-default}` / `\${VAR}` in a single string.
fn interpolate_value(
    input: &str,
    source_key: &str,
    map: &IndexMap<String, EnvEntry>,
    in_progress: &mut HashSet<String>,
) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Escaped `\${`
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
            // Check if next is `${`
            if i + 2 < chars.len() && chars[i + 2] == '{' {
                out.push_str("${");
                i += 3; // skip \, $, {
                continue;
            }
        }

        if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
            // Find matching `}`
            let start = i + 2;
            let mut depth = 1usize;
            let mut j = start;
            while j < chars.len() {
                match chars[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            if depth != 0 {
                // Unterminated ${, emit literally
                out.push_str("${");
                i += 2;
                continue;
            }

            let expr: String = chars[start..j].iter().collect();
            i = j + 1; // past `}`

            let (var_name, default_val) = parse_expr(&expr);

            // Cycle detection
            if in_progress.contains(var_name) {
                return Err(Error::CircularInterpolation(format!(
                    "{source_key} → {var_name}"
                )));
            }
            in_progress.insert(var_name.to_string());

            let resolved = resolve_var(var_name, default_val, map, in_progress, source_key)?;
            out.push_str(&resolved);

            in_progress.remove(var_name);
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }

    Ok(out)
}

/// Split `VAR:-default` into `("VAR", Some("default"))` or `("VAR", None)`.
fn parse_expr(expr: &str) -> (&str, Option<&str>) {
    if let Some(pos) = expr.find(":-") {
        (&expr[..pos], Some(&expr[pos + 2..]))
    } else {
        (expr, None)
    }
}

/// Look up `var_name` in the merged map, then in `std::env`.
fn resolve_var(
    var_name: &str,
    default_val: Option<&str>,
    map: &IndexMap<String, EnvEntry>,
    in_progress: &mut HashSet<String>,
    source_key: &str,
) -> Result<String> {
    // Check merged map first (may itself need interpolation)
    if let Some(entry) = map.get(var_name) {
        let raw = entry.value.clone();
        let expanded = interpolate_value(&raw, var_name, map, in_progress)?;
        return Ok(expanded);
    }

    // Fall back to process environment
    if let Ok(val) = std::env::var(var_name) {
        return Ok(val);
    }

    // Use default if provided
    if let Some(default) = default_val {
        return Ok(default.to_string());
    }

    // Variable not found, emit empty string (bash semantics for unset)
    tracing::debug!(
        key = source_key,
        var = var_name,
        "interpolation: variable not found, using empty string"
    );
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn entry(value: &str) -> EnvEntry {
        EnvEntry {
            value: value.to_string(),
            source_file: PathBuf::from(".env"),
            source_line: 1,
            overridden_by: Vec::new(),
            interpolate: true,
        }
    }

    fn map_from(pairs: &[(&str, &str)]) -> IndexMap<String, EnvEntry> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), entry(v)))
            .collect()
    }

    fn expand(input: &str, map: &IndexMap<String, EnvEntry>) -> Result<String> {
        let mut in_progress = HashSet::new();
        interpolate_value(input, "TEST", map, &mut in_progress)
    }

    #[test]
    fn simple_var() {
        let m = map_from(&[("NAME", "world")]);
        assert_eq!(expand("hello ${NAME}", &m).unwrap(), "hello world");
    }

    #[test]
    fn default_value_when_missing() {
        let m = map_from(&[]);
        assert_eq!(expand("${PORT:-8080}", &m).unwrap(), "8080");
    }

    #[test]
    fn default_not_used_when_var_set() {
        let m = map_from(&[("PORT", "3000")]);
        assert_eq!(expand("${PORT:-8080}", &m).unwrap(), "3000");
    }

    #[test]
    fn escaped_interpolation_literal() {
        let m = map_from(&[]);
        assert_eq!(expand("\\${NOT_EXPANDED}", &m).unwrap(), "${NOT_EXPANDED}");
    }

    #[test]
    fn chain_interpolation() {
        let m = map_from(&[("BASE", "hello"), ("GREETING", "${BASE}_world")]);
        assert_eq!(expand("${GREETING}", &m).unwrap(), "hello_world");
    }

    #[test]
    fn circular_interpolation_detected() {
        let m = map_from(&[("A", "${B}"), ("B", "${A}")]);
        let result = expand("${A}", &m);
        assert!(matches!(result, Err(Error::CircularInterpolation(_))));
    }

    #[test]
    fn interpolate_all_updates_map() {
        let mut m = map_from(&[
            ("BASE", "hello"),
            ("GREETING", "${BASE}_world"),
        ]);
        interpolate_all(&mut m).unwrap();
        assert_eq!(m.get("GREETING").unwrap().value, "hello_world");
        // BASE unchanged
        assert_eq!(m.get("BASE").unwrap().value, "hello");
    }

    #[test]
    fn missing_var_resolves_empty() {
        let m = map_from(&[]);
        assert_eq!(expand("${MISSING}", &m).unwrap(), "");
    }

    #[test]
    fn unterminated_brace_literal() {
        let m = map_from(&[]);
        // `${` without closing `}` — emit literally
        assert_eq!(expand("${", &m).unwrap(), "${");
    }
}
