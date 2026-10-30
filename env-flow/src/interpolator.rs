/// Variable interpolation for env values.
///
/// Supported syntax:
/// - `${VAR}`          → value of VAR (from EnvVars then std::env)
/// - `${VAR:-default}` → value of VAR, or "default" if unset/empty
/// - `\$`              → literal `$` (so `\${VAR}` is the literal `${VAR}`)
/// - `${SELF}` inside SELF's own value → SELF's previous value from the process
///   environment (`PATH=${PATH}:./bin`)
/// - Single-quoted values are NEVER interpolated (bash semantics, handled in loader)
///
/// Cycle detection: DFS with an in-progress set.
/// Interpolation can see variables set by earlier layers (cross-layer references).
use std::collections::HashSet;

use indexmap::IndexMap;

use crate::{types::EnvEntry, Error, Result};

/// Interpolate all values in the map in-place.
///
/// Variables that are pending interpolation can reference each other
/// (forward references via `std::env`).
pub fn interpolate_all(map: &mut IndexMap<String, EnvEntry>) -> Result<()> {
    // Collect keys so we can iterate without borrow conflicts
    let keys: Vec<String> = map.keys().cloned().collect();
    // Keys whose `value` is final (expanded, or never to be expanded). Expanding
    // them again would interpret text that is no longer source syntax, e.g. a
    // `${X}` that was produced by the escape `\${X}`.
    let mut done: HashSet<String> = HashSet::new();

    for key in &keys {
        let (value, should_interpolate) = {
            let entry = map.get(key).unwrap();
            (entry.value.clone(), entry.interpolate)
        };
        if should_interpolate {
            let mut in_progress = HashSet::new();
            let resolved = interpolate_value(&value, key, map, &done, &mut in_progress)?;
            map.get_mut(key).unwrap().value = resolved;
        }
        // Single-quoted values must never be expanded
        done.insert(key.clone());
    }

    Ok(())
}

/// Expand `${VAR}` / `${VAR:-default}` / `\$` in a single string.
fn interpolate_value(
    input: &str,
    source_key: &str,
    map: &IndexMap<String, EnvEntry>,
    done: &HashSet<String>,
    in_progress: &mut HashSet<String>,
) -> Result<String> {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Escaped `\$`: a literal dollar, never the start of an expansion
        if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
            out.push('$');
            i += 2;
            continue;
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

            // A variable referring to itself extends its previous value
            // (`PATH=${PATH}:./bin`); that used to be reported as a cycle and
            // fail the whole start.
            if var_name == source_key {
                out.push_str(&from_process_env(var_name, default_val));
                continue;
            }

            // Cycle detection
            if in_progress.contains(var_name) {
                return Err(Error::CircularInterpolation(format!(
                    "{source_key} → {var_name}"
                )));
            }
            in_progress.insert(var_name.to_string());

            let resolved = resolve_var(var_name, default_val, map, done, in_progress, source_key)?;
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

/// `var_name` from the process environment, or `default` when unset or empty.
fn from_process_env(var_name: &str, default_val: Option<&str>) -> String {
    match std::env::var(var_name) {
        Ok(val) if !val.is_empty() || default_val.is_none() => val,
        _ => default_val.unwrap_or_default().to_string(),
    }
}

/// Look up `var_name` in the merged map, then in `std::env`.
fn resolve_var(
    var_name: &str,
    default_val: Option<&str>,
    map: &IndexMap<String, EnvEntry>,
    done: &HashSet<String>,
    in_progress: &mut HashSet<String>,
    source_key: &str,
) -> Result<String> {
    // Check merged map first (may itself need interpolation)
    let value = if let Some(entry) = map.get(var_name) {
        if done.contains(var_name) || !entry.interpolate {
            // Final already, or single-quoted: use verbatim.
            entry.value.clone()
        } else {
            interpolate_value(&entry.value, var_name, map, done, in_progress)?
        }
    } else {
        // Fall back to process environment
        std::env::var(var_name).unwrap_or_default()
    };

    // `${VAR:-default}`: the default applies when VAR is unset *or empty*.
    if value.is_empty() {
        if let Some(default) = default_val {
            return Ok(default.to_string());
        }
        if !map.contains_key(var_name) {
            // Variable not found, emit empty string (bash semantics for unset)
            tracing::debug!(
                key = source_key,
                var = var_name,
                "interpolation: variable not found, using empty string"
            );
        }
    }
    Ok(value)
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
        interpolate_value(input, "TEST", map, &HashSet::new(), &mut in_progress)
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
        let mut m = map_from(&[("BASE", "hello"), ("GREETING", "${BASE}_world")]);
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

    fn interpolated(pairs: &[(&str, &str, bool)]) -> Result<IndexMap<String, EnvEntry>> {
        let mut map: IndexMap<String, EnvEntry> = pairs
            .iter()
            .map(|(k, v, interpolate)| {
                let mut e = entry(v);
                e.interpolate = *interpolate;
                (k.to_string(), e)
            })
            .collect();
        interpolate_all(&mut map)?;
        Ok(map)
    }

    #[test]
    fn self_reference_extends_the_process_value() {
        // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
        unsafe { std::env::set_var("ENVFLOW_TEST_SELF_PATH", "/usr/bin") };
        let map = interpolated(&[(
            "ENVFLOW_TEST_SELF_PATH",
            "${ENVFLOW_TEST_SELF_PATH}:./bin",
            true,
        )])
        .unwrap();
        assert_eq!(map["ENVFLOW_TEST_SELF_PATH"].value, "/usr/bin:./bin");
        // SAFETY: test code; only std reads the environment here, and std serialises its own accesses.
        unsafe { std::env::remove_var("ENVFLOW_TEST_SELF_PATH") };
    }

    #[test]
    fn real_cycles_are_still_errors() {
        assert!(interpolated(&[("A", "${B}", true), ("B", "${A}", true)]).is_err());
    }

    #[test]
    fn single_quoted_values_stay_literal_when_referenced() {
        let map =
            interpolated(&[("A", "${HOME_NOT_EXPANDED}", false), ("B", "${A}", true)]).unwrap();
        assert_eq!(map["B"].value, "${HOME_NOT_EXPANDED}");
    }

    #[test]
    fn escaped_dollar_is_not_expanded_again_through_a_reference() {
        let map = interpolated(&[
            ("A", "\\${X}", true),
            ("X", "boom", true),
            ("B", "${A}", true),
        ])
        .unwrap();
        assert_eq!(map["A"].value, "${X}");
        assert_eq!(map["B"].value, "${X}");
    }

    #[test]
    fn default_applies_to_empty_values() {
        let map = interpolated(&[("EMPTY", "", true), ("V", "${EMPTY:-fallback}", true)]).unwrap();
        assert_eq!(map["V"].value, "fallback");
    }

    #[test]
    fn escaped_dollar_in_double_quotes_end_to_end() {
        let parsed = crate::parser::parse(
            "X=boom\nA=\"\\${X} costs \\$5\"\n",
            std::path::Path::new(".env"),
            true,
        )
        .unwrap();
        let mut map: IndexMap<String, EnvEntry> = parsed
            .entries
            .into_iter()
            .map(|e| {
                let mut v = entry(&e.raw_value);
                v.interpolate = e.interpolate;
                (e.key, v)
            })
            .collect();
        interpolate_all(&mut map).unwrap();
        assert_eq!(map["A"].value, "${X} costs $5");
    }
}
