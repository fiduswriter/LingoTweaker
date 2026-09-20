//! Port of LT `StringMatcher.getPossibleRegexpValues`: the complete set of
//! strings a simple regexp can match. The set is exact, so it can replace
//! regex matching for those tokens (`StringMatcher.create` does the same).

/// Java `RegexpParser`: parses the subset of regex syntax for which the set
/// of accepted strings can be enumerated. `Err` mirrors `TooComplexRegexp`.
struct RegexpParser<'a> {
    regexp: Vec<char>,
    pos: usize,
    _pattern: &'a str,
}

type Values = Vec<String>;

impl<'a> RegexpParser<'a> {
    fn new(pattern: &'a str) -> Self {
        let mut chars: Vec<char> = pattern.chars().collect();
        // Java strips the anchors/word boundaries it cannot represent.
        if chars.starts_with(&['\\', 'b']) {
            chars.drain(0..2);
        }
        if chars.first() == Some(&'^') {
            chars.remove(0);
        }
        if chars.len() >= 2 && chars[chars.len() - 2] == '\\' && chars[chars.len() - 1] == 'b' {
            // `\\b` at the end, but not `\\\\b` (escaped backslash before b)
            if chars.len() < 3 || chars[chars.len() - 3] != '\\' {
                chars.truncate(chars.len() - 2);
            }
        }
        if chars.last() == Some(&'$') && (chars.len() < 2 || chars[chars.len() - 2] != '\\') {
            chars.pop();
        }
        Self {
            regexp: chars,
            pos: 0,
            _pattern: pattern,
        }
    }

    const UNSUPPORTED: &'static str = "?$^{}*+";
    const FINISHING: &'static str = ")|";
    const NON_LITERAL: &'static str = ")|?$^{}*+([\\.";

    fn disjunction(&mut self) -> Result<Values, ()> {
        let mut components = vec![self.concatenation()?];
        loop {
            if self.pos >= self.regexp.len() || self.regexp[self.pos] != '|' {
                if components.len() == 1 {
                    return Ok(components.pop().unwrap());
                }
                return Ok(components.into_iter().flatten().collect());
            }
            self.pos += 1;
            components.push(self.concatenation()?);
        }
    }

    fn concatenation(&mut self) -> Result<Values, ()> {
        let mut result = self.postfix()?;
        while self.pos < self.regexp.len() {
            let c = self.regexp[self.pos];
            if Self::FINISHING.contains(c) {
                break;
            }
            if Self::UNSUPPORTED.contains(c) {
                return Err(());
            }
            let right = self.postfix()?;
            let mut combined = Values::with_capacity(result.len() * right.len());
            for l in &result {
                for r in &right {
                    combined.push(format!("{l}{r}"));
                }
            }
            result = combined;
        }
        Ok(result)
    }

    fn postfix(&mut self) -> Result<Values, ()> {
        let group = self.atom()?;
        if self.pos < self.regexp.len() {
            // Java `RegexpParser.postfix`: `{...}` → `unknown()` (too complex)
            if self.regexp[self.pos] == '{' {
                return Err(());
            }
            let next = self.regexp[self.pos];
            if "*+?".contains(next) {
                self.pos += 1;
                return match next {
                    '?' => {
                        let mut out = vec![String::new()];
                        out.extend(group);
                        Ok(out)
                    }
                    _ => Err(()),
                };
            }
        }
        Ok(group)
    }

    fn atom(&mut self) -> Result<Values, ()> {
        if self.pos >= self.regexp.len() {
            return Ok(vec![String::new()]);
        }
        match self.regexp[self.pos] {
            '(' => {
                self.pos += 1;
                if self.regexp.get(self.pos) == Some(&'?') {
                    self.pos += 1;
                    if self.regexp.get(self.pos) != Some(&':') {
                        return Err(());
                    }
                    self.pos += 1;
                }
                let group = self.disjunction()?;
                if self.regexp.get(self.pos) != Some(&')') {
                    return Err(());
                }
                self.pos += 1;
                Ok(group)
            }
            '[' => self.square_bracket_group(),
            '\\' => {
                self.pos += 1;
                match self.escape() {
                    Some(c) => Ok(vec![c.to_string()]),
                    None => Err(()),
                }
            }
            '.' => {
                self.pos += 1;
                Err(())
            }
            _ => {
                let literal_start = self.pos;
                while self.pos < self.regexp.len()
                    && !Self::NON_LITERAL.contains(self.regexp[self.pos])
                {
                    self.pos += 1;
                }
                if literal_start + 1 < self.pos
                    && self.pos < self.regexp.len()
                    && self.regexp[self.pos] == '?'
                {
                    self.pos -= 1;
                }
                Ok(vec![self.regexp[literal_start..self.pos].iter().collect()])
            }
        }
    }

    fn square_bracket_group(&mut self) -> Result<Values, ()> {
        self.pos += 1; // '['
        let start = self.pos;
        let mut options: Option<Vec<char>> = Some(Vec::new());
        loop {
            let Some(&c1) = self.regexp.get(self.pos) else {
                return Err(());
            };
            self.pos += 1;
            if c1 == ']' {
                break;
            }
            if c1 == '-' && self.pos != start + 1 && self.regexp.get(self.pos) != Some(&']') {
                let last = options.as_ref().and_then(|o| o.last().copied());
                let Some(&next) = self.regexp.get(self.pos) else {
                    return Err(());
                };
                self.pos += 1;
                let invalid = match last {
                    None => true,
                    Some(last) => next == '\\' || (next as i32 - last as i32) > 10,
                };
                if invalid {
                    options = None;
                }
                if let (Some(options), Some(last)) = (options.as_mut(), last) {
                    if next >= last {
                        for c in (last as u32 + 1)..=(next as u32) {
                            if let Some(c) = char::from_u32(c) {
                                options.push(c);
                            } else {
                                return Err(());
                            }
                        }
                    }
                }
            } else if c1 == '^' {
                options = None;
            } else if c1 == '[' {
                return Err(());
            } else {
                let simple = if c1 == '\\' {
                    match self.escape() {
                        Some(c) => c,
                        None => return Err(()),
                    }
                } else {
                    c1
                };
                if let Some(options) = options.as_mut() {
                    options.push(simple);
                }
            }
        }
        let Some(options) = options else {
            return Err(());
        };
        if options.is_empty() {
            return Err(());
        }
        Ok(options.into_iter().map(|c| c.to_string()).collect())
    }

    /// Java `RegexpParser.escape`: `null` means "unknown", `Err` is signalled
    /// through `None` in callers that treat it as too complex.
    fn escape(&mut self) -> Option<char> {
        let next = *self.regexp.get(self.pos)?;
        self.pos += 1;
        if "0xucpP".contains(next) {
            return None;
        }
        if next.is_alphanumeric() {
            return None;
        }
        Some(next)
    }
}

/// Java `StringMatcher.getPossibleRegexpValues`: all strings the regexp can
/// match, or `None` if the set cannot be enumerated.
pub fn possible_values(pattern: &str) -> Option<Vec<String>> {
    // `StringMatcher.create`: `\0` is treated as a literal, not a regexp
    if pattern == "\\0" {
        return Some(vec!["\\0".to_string()]);
    }
    let mut parser = RegexpParser::new(pattern);
    let mut values = parser.disjunction().ok()?;
    values.sort();
    values.dedup();
    if values.is_empty() {
        // Java returns an empty set here; callers treat it as unbounded
        return None;
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_alternations() {
        let mut v = possible_values("(foo|bar)").unwrap();
        v.sort();
        assert_eq!(v, vec!["bar", "foo"]);
        assert_eq!(possible_values("ab?").unwrap(), vec!["a", "ab"]);
        assert_eq!(possible_values("cat").unwrap(), vec!["cat"]);
        assert_eq!(possible_values("[abc]x").unwrap().len(), 3);
        assert!(possible_values("a.*b").is_none());
        assert!(possible_values("ab+").is_none());
    }
}
