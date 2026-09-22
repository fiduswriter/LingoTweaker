//! Regex helpers shared by the rule compiler and the SRX sentence tokenizer.
//!
//! Java allows variable-length lookbehinds; `fancy-regex` (which, unlike Java,
//! requires constant-size lookbehinds) does not. [`expand_lookbehinds`]
//! rewrites `(?<!A-?[0-9.]{0,5})` into `(?<!A)(?<!A-)(?<!A[0-9.])...` — each
//! alternative matches a fixed length, which is equivalent for negative
//! lookbehind; positive lookbehind alternatives are joined with `|`.
//!
//! Only top-level `?` and bounded `{m,n}` quantifiers are expanded; patterns
//! with unbounded quantifiers inside lookbehinds (or excessive expansion) pass
//! through unchanged and fail compilation as before.
//! [`expand_lookbehinds_bounded`] additionally approximates unbounded
//! quantifiers inside a lookbehind by a bounded expansion (used by the SRX
//! tokenizer, whose Ukrainian module has such a pattern).

/// Expand bounded lookbehinds into constant-size alternatives.
pub fn expand_lookbehinds(pattern: &str) -> String {
    expand_impl(pattern, None, 64, true)
}

/// Like [`expand_lookbehinds`], but treats `+`/`*` inside a lookbehind body as
/// `{1,max}`/`{0,max}` (an approximation; the SRX tokenizer's Ukrainian rules
/// need it). Nested group alternations are flattened too (Java allows
/// variable-length lookbehind, `fancy-regex` does not); the wrapping group is
/// kept (its content is expanded recursively), so group numbering is stable.
pub fn expand_lookbehinds_bounded(pattern: &str, max: u32) -> String {
    expand_impl(pattern, Some(max), 512, false)
}

fn expand_impl(
    pattern: &str,
    unbounded_max: Option<u32>,
    max_paths: usize,
    strip_wrapper: bool,
) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut rest = pattern;
    loop {
        let next = match ["(?<!", "(?<="].iter().filter_map(|m| rest.find(m)).min() {
            Some(pos) => pos,
            None => {
                out.push_str(rest);
                return out;
            }
        };
        let negative = rest[next..].starts_with("(?<!");
        let prefix_len = 4;
        out.push_str(&rest[..next]);
        let Some(close) = find_group_close(&rest[next..]) else {
            out.push_str(&rest[next..next + prefix_len]);
            rest = &rest[next + prefix_len..];
            continue;
        };
        let body = &rest[next + prefix_len..next + close];
        rest = &rest[next + close + 1..];
        // a negative lookbehind wrapped in a single group is redundant:
        // `(?<!(a|bb|ccc))` ≡ `(?<!a)(?<!bb)(?<!ccc)` — strip it so the
        // inner alternation branches can be expanded. The first expanded
        // path keeps a capturing group so group numbering of the rest of
        // the pattern is preserved (Java numbers lookbehind groups too;
        // they never capture when the negative assertion succeeds).
        let body_owned;
        let mut wrapped_first_path = false;
        let body = if strip_wrapper
            && negative
            && body.starts_with('(')
            && body.ends_with(')')
            && find_group_close(body) == Some(body.len() - 1)
        {
            body_owned = &body[1..body.len() - 1];
            wrapped_first_path = true;
            body_owned
        } else {
            body
        };
        match expand_body_paths(body, unbounded_max, max_paths) {
            Some(paths) if paths.len() <= max_paths && paths.iter().all(|p| !p.is_empty()) => {
                if negative {
                    for (i, p) in paths.iter().enumerate() {
                        if i == 0 && wrapped_first_path {
                            out.push_str("(?!(");
                            out.push_str(p);
                            out.push_str("))");
                        } else {
                            out.push_str("(?<!");
                            out.push_str(p);
                            out.push(')');
                        }
                    }
                } else {
                    out.push_str("(?<=");
                    out.push_str(&paths.join("|"));
                    out.push(')');
                }
            }
            _ => {
                // leave unchanged (either trivially empty or too complex)
                out.push_str(if negative { "(?<!" } else { "(?<=" });
                out.push_str(body);
                out.push(')');
            }
        }
    }
}

/// Find the index of the `)` closing the group that opens at position 0 of
/// `s` (`s` starts with the group opener), respecting escapes and classes.
fn find_group_close(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut depth = 0usize;
    let mut i = 0usize;
    let mut in_class = false;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 1,
            b'[' if !in_class => in_class = true,
            b']' if in_class => in_class = false,
            b'(' if !in_class => depth += 1,
            b')' if !in_class => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Split a regex body into top-level units (verbatim strings, possibly with
/// a quantifier) and expand bounded quantifiers into all fixed-length paths.
/// Top-level `|` branches are expanded independently and the path sets are
/// unioned (for negative lookbehind, each path gets its own assertion).
fn expand_body_paths(
    body: &str,
    unbounded_max: Option<u32>,
    max_paths: usize,
) -> Option<Vec<String>> {
    let mut paths: Vec<String> = Vec::new();
    for branch in split_top_level_branches(body) {
        let branch_paths = expand_branch_paths(&branch, unbounded_max, max_paths)?;
        paths.extend(branch_paths);
        if paths.len() > max_paths {
            return None;
        }
    }
    Some(paths)
}

/// Split on top-level `|` (outside groups/classes/escapes).
fn split_top_level_branches(body: &str) -> Vec<String> {
    let mut branches = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    let mut in_class = false;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                current.push(c);
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '[' if !in_class => {
                in_class = true;
                current.push('[');
            }
            ']' if in_class => {
                in_class = false;
                current.push(']');
            }
            '(' if !in_class => {
                depth += 1;
                current.push('(');
            }
            ')' if !in_class => {
                depth = depth.saturating_sub(1);
                current.push(')');
            }
            '|' if !in_class && depth == 0 => {
                branches.push(current);
                current = String::new();
            }
            _ => current.push(c),
        }
    }
    branches.push(current);
    branches
}

/// One parsed regex unit: the fixed-length alternatives it can match plus an
/// optional repetition range.
type Unit = (Vec<String>, Option<(u32, u32)>);

/// Expand one alternation-free branch into fixed-length paths.
fn expand_branch_paths(
    branch: &str,
    unbounded_max: Option<u32>,
    max_paths: usize,
) -> Option<Vec<String>> {
    let body = branch;
    let chars: Vec<char> = body.chars().collect();
    let mut units: Vec<Unit> = Vec::new();
    let mut i = 0usize;
    while i < chars.len() {
        let base_start = i;
        match chars[i] {
            '\\' => i += 2,
            '[' => {
                i += 1;
                while i < chars.len() && chars[i] != ']' {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
            }
            '(' => {
                let tail: String = chars[i..].iter().collect();
                // `find_group_close` returns a byte index into `tail`; convert
                // it to a character count before advancing the char cursor.
                let close_bytes = find_group_close(&tail)?;
                let close_chars = tail[..close_bytes].chars().count();
                let inner: String = chars[i + 1..i + close_chars].iter().collect();
                // A group is itself a (possibly alternation) sub-pattern; its
                // paths multiply into the branch. Java lookbehind may contain
                // variable-length alternations, `fancy-regex` may not.
                let mut base_paths = expand_body_paths(&inner, unbounded_max, max_paths)?;
                if base_paths.is_empty() {
                    base_paths.push(String::new());
                }
                units.push((base_paths, None));
                i += close_chars + 1;
                // attach the group's quantifier (if any)
                if i < chars.len() {
                    match chars[i] {
                        '?' => {
                            i += 1;
                            let unit = units.last_mut()?;
                            unit.1 = Some((0, 1));
                        }
                        '*' => {
                            let max = unbounded_max?;
                            i += 1;
                            let unit = units.last_mut()?;
                            unit.1 = Some((0, max));
                        }
                        '+' => {
                            let max = unbounded_max?;
                            i += 1;
                            let unit = units.last_mut()?;
                            unit.1 = Some((1, max));
                        }
                        _ => {}
                    }
                }
                continue;
            }
            ')' => return None,
            _ => i += 1,
        }
        if i > chars.len() {
            return None;
        }
        let base: String = chars[base_start..i].iter().collect();
        let quantifier = if i < chars.len() {
            match chars[i] {
                '?' => {
                    i += 1;
                    Some((0, 1))
                }
                '{' => {
                    let close = chars[i..].iter().position(|c| *c == '}')?;
                    let spec: String = chars[i + 1..i + close].iter().collect();
                    let (m, n) = match spec.split_once(',') {
                        Some((m, n)) => (m.parse::<u32>().ok()?, n.parse::<u32>().ok()?),
                        None => {
                            let m = spec.parse::<u32>().ok()?;
                            (m, m)
                        }
                    };
                    i += close + 1;
                    Some((m, n))
                }
                '*' => {
                    let max = unbounded_max?;
                    i += 1;
                    Some((0, max))
                }
                '+' => {
                    let max = unbounded_max?;
                    i += 1;
                    Some((1, max))
                }
                _ => None,
            }
        } else {
            None
        };
        units.push((vec![base], quantifier));
    }
    let mut paths: Vec<String> = vec![String::new()];
    for (bases, quantifier) in units {
        let mut next = Vec::new();
        for p in &paths {
            for base in &bases {
                match quantifier {
                    None => {
                        let mut candidate = p.clone();
                        candidate.push_str(base);
                        next.push(candidate);
                    }
                    Some((m, n)) => {
                        if n > 10 {
                            return None;
                        }
                        for k in m..=n {
                            let mut candidate = p.clone();
                            for _ in 0..k {
                                candidate.push_str(base);
                            }
                            next.push(candidate);
                        }
                    }
                }
            }
        }
        if next.len() > max_paths {
            return None;
        }
        paths = next;
    }
    Some(paths)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_unbounded_lookbehind() {
        let p = r"(?<!(Куан[\h]+Ю|(Петр|Олександр)([аоу]|ові|ом)?[\h]+[IІ]+))\.";
        let e = expand_lookbehinds_bounded(p, 4);
        println!("EXPANDED: {e}");
        assert!(fancy_regex::Regex::new(&e).is_ok(), "must compile: {e}");
    }
}
