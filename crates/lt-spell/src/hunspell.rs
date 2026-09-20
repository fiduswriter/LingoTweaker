//! Faithful port of the hunspell 1.7.2 spelling checker (`spell()` +
//! affix/compound/break logic) as used by the upstream German speller
//! through the native libhunspell binding.
//!
//! Scope: everything `hunspell_spell()` needs for the German dictionaries
//! (`de_DE`/`de_AT`/`de_CH`). Suggestion generation (`suggestmgr`) is not
//! ported; the German rule takes its suggestions from the morfologik
//! spellers, like Java does.
//!
//! Ported from the upstream hunspell v1.7.2 sources: `hunspell.cxx`
//! (`spell`, `spell_internal`, `cleanword2`, `checkword`, `spellsharps`),
//! `affixmgr.cxx` (`prefix_check`, `suffix_check`, `*_twosfx`,
//! `compound_check`, `parse_affix`, `encodeit`, `condlen`, `isSubset`,
//! `isRevSubset`, `setcminmax`), `affentry.cxx` (`PfxEntry`/`SfxEntry`
//! `checkword`/`check_twosfx`, `test_condition`) and `hashmgr.cxx`
//! (dictionary load incl. `add_hidden_capitalized_word`).
//!
//! Unsupported (erroring at load, so a dictionary that needs them is never
//! silently mis-checked): `COMPLEXPREFIXES`, `AF`/`AM` flag aliases,
//! non-default `FLAG` modes, `ICONV`/`OCONV`, `IGNORE`, `COMPOUNDRULE`,
//! `CHECKCOMPOUNDPATTERN`/`CHECKCOMPOUNDREP`/`CHECKCOMPOUNDTRIPLE`/
//! `CHECKCOMPOUNDCASE`/`CHECKCOMPOUNDDUP`/`COMPOUNDWORDMAX`. None of them
//! occur (uncommented) in the German `.aff` files.

use std::path::Path;

use lt_core::{CoreError, Result};

const MAXWORDLEN: usize = 100;
const MAXWORDUTF8LEN: usize = MAXWORDLEN * 3;
const MAXSHARPS: usize = 5;

/// One dictionary entry, decoded on demand from the raw `.dic` bytes
/// (`hentry` view: flags, ONLYUPCASE marker, byte length).
#[derive(Clone, Copy)]
struct DicEntry<'a> {
    flags: &'a [u8],
    only_upcase: bool,
    /// byte length of the dictionary word (`hentry::blen`)
    word_len: usize,
}

/// A dictionary entry as stored in the sorted index: offsets into the raw
/// `.dic` bytes and the concatenated flag arena.
#[derive(Clone, Copy, Debug)]
struct PackedEntry {
    word_off: u32,
    word_len: u32,
    flags_off: u32,
    flags_len: u32,
    only_upcase: bool,
}

/// The dictionary kept in its raw (compressed) form: the `.dic` bytes are
/// stored as-is, an entry costs a 20-byte index record, and words/flags are
/// decoded per lookup (expanding `de_DE.dic` into a hash map cost ~90 MB,
/// D-033).
struct WordIndex {
    dic: Vec<u8>,
    flags: Vec<u8>,
    /// sorted by word bytes (stable: insertion order per word)
    entries: Vec<PackedEntry>,
}

impl WordIndex {
    fn word<'a>(dic: &'a [u8], e: &PackedEntry) -> &'a [u8] {
        let start = e.word_off as usize;
        &dic[start..start + e.word_len as usize]
    }

    /// Entries for `word` in insertion order (empty when absent).
    fn entries_for(&self, word: &[u8]) -> &[PackedEntry] {
        let start = self
            .entries
            .partition_point(|e| Self::word(&self.dic, e) < word);
        let end = self
            .entries
            .partition_point(|e| Self::word(&self.dic, e) <= word);
        &self.entries[start..end]
    }

    fn view(&self, e: &PackedEntry) -> DicEntry<'_> {
        let off = e.flags_off as usize;
        DicEntry {
            flags: &self.flags[off..off + e.flags_len as usize],
            only_upcase: e.only_upcase,
            word_len: e.word_len as usize,
        }
    }
}

#[derive(Debug, Clone)]
struct AffixEntry {
    flag: u8,
    cross: bool,
    strip: Vec<u8>,
    appnd: Vec<u8>,
    cont: Vec<u8>,
    /// Condition exactly as hunspell stores it (suffixes: byte-reversed with
    /// '['/']' swapped; see `parse_affix`).
    cond: Vec<u8>,
    numconds: usize,
}

#[derive(Debug)]
struct Aff {
    /// Non-empty prefixes, in hunspell's iteration order (sorted by affix
    /// string, ties in reverse file order).
    prefixes: Vec<AffixEntry>,
    /// Empty prefixes in hunspell's head-insertion order (reverse file
    /// order).
    empty_prefixes: Vec<AffixEntry>,
    suffixes: Vec<AffixEntry>,
    empty_suffixes: Vec<AffixEntry>,
    compound: bool,
    compound_flag: Option<u8>,
    compound_begin: Option<u8>,
    compound_middle: Option<u8>,
    compound_end: Option<u8>,
    compound_permit: Option<u8>,
    compound_forbid: Option<u8>,
    compound_root: Option<u8>,
    compound_min: usize,
    forbidden_word: Option<u8>,
    keep_case: Option<u8>,
    need_affix: Option<u8>,
    only_in_compound: Option<u8>,
    circumfix: Option<u8>,
    checksharps: bool,
    fullstrip: bool,
    have_cont_class: bool,
    cont_classes: Box<[bool; 256]>,
    break_patterns: Vec<Vec<u8>>,
}

impl Default for Aff {
    fn default() -> Self {
        Aff {
            prefixes: Vec::new(),
            empty_prefixes: Vec::new(),
            suffixes: Vec::new(),
            empty_suffixes: Vec::new(),
            compound: false,
            compound_flag: None,
            compound_begin: None,
            compound_middle: None,
            compound_end: None,
            compound_permit: None,
            compound_forbid: None,
            compound_root: None,
            compound_min: 3,
            forbidden_word: None,
            keep_case: None,
            need_affix: None,
            only_in_compound: None,
            circumfix: None,
            checksharps: false,
            fullstrip: false,
            have_cont_class: false,
            cont_classes: Box::new([false; 256]),
            break_patterns: Vec::new(),
        }
    }
}

impl Aff {
    fn parse(text: &str) -> Result<Aff> {
        let mut aff = Aff::default();
        let mut pending: Vec<(bool, u8, bool, AffixEntry)> = Vec::new();
        let mut lines = text.lines().peekable();
        while let Some(raw) = lines.next() {
            let line = raw.trim_end_matches('\r');
            let mut it = line.split_whitespace();
            let Some(kind) = it.next() else { continue };
            if kind.starts_with('#') {
                continue;
            }
            match kind {
                "PFX" | "SFX" => {
                    let is_prefix = kind == "PFX";
                    let flag = single_flag(&mut it, line)?;
                    let cross = it.next().map(|f| f == "Y").unwrap_or(false);
                    let count: usize = it
                        .next()
                        .and_then(|c| c.parse().ok())
                        .ok_or_else(|| parse_err(line))?;
                    for _ in 0..count {
                        let eline = lines.next().ok_or_else(|| parse_err(line))?;
                        let eline = eline.trim_end_matches('\r');
                        let entry = parse_affix_entry(eline, is_prefix, flag, cross)?;
                        for &c in &entry.cont {
                            aff.have_cont_class = true;
                            aff.cont_classes[c as usize] = true;
                        }
                        pending.push((is_prefix, flag, cross, entry));
                    }
                }
                _ => parse_directive(&mut aff, kind, &mut it, line, &mut lines)?,
            }
        }
        // hunspell inserts rules into per-first-byte binary trees (ties: a
        // later insertion becomes the left child, so in-order visits it
        // first) and flattens them in-order. Reproduce that order: sort by
        // the BST key ascending, ties in reverse file order. Empty affix
        // strings are prepended to bucket 0 by head insertion, i.e. reverse
        // file order.
        for (is_prefix, _flag, _cross, entry) in pending {
            if is_prefix {
                if entry.appnd.is_empty() {
                    aff.empty_prefixes.insert(0, entry);
                } else {
                    let mut idx = aff.prefixes.len();
                    // insert in sorted position; ties go before existing
                    // (reverse file order)
                    for (i, e) in aff.prefixes.iter().enumerate() {
                        if entry.appnd <= e.appnd {
                            idx = i;
                            break;
                        }
                    }
                    aff.prefixes.insert(idx, entry);
                }
            } else if entry.appnd.is_empty() {
                aff.empty_suffixes.insert(0, entry);
            } else {
                let key = |e: &AffixEntry| {
                    let mut k = e.appnd.clone();
                    k.reverse();
                    k
                };
                let entry_key = key(&entry);
                let mut idx = aff.suffixes.len();
                for (i, e) in aff.suffixes.iter().enumerate() {
                    if entry_key <= key(e) {
                        idx = i;
                        break;
                    }
                }
                aff.suffixes.insert(idx, entry);
            }
        }
        aff.compound = aff.compound_begin.is_some()
            || aff.compound_middle.is_some()
            || aff.compound_end.is_some()
            || aff.compound_flag.is_some();
        Ok(aff)
    }
}

fn parse_err(line: &str) -> CoreError {
    CoreError::Data(format!("hunspell affix parse error: {line:?}"))
}

fn single_flag(it: &mut std::str::SplitWhitespace<'_>, line: &str) -> Result<u8> {
    let token = it.next().ok_or_else(|| parse_err(line))?;
    let bytes = token.as_bytes();
    if bytes.len() != 1 {
        return Err(CoreError::Data(format!(
            "hunspell flag {token:?} is not a single byte in {line:?} (FLAG modes are unsupported)"
        )));
    }
    Ok(bytes[0])
}

fn parse_affix_entry(line: &str, is_prefix: bool, flag: u8, cross: bool) -> Result<AffixEntry> {
    let mut it = line.split_whitespace();
    let _type = it.next().ok_or_else(|| parse_err(line))?;
    let f = single_flag(&mut it, line)?;
    if f != flag {
        return Err(parse_err(line));
    }
    let strip = it.next().ok_or_else(|| parse_err(line))?;
    let add_field = it.next().ok_or_else(|| parse_err(line))?;
    let cond_field = it.next().ok_or_else(|| parse_err(line))?;
    let strip = if strip == "0" {
        Vec::new()
    } else {
        strip.as_bytes().to_vec()
    };
    let add_str = add_field.split('/').next().unwrap_or("");
    let cont = match add_field.split_once('/') {
        Some((_, c)) => c.as_bytes().to_vec(),
        None => Vec::new(),
    };
    let appnd = if add_str == "0" {
        Vec::new()
    } else {
        add_str.as_bytes().to_vec()
    };
    // `redundant_condition` (UTF-8 path: only the plain string compare).
    let mut cond_field = cond_field.to_string();
    if !strip.is_empty() && cond_field != "." {
        let redundant = if is_prefix {
            strip.starts_with(cond_field.as_bytes())
        } else {
            strip.len() >= cond_field.len() && strip.ends_with(cond_field.as_bytes())
        };
        if redundant {
            cond_field = ".".to_string();
        }
    }
    let mut cond = cond_field.as_bytes().to_vec();
    let numconds = if cond_field == "." { 0 } else { condlen(&cond) };
    if !is_prefix {
        cond.reverse();
        reverse_condition(&mut cond);
    }
    Ok(AffixEntry {
        flag,
        cross,
        strip,
        appnd,
        cont,
        cond,
        numconds,
    })
}

/// hunspell `condlen`: number of condition characters (groups count once).
fn condlen(cond: &[u8]) -> usize {
    let mut l = 0;
    let mut group = false;
    for &c in cond {
        if c == b'[' {
            group = true;
            l += 1;
        } else if c == b']' {
            group = false;
        } else if !group && (c & 0x80 == 0 || c & 0xc0 == 0x80) {
            l += 1;
        }
    }
    l
}

/// hunspell `reverse_condition`: swap `[`/`]` and move `^` for negated
/// groups when reversing a condition string. `k` walks the string backwards
/// like a C++ reverse iterator, so `*(k - 1)` is the byte *after* `k`.
fn reverse_condition(piece: &mut [u8]) {
    if piece.is_empty() {
        return;
    }
    let mut neg = false;
    for k in (0..piece.len()).rev() {
        let next = if k + 1 < piece.len() { piece[k + 1] } else { 0 };
        match piece[k] {
            b'[' => {
                if neg && k + 1 < piece.len() {
                    piece[k + 1] = b'[';
                } else {
                    piece[k] = b']';
                }
            }
            b']' => {
                piece[k] = b'[';
                if neg && k + 1 < piece.len() {
                    piece[k + 1] = b'^';
                }
                neg = false;
            }
            b'^' => {
                if next == b']' {
                    neg = true;
                } else if neg && k + 1 < piece.len() {
                    piece[k + 1] = piece[k];
                }
            }
            _ => {
                if neg && k + 1 < piece.len() {
                    piece[k + 1] = piece[k];
                }
            }
        }
    }
}

/// Parse a non-affix directive (`AffixMgr::parse_file` subset).
fn parse_directive<'a>(
    aff: &mut Aff,
    kind: &str,
    it: &mut std::str::SplitWhitespace<'a>,
    line: &str,
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
) -> Result<()> {
    match kind {
        "SET" | "LANG" | "TRY" | "REP" | "KEY" | "MAP" | "PHONE" | "NOSUGGEST" | "WARN"
        | "SUBSTANDARD" | "FORCEUCASE" | "SYLLABLENUM" | "WORDCHARS" | "MAXNGRAMSUGS"
        | "MAXDIFF" | "ONLYMAXDIFF" | "MAXCPDSUGS" | "MAXSUGS" | "NOSPLITSUGS" | "SUGSWITHDOTS"
        | "LEMMA_PRESENT" | "OCONV" | "ICONV" => {}
        "FLAG"
        | "AF"
        | "AM"
        | "COMPLEXPREFIXES"
        | "IGNORE"
        | "COMPOUNDRULE"
        | "CHECKCOMPOUNDPATTERN"
        | "COMPOUNDWORDMAX"
        | "CHECKCOMPOUNDCASE"
        | "CHECKCOMPOUNDREP"
        | "CHECKCOMPOUNDTRIPLE"
        | "CHECKCOMPOUNDDUP"
        | "SIMPLIFIEDTRIPLE"
        | "COMPOUNDMORESUFFIXES" => {
            return Err(CoreError::Data(format!(
                "hunspell directive {kind:?} is not supported by the in-tree checker ({line:?})"
            )));
        }
        "FULLSTRIP" => aff.fullstrip = true,
        "CHECKSHARPS" => aff.checksharps = true,
        "COMPOUNDFLAG" => aff.compound_flag = Some(single_flag(it, line)?),
        "COMPOUNDBEGIN" => aff.compound_begin = Some(single_flag(it, line)?),
        "COMPOUNDMIDDLE" => aff.compound_middle = Some(single_flag(it, line)?),
        "COMPOUNDEND" => aff.compound_end = Some(single_flag(it, line)?),
        "COMPOUNDPERMITFLAG" => aff.compound_permit = Some(single_flag(it, line)?),
        "COMPOUNDFORBIDFLAG" => aff.compound_forbid = Some(single_flag(it, line)?),
        "COMPOUNDROOT" => aff.compound_root = Some(single_flag(it, line)?),
        "FORBIDDENWORD" => aff.forbidden_word = Some(single_flag(it, line)?),
        "KEEPCASE" => aff.keep_case = Some(single_flag(it, line)?),
        "NEEDAFFIX" | "PSEUDOROOT" => aff.need_affix = Some(single_flag(it, line)?),
        "ONLYINCOMPOUND" => aff.only_in_compound = Some(single_flag(it, line)?),
        "CIRCUMFIX" => aff.circumfix = Some(single_flag(it, line)?),
        "COMPOUNDMIN" => aff.compound_min = it.next().and_then(|v| v.parse().ok()).unwrap_or(1),
        "BREAK" => {
            let count: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            for _ in 0..count {
                let Some(bline) = lines.next() else { break };
                let bline = bline.trim_end_matches('\r');
                // each entry line is `BREAK <pattern>`
                let pattern = bline.split_whitespace().nth(1).unwrap_or("");
                if !pattern.is_empty() {
                    aff.break_patterns.push(pattern.as_bytes().to_vec());
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// The in-tree hunspell checker for one `.aff`/`.dic` pair.
pub struct HunspellChecker {
    aff: Aff,
    index: WordIndex,
}

impl HunspellChecker {
    pub fn load(aff_path: &Path, dic_path: &Path) -> Result<Self> {
        let aff_text = lt_data::fs::read_to_string(aff_path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", aff_path.display())))?;
        let dic_text = lt_data::fs::read_to_string(dic_path)
            .map_err(|e| CoreError::Data(format!("cannot read {}: {e}", dic_path.display())))?;
        Self::from_strs(&aff_text, &dic_text)
    }

    pub fn from_strs(aff_text: &str, dic_text: &str) -> Result<Self> {
        let aff = Aff::parse(aff_text)?;
        let mut dic: Vec<u8> = Vec::with_capacity(dic_text.len());
        let mut flag_arena: Vec<u8> = Vec::new();
        let mut raw: Vec<PackedEntry> = Vec::new();
        let mut lines = dic_text.lines();
        lines.next(); // entry count
        for line in lines {
            let line = line.trim_end_matches('\r');
            if line.starts_with('#') {
                continue;
            }
            let mut word_part = line;
            if let Some(tab) = line.find('\t') {
                word_part = &line[..tab];
            }
            if word_part.is_empty() {
                continue;
            }
            let bytes = word_part.as_bytes();
            let mut slash = None;
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'/' {
                    slash = Some(i);
                    break;
                }
                i += 1;
            }
            let (word_raw, flags_raw) = match slash {
                Some(pos) if pos > 0 => (&bytes[..pos], Some(&bytes[pos + 1..])),
                _ => (bytes, None),
            };
            let mut word: Vec<u8> = Vec::with_capacity(word_raw.len());
            let mut k = 0;
            while k < word_raw.len() {
                if word_raw[k] == b'\\' && k + 1 < word_raw.len() && word_raw[k + 1] == b'/' {
                    word.push(b'/');
                    k += 2;
                } else {
                    word.push(word_raw[k]);
                    k += 1;
                }
            }
            if word.is_empty() {
                continue;
            }
            let mut flags: Vec<u8> = flags_raw.map(|f| f.to_vec()).unwrap_or_default();
            flags.sort_unstable();
            let flags_off = flag_arena.len() as u32;
            flag_arena.extend_from_slice(&flags);
            let flags_len = flags.len() as u32;
            let push =
                |dic: &mut Vec<u8>, raw: &mut Vec<PackedEntry>, word: &[u8], only_upcase: bool| {
                    let word_off = dic.len() as u32;
                    dic.extend_from_slice(word);
                    raw.push(PackedEntry {
                        word_off,
                        word_len: word.len() as u32,
                        flags_off,
                        flags_len,
                        only_upcase,
                    });
                };
            // `add_word` + `add_hidden_capitalized_word` (hashmgr.cxx). The
            // hidden capitalized homonym is dropped when a homonym of the
            // same word already exists, and a normal entry replaces an
            // existing hidden homonym (the ONLYUPCASEFLAG merge in
            // `HashMgr::add_word`); the merge is reproduced below from the
            // insertion-ordered records.
            let captype = captype_utf8(&word);
            if matches!(captype, Captype::HuhCap | Captype::HuhInitCap)
                || (captype == Captype::AllCap && !flags.is_empty())
            {
                let forbidden = aff.forbidden_word.is_some_and(|f| flags.contains(&f));
                if !forbidden {
                    let mut hidden = lowercase_bytes(&word);
                    let first = upper_bytes(&hidden[..char_len(&hidden)]);
                    hidden.splice(0..char_len(&hidden), first);
                    push(&mut dic, &mut raw, &hidden, true);
                }
            }
            push(&mut dic, &mut raw, &word, false);
        }
        // stable sort by word bytes, then reproduce the ONLYUPCASE merge per
        // word group in insertion order
        raw.sort_by(|a, b| WordIndex::word(&dic, a).cmp(WordIndex::word(&dic, b)));
        let mut entries: Vec<PackedEntry> = Vec::with_capacity(raw.len());
        let mut i = 0;
        while i < raw.len() {
            let word = WordIndex::word(&dic, &raw[i]);
            let mut j = i + 1;
            while j < raw.len() && WordIndex::word(&dic, &raw[j]) == word {
                j += 1;
            }
            let mut result: Vec<PackedEntry> = Vec::new();
            for rec in &raw[i..j] {
                if rec.only_upcase {
                    if result.is_empty() {
                        result.push(*rec);
                    }
                } else if result.last().is_some_and(|e| e.only_upcase) {
                    *result.last_mut().unwrap() = *rec;
                } else {
                    result.push(*rec);
                }
            }
            entries.extend(result);
            i = j;
        }
        Ok(Self {
            aff,
            index: WordIndex {
                dic,
                flags: flag_arena,
                entries,
            },
        })
    }

    /// `Hunspell::spell()`.
    pub fn spell(&self, word: &str) -> bool {
        self.spell_internal(word.as_bytes())
    }

    fn spell_internal(&self, word: &[u8]) -> bool {
        if word.len() >= MAXWORDUTF8LEN {
            return false;
        }
        // `cleanword2`: skip leading blanks, strip trailing periods.
        let src = skip_leading_bytes(word, b' ');
        let mut end = src.len();
        let mut abbv = 0usize;
        while end > 0 && src[end - 1] == b'.' {
            end -= 1;
            abbv += 1;
        }
        let scw0 = &src[..end];
        if scw0.is_empty() {
            return true;
        }
        let orig_captype = captype_utf8(scw0);
        // Allow numbers with dots, dashes and commas (but forbid double
        // separators).
        {
            const NBEGIN: u8 = 0;
            const NNUM: u8 = 1;
            const NSEP: u8 = 2;
            let mut state = NBEGIN;
            let mut i = 0;
            while i < scw0.len() {
                let c = scw0[i];
                if c.is_ascii_digit() {
                    state = NNUM;
                } else if c == b',' || c == b'.' || c == b'-' {
                    if state == NSEP || i == 0 {
                        break;
                    }
                    state = NSEP;
                } else {
                    break;
                }
                i += 1;
            }
            if i == scw0.len() && state == NNUM {
                return true;
            }
        }
        let mut info = Info::default();
        let mut scw = scw0.to_vec();
        let found = match orig_captype {
            Captype::HuhCap | Captype::HuhInitCap => {
                info.orig_cap = true;
                let mut found = self.checkword(&scw, &mut info).is_some();
                if !found && abbv > 0 {
                    let mut with_dot = scw.clone();
                    with_dot.push(b'.');
                    found = self.checkword(&with_dot, &mut info).is_some();
                }
                found
            }
            Captype::NoCap => {
                let mut found = self.checkword(&scw, &mut info).is_some();
                if !found && abbv > 0 {
                    let mut with_dot = scw.clone();
                    with_dot.push(b'.');
                    found = self.checkword(&with_dot, &mut info).is_some();
                }
                found
            }
            Captype::AllCap => {
                info.orig_cap = true;
                let mut found = self.checkword(&scw, &mut info).is_some();
                if !found && abbv > 0 {
                    let mut with_dot = scw.clone();
                    with_dot.push(b'.');
                    found = self.checkword(&with_dot, &mut info).is_some();
                }
                if !found {
                    // Catalan/French/Italian apostrophe prefixes.
                    if let Some(apos) = scw.iter().position(|&c| c == b'\'') {
                        let lower = lowercase_bytes(&scw);
                        if apos < lower.len().saturating_sub(1) {
                            let (p1, p2) = lower.split_at(apos + 1);
                            let mut cand = p1.to_vec();
                            let mut p2_t = p2.to_vec();
                            initcap_bytes(&mut p2_t);
                            cand.extend_from_slice(&p2_t);
                            if self.checkword(&cand, &mut info).is_some() {
                                found = true;
                            } else {
                                let mut titled = lower.clone();
                                initcap_bytes(&mut titled);
                                if self.checkword(&titled, &mut info).is_some() {
                                    found = true;
                                }
                            }
                        }
                    }
                }
                if !found && self.aff.checksharps && contains(&scw, b"SS") {
                    let lower = lowercase_bytes(&scw);
                    let mut b = lower.clone();
                    found = self.spellsharps(&mut b, 0, 0, 0, &mut info);
                    if !found {
                        let mut b = lower.clone();
                        initcap_bytes(&mut b);
                        found = self.spellsharps(&mut b, 0, 0, 0, &mut info);
                    }
                    if !found && abbv > 0 {
                        let mut b = lower.clone();
                        b.push(b'.');
                        found = self.spellsharps(&mut b, 0, 0, 0, &mut info);
                        if !found {
                            let mut b = lower.clone();
                            initcap_bytes(&mut b);
                            b.push(b'.');
                            found = self.spellsharps(&mut b, 0, 0, 0, &mut info);
                        }
                    }
                }
                if !found {
                    self.spell_initcap(&mut scw, Captype::AllCap, abbv, &mut info)
                } else {
                    true
                }
            }
            Captype::InitCap => self.spell_initcap(&mut scw, Captype::InitCap, abbv, &mut info),
        };
        if found {
            return true;
        }
        if info.forbidden {
            return false;
        }
        self.spell_break(&scw)
    }

    /// The shared `INITCAP` case (also the ALLCAP fallthrough).
    fn spell_initcap(
        &self,
        scw: &mut Vec<u8>,
        captype: Captype,
        abbv: usize,
        info: &mut Info,
    ) -> bool {
        let idot = scw.len() >= 2 && scw[0] == 0xC4 && scw[1] == 0xB0;
        info.orig_cap = true;
        if captype == Captype::AllCap {
            *scw = lowercase_bytes(scw);
            initcap_bytes(scw);
            if idot && scw.len() >= 2 {
                scw[0] = 0xC4;
                scw[1] = 0xB0;
            }
        }
        if captype == Captype::InitCap {
            info.init_cap = true;
        }
        let lower = lowercase_bytes(scw);
        let mut found = self.checkword(scw, info).is_some();
        if captype == Captype::InitCap {
            info.init_cap = false;
        }
        if info.forbidden {
            return false;
        }
        if found {
            if let Some(entry) = self.lookup_first(scw) {
                if self.is_keepcase(entry) && captype == Captype::AllCap {
                    found = false;
                }
            }
        }
        if found || idot {
            return found;
        }
        let mut check = lower.clone();
        initcap_bytes(scw);
        found = self.checkword(&check, info).is_some();
        if !found && abbv > 0 {
            check.push(b'.');
            found = self.checkword(&check, info).is_some();
            if !found {
                let mut t = scw.clone();
                t.push(b'.');
                found = self.checkword(&t, info).is_some();
                if found {
                    if let Some(entry) = self.lookup_first(&t) {
                        if self.is_keepcase(entry) && captype == Captype::AllCap {
                            found = false;
                        }
                    }
                }
            }
        }
        if found {
            if let Some(entry) = self.lookup_first(&check) {
                let has_sharp = contains(&check, "ß".as_bytes());
                if self.is_keepcase(entry)
                    && (captype == Captype::AllCap || !(self.aff.checksharps && has_sharp))
                {
                    found = false;
                }
            }
        }
        found
    }

    fn spellsharps(
        &self,
        base: &mut Vec<u8>,
        n_pos: usize,
        n: usize,
        repnum: usize,
        info: &mut Info,
    ) -> bool {
        match find_sub(base, b"ss", n_pos) {
            Some(pos) if n < MAXSHARPS => {
                base[pos] = 0xC3;
                base[pos + 1] = 0x9F;
                if self.spellsharps(base, pos + 2, n + 1, repnum + 1, info) {
                    return true;
                }
                base[pos] = b's';
                base[pos + 1] = b's';
                self.spellsharps(base, pos + 2, n + 1, repnum, info)
            }
            _ if repnum > 0 => self.checkword(base, info).is_some(),
            _ => false,
        }
    }

    /// hunspell `checkword`.
    fn checkword(&self, word: &[u8], info: &mut Info) -> Option<DicEntry<'_>> {
        if word.is_empty() {
            return None;
        }
        let entries = self.index.entries_for(word);
        if !entries.is_empty() {
            if self.has_flag(self.index.view(&entries[0]), self.aff.forbidden_word) {
                info.forbidden = true;
                return None;
            }
            for packed in entries {
                let e = self.index.view(packed);
                let skip_needaffix = self.aff.need_affix.is_some_and(|f| e.flags.contains(&f));
                let skip_only = self
                    .aff
                    .only_in_compound
                    .is_some_and(|f| e.flags.contains(&f));
                let skip_upcase = e.only_upcase && info.init_cap;
                if !(skip_needaffix || skip_only || skip_upcase) {
                    return Some(e);
                }
            }
        }
        let mut state = AffixState::default();
        if let Some(entry) =
            self.affix_check(word, 0, word.len(), None, InCompound::Not, &mut state)
        {
            let only = self
                .aff
                .only_in_compound
                .is_some_and(|f| entry.flags.contains(&f));
            let upcase = entry.only_upcase && info.init_cap;
            if !(only || upcase) {
                if self.has_flag(entry, self.aff.forbidden_word) {
                    info.forbidden = true;
                    return None;
                }
                return Some(entry);
            }
        }
        if self.aff.compound {
            let mut info2 = *info;
            info2.compound_2 = true;
            let he = self.compound_check(word, 0, 0, 100, 0, &mut info2);
            *info = info2;
            info.compound_2 = false;
            if he.is_none() && !info.compound_2 {
                let mut info3 = *info;
                let he = self.compound_check(word, 0, 0, 100, 0, &mut info3);
                *info = info3;
                if he.is_some() && !word[0].is_ascii_digit() {
                    // hunspell vetoes a 3+-part compound when the suggester
                    // finds a simple correction ("dictionary word with a
                    // typo"). The suggester is not ported; documented in
                    // D-033.
                }
                if he.is_some() {
                    info.compound = true;
                    return he;
                }
            }
            if he.is_some() {
                info.compound = true;
                return he;
            }
        }
        None
    }

    fn has_flag(&self, entry: DicEntry<'_>, flag: Option<u8>) -> bool {
        flag.is_some_and(|f| entry.flags.contains(&f))
    }

    fn is_keepcase(&self, entry: DicEntry<'_>) -> bool {
        self.has_flag(entry, self.aff.keep_case)
    }

    fn lookup_first(&self, word: &[u8]) -> Option<DicEntry<'_>> {
        self.index
            .entries_for(word)
            .first()
            .map(|e| self.index.view(e))
    }

    /// `AffixMgr::affix_check`.
    fn affix_check(
        &self,
        word: &[u8],
        start: usize,
        len: usize,
        needflag: Option<u8>,
        in_compound: InCompound,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        if let Some(entry) = self.prefix_check(word, start, len, in_compound, needflag, state) {
            return Some(entry);
        }
        if let Some(entry) = self.suffix_check(
            word,
            start,
            len,
            0,
            None,
            None,
            needflag,
            in_compound,
            state,
        ) {
            return Some(entry);
        }
        if self.aff.have_cont_class {
            state.sfx = None;
            state.pfx = None;
            if let Some(entry) =
                self.suffix_check_twosfx(word, start, len, 0, None, needflag, state)
            {
                return Some(entry);
            }
            if let Some(entry) =
                self.prefix_check_twosfx(word, start, len, InCompound::Not, needflag, state)
            {
                return Some(entry);
            }
        }
        None
    }

    fn prefix_check(
        &self,
        word: &[u8],
        start: usize,
        len: usize,
        in_compound: InCompound,
        needflag: Option<u8>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        state.pfx = None;
        state.sfxappnd = None;
        for (idx, e) in self.aff.empty_prefixes.iter().enumerate() {
            if self.prefix_allowed_in_compound(e, in_compound) {
                if let Some(entry) = self.pfx_entry_checkword(
                    e,
                    idx,
                    true,
                    word,
                    start,
                    len,
                    in_compound,
                    needflag,
                    state,
                ) {
                    state.pfx = Some(PfxId::Empty(idx));
                    return Some(entry);
                }
            }
        }
        if start >= word.len() {
            return None;
        }
        let first = word[start];
        for (idx, e) in self.aff.prefixes.iter().enumerate() {
            if e.appnd.first().copied() != Some(first) {
                continue;
            }
            if !is_subset(&e.appnd, &word[start..]) {
                continue;
            }
            if !self.prefix_allowed_in_compound(e, in_compound) {
                continue;
            }
            if let Some(entry) = self.pfx_entry_checkword(
                e,
                idx,
                false,
                word,
                start,
                len,
                in_compound,
                needflag,
                state,
            ) {
                state.pfx = Some(PfxId::Entry(idx));
                return Some(entry);
            }
        }
        None
    }

    fn prefix_allowed_in_compound(&self, e: &AffixEntry, in_compound: InCompound) -> bool {
        let only = self
            .aff
            .only_in_compound
            .is_some_and(|f| e.cont.contains(&f));
        if in_compound == InCompound::Not && only {
            return false;
        }
        if in_compound == InCompound::End {
            let permit = self
                .aff
                .compound_permit
                .is_some_and(|f| e.cont.contains(&f));
            if !permit {
                return false;
            }
        }
        true
    }

    /// `PfxEntry::checkword`.
    #[allow(clippy::too_many_arguments)]
    fn pfx_entry_checkword(
        &self,
        e: &AffixEntry,
        idx: usize,
        empty: bool,
        word: &[u8],
        start: usize,
        len: usize,
        in_compound: InCompound,
        needflag: Option<u8>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        if len < e.appnd.len() {
            return None;
        }
        let tmpl = len - e.appnd.len();
        if !(tmpl > 0 || (tmpl == 0 && self.aff.fullstrip)) {
            return None;
        }
        let mut tmpword = e.strip.clone();
        tmpword.extend_from_slice(word.get(start + e.appnd.len()..start + len)?);
        if !test_condition_prefix(&e.cond, e.numconds, &tmpword) {
            return None;
        }
        let tmpl = tmpl + e.strip.len();
        let entries = self.index.entries_for(&tmpword);
        if !entries.is_empty() {
            for packed in entries {
                let he = self.index.view(packed);
                if he.flags.contains(&e.flag)
                    && !self.aff.need_affix.is_some_and(|f| e.cont.contains(&f))
                    && (needflag.is_none()
                        || he.flags.iter().any(|f| Some(*f) == needflag)
                        || e.cont.iter().any(|f| Some(*f) == needflag))
                {
                    return Some(he);
                }
            }
        }
        if e.cross {
            let pfx_id = if empty {
                PfxId::Empty(idx)
            } else {
                PfxId::Entry(idx)
            };
            if let Some(entry) = self.suffix_check(
                &tmpword,
                0,
                tmpl,
                AFFIX_XPRODUCT,
                Some(pfx_id),
                None,
                needflag,
                in_compound,
                state,
            ) {
                return Some(entry);
            }
        }
        None
    }

    /// `AffixMgr::prefix_check_twosfx`.
    fn prefix_check_twosfx(
        &self,
        word: &[u8],
        start: usize,
        len: usize,
        in_compound: InCompound,
        needflag: Option<u8>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        state.pfx = None;
        state.sfxappnd = None;
        for (idx, e) in self.aff.empty_prefixes.iter().enumerate() {
            if let Some(entry) = self.pfx_entry_check_twosfx(
                e,
                idx,
                true,
                word,
                start,
                len,
                in_compound,
                needflag,
                state,
            ) {
                return Some(entry);
            }
        }
        if start >= word.len() {
            return None;
        }
        let first = word[start];
        for (idx, e) in self.aff.prefixes.iter().enumerate() {
            if e.appnd.first().copied() != Some(first) || !is_subset(&e.appnd, &word[start..]) {
                continue;
            }
            if let Some(entry) = self.pfx_entry_check_twosfx(
                e,
                idx,
                false,
                word,
                start,
                len,
                in_compound,
                needflag,
                state,
            ) {
                state.pfx = Some(PfxId::Entry(idx));
                return Some(entry);
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn pfx_entry_check_twosfx(
        &self,
        e: &AffixEntry,
        idx: usize,
        empty: bool,
        word: &[u8],
        start: usize,
        len: usize,
        in_compound: InCompound,
        needflag: Option<u8>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        if len < e.appnd.len() {
            return None;
        }
        let tmpl = len - e.appnd.len();
        if !((tmpl > 0 || (tmpl == 0 && self.aff.fullstrip)) && tmpl + e.strip.len() >= e.numconds)
        {
            return None;
        }
        let mut tmpword = e.strip.clone();
        tmpword.extend_from_slice(word.get(start + e.appnd.len()..start + len)?);
        if !test_condition_prefix(&e.cond, e.numconds, &tmpword) {
            return None;
        }
        let tmpl = tmpl + e.strip.len();
        if e.cross && in_compound != InCompound::Begin {
            let pfx_id = if empty {
                PfxId::Empty(idx)
            } else {
                PfxId::Entry(idx)
            };
            return self.suffix_check_twosfx(
                &tmpword,
                0,
                tmpl,
                AFFIX_XPRODUCT,
                Some(pfx_id),
                needflag,
                state,
            );
        }
        None
    }

    /// `AffixMgr::suffix_check`.
    #[allow(clippy::too_many_arguments)]
    fn suffix_check(
        &self,
        word: &[u8],
        start: usize,
        len: usize,
        sfxopts: u32,
        ppfx: Option<PfxId>,
        cclass: Option<u8>,
        needflag: Option<u8>,
        in_compound: InCompound,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        for (idx, e) in self.aff.empty_suffixes.iter().enumerate() {
            if cclass.is_some() && e.cont.is_empty() {
                continue;
            }
            if !self.suffix_entry_allowed(e, ppfx, cclass, in_compound) {
                continue;
            }
            let badflag = if in_compound == InCompound::Not {
                self.aff.only_in_compound
            } else {
                None
            };
            if let Some(entry) = self.sfx_entry_checkword(
                e, word, start, len, sfxopts, ppfx, cclass, needflag, badflag, state,
            ) {
                state.sfx = Some(SfxId::Empty(idx));
                return Some(entry);
            }
        }
        if len == 0 {
            return None;
        }
        let last = word[start + len - 1];
        for (idx, e) in self.aff.suffixes.iter().enumerate() {
            if e.appnd.last().copied() != Some(last) {
                continue;
            }
            if !appnd_matches_word_end(&e.appnd, &word[start..start + len]) {
                continue;
            }
            if !self.suffix_entry_allowed(e, ppfx, cclass, in_compound) {
                continue;
            }
            if in_compound == InCompound::End
                && ppfx.is_none()
                && self
                    .aff
                    .only_in_compound
                    .is_some_and(|f| e.cont.contains(&f))
            {
                continue;
            }
            let badflag = if in_compound == InCompound::Not {
                self.aff.only_in_compound
            } else {
                None
            };
            if let Some(entry) = self.sfx_entry_checkword(
                e, word, start, len, sfxopts, ppfx, cclass, needflag, badflag, state,
            ) {
                state.sfx = Some(SfxId::Entry(idx));
                state.sfxflag = Some(e.flag);
                if e.cont.is_empty() {
                    state.sfxappnd = Some(e.appnd.clone());
                }
                return Some(entry);
            }
        }
        None
    }

    fn suffix_entry_allowed(
        &self,
        e: &AffixEntry,
        ppfx: Option<PfxId>,
        cclass: Option<u8>,
        in_compound: InCompound,
    ) -> bool {
        let permit = self
            .aff
            .compound_permit
            .is_some_and(|f| e.cont.contains(&f));
        if in_compound == InCompound::Begin && !permit {
            return false;
        }
        if let Some(circum) = self.aff.circumfix {
            let pfx_has = ppfx
                .map(|p| self.pfx_entry(p).cont.contains(&circum))
                .unwrap_or(false);
            let sfx_has = e.cont.contains(&circum);
            let no_pfx_flag = !ppfx
                .map(|p| self.pfx_entry(p).cont.contains(&circum))
                .unwrap_or(false);
            if !((no_pfx_flag && !sfx_has) || (pfx_has && sfx_has)) {
                return false;
            }
        }
        if in_compound == InCompound::Not
            && self
                .aff
                .only_in_compound
                .is_some_and(|f| e.cont.contains(&f))
        {
            return false;
        }
        if cclass.is_none() {
            let sfx_needaffix = self.aff.need_affix.is_some_and(|f| e.cont.contains(&f));
            let pfx_needaffix = ppfx
                .map(|p| {
                    self.aff
                        .need_affix
                        .is_some_and(|f| self.pfx_entry(p).cont.contains(&f))
                })
                .unwrap_or(false);
            if sfx_needaffix && !pfx_needaffix {
                return false;
            }
        }
        true
    }

    fn pfx_entry(&self, p: PfxId) -> &AffixEntry {
        match p {
            PfxId::Entry(i) => &self.aff.prefixes[i],
            PfxId::Empty(i) => &self.aff.empty_prefixes[i],
        }
    }

    fn sfx_entry(&self, s: SfxId) -> &AffixEntry {
        match s {
            SfxId::Entry(i) => &self.aff.suffixes[i],
            SfxId::Empty(i) => &self.aff.empty_suffixes[i],
        }
    }

    /// `SfxEntry::checkword`.
    #[allow(clippy::too_many_arguments)]
    fn sfx_entry_checkword(
        &self,
        e: &AffixEntry,
        word: &[u8],
        start: usize,
        len: usize,
        optflags: u32,
        ppfx: Option<PfxId>,
        cclass: Option<u8>,
        needflag: Option<u8>,
        badflag: Option<u8>,
        _state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        if optflags & AFFIX_XPRODUCT != 0 && !e.cross {
            return None;
        }
        if len < e.appnd.len() {
            return None;
        }
        let tmpl = len - e.appnd.len();
        if !((tmpl > 0 || (tmpl == 0 && self.aff.fullstrip)) && tmpl + e.strip.len() >= e.numconds)
        {
            return None;
        }
        let mut tmpstring = word[start..start + tmpl].to_vec();
        tmpstring.extend_from_slice(&e.strip);
        if !test_condition_suffix(&e.cond, e.numconds, &tmpstring) {
            return None;
        }
        let ep_flag = ppfx.map(|p| self.pfx_entry(p).flag);
        let ep_cont = ppfx.map(|p| self.pfx_entry(p).cont.clone());
        let entries = self.index.entries_for(&tmpstring);
        if !entries.is_empty() {
            for packed in entries {
                let he = self.index.view(packed);
                let cond_suffix = he.flags.contains(&e.flag)
                    || ep_cont.as_ref().is_some_and(|c| c.contains(&e.flag));
                if !cond_suffix {
                    continue;
                }
                if optflags & AFFIX_XPRODUCT != 0 {
                    let via_he = ep_flag.is_some_and(|f| he.flags.contains(&f));
                    let via_cont = ep_flag.is_some_and(|f| e.cont.contains(&f));
                    if !(via_he || via_cont) {
                        continue;
                    }
                }
                if let Some(cc) = cclass {
                    if !e.cont.contains(&cc) {
                        continue;
                    }
                }
                if let Some(bf) = badflag {
                    if he.flags.contains(&bf) {
                        continue;
                    }
                }
                if let Some(nf) = needflag {
                    if !(he.flags.contains(&nf) || e.cont.contains(&nf)) {
                        continue;
                    }
                }
                return Some(he);
            }
        }
        None
    }

    /// `AffixMgr::suffix_check_twosfx`.
    #[allow(clippy::too_many_arguments)]
    fn suffix_check_twosfx(
        &self,
        word: &[u8],
        start: usize,
        len: usize,
        sfxopts: u32,
        ppfx: Option<PfxId>,
        needflag: Option<u8>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        for (idx, e) in self.aff.empty_suffixes.iter().enumerate() {
            if !self.aff.cont_classes[e.flag as usize] {
                continue;
            }
            if let Some(entry) = self.sfx_entry_check_twosfx(
                e,
                SfxId::Empty(idx),
                word,
                start,
                len,
                sfxopts,
                ppfx,
                needflag,
                state,
            ) {
                return Some(entry);
            }
        }
        if len == 0 {
            return None;
        }
        let last = word[start + len - 1];
        for (idx, e) in self.aff.suffixes.iter().enumerate() {
            if e.appnd.last().copied() != Some(last)
                || !appnd_matches_word_end(&e.appnd, &word[start..start + len])
            {
                continue;
            }
            if !self.aff.cont_classes[e.flag as usize] {
                continue;
            }
            if let Some(entry) = self.sfx_entry_check_twosfx(
                e,
                SfxId::Entry(idx),
                word,
                start,
                len,
                sfxopts,
                ppfx,
                needflag,
                state,
            ) {
                state.sfxflag = Some(e.flag);
                if e.cont.is_empty() {
                    state.sfxappnd = Some(e.appnd.clone());
                }
                return Some(entry);
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn sfx_entry_check_twosfx(
        &self,
        e: &AffixEntry,
        _sfx_id: SfxId,
        word: &[u8],
        start: usize,
        len: usize,
        optflags: u32,
        ppfx: Option<PfxId>,
        needflag: Option<u8>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        if optflags & AFFIX_XPRODUCT != 0 && !e.cross {
            return None;
        }
        if len < e.appnd.len() {
            return None;
        }
        let tmpl = len - e.appnd.len();
        if !((tmpl > 0 || (tmpl == 0 && self.aff.fullstrip)) && tmpl + e.strip.len() >= e.numconds)
        {
            return None;
        }
        let mut tmpword = word[start..start + tmpl].to_vec();
        tmpword.extend_from_slice(&e.strip);
        let tmpl = tmpl + e.strip.len();
        if !test_condition_suffix(&e.cond, e.numconds, &tmpword) {
            return None;
        }
        if let Some(p) = ppfx {
            let conditional = e.cont.contains(&self.pfx_entry(p).flag);
            if conditional {
                self.suffix_check(
                    &tmpword,
                    0,
                    tmpl,
                    0,
                    None,
                    Some(e.flag),
                    needflag,
                    InCompound::Not,
                    state,
                )
            } else {
                self.suffix_check(
                    &tmpword,
                    0,
                    tmpl,
                    optflags,
                    ppfx,
                    Some(e.flag),
                    needflag,
                    InCompound::Not,
                    state,
                )
            }
        } else {
            self.suffix_check(
                &tmpword,
                0,
                tmpl,
                0,
                None,
                Some(e.flag),
                needflag,
                InCompound::Not,
                state,
            )
        }
    }

    /// `AffixMgr::compound_check`, restricted to the feature set used by the
    /// German dictionaries (no COMPOUNDRULE, no CHECKCOMPOUNDPATTERN, no
    /// SIMPLIFIEDTRIPLE/CHECKCOMPOUNDTRIPLE, no COMPOUNDWORDMAX).
    #[allow(clippy::only_used_in_recursion)]
    fn compound_check(
        &self,
        word: &[u8],
        wordnum_in: usize,
        numsyllable: usize,
        maxwordnum: usize,
        _wnum: usize,
        info: &mut Info,
    ) -> Option<DicEntry<'_>> {
        let (cmin, cmax) = setcminmax(self.aff.compound_min, word);
        let st = word.to_vec();
        let mut wordnum = wordnum_in;
        let mut i = cmin;
        while i < cmax {
            while i < st.len() && st[i] & 0xC0 == 0x80 {
                i += 1;
            }
            if i >= cmax {
                break;
            }
            let oldwordnum = wordnum;
            let _ = oldwordnum;
            let mut checked_prefix = false;
            let mut state = AffixState::default();
            // --- FIRST WORD ---
            let first_part = &st[..i];
            let mut rv: Option<DicEntry<'_>> = None;
            let entries = self.index.entries_for(first_part);
            if !entries.is_empty() {
                let mut it = entries.iter();
                let mut candidate = it.next().map(|e| self.index.view(e));
                // dictionary stems with COMPOUNDFORBIDFLAG never start a
                // compound
                if candidate.is_some_and(|e| {
                    self.aff
                        .compound_forbid
                        .is_some_and(|f| e.flags.contains(&f))
                }) {
                    i += 1;
                    continue;
                }
                while let Some(e) = candidate {
                    let skip = self.aff.need_affix.is_some_and(|f| e.flags.contains(&f))
                        || !(self.aff.compound_flag.is_some_and(|f| e.flags.contains(&f))
                            || (wordnum == 0
                                && self
                                    .aff
                                    .compound_begin
                                    .is_some_and(|f| e.flags.contains(&f)))
                            || (wordnum > 0
                                && self
                                    .aff
                                    .compound_middle
                                    .is_some_and(|f| e.flags.contains(&f))));
                    if !skip {
                        rv = Some(e);
                        break;
                    }
                    candidate = it.next().map(|e| self.index.view(e));
                }
            }
            if rv.is_none() {
                // affixed first word with the compound role flag
                let role = if wordnum == 0 {
                    self.aff.compound_begin
                } else {
                    self.aff.compound_middle
                };
                if let Some(nf) = role {
                    let mut found = self.suffix_check(
                        first_part,
                        0,
                        i,
                        0,
                        None,
                        None,
                        Some(nf),
                        InCompound::Begin,
                        &mut state,
                    );
                    if found.is_none() {
                        found = self.prefix_check(
                            first_part,
                            0,
                            i,
                            InCompound::Begin,
                            Some(nf),
                            &mut state,
                        );
                    }
                    if found.is_some() {
                        rv = found;
                        checked_prefix = true;
                    }
                }
            } else if let Some(entry) = rv {
                if self.has_flag(entry, self.aff.forbidden_word)
                    || self.has_flag(entry, self.aff.need_affix)
                    || entry.only_upcase
                {
                    i += 1;
                    continue;
                }
            }
            // pfx/sfx continuation flags
            if let Some(entry) = rv {
                let sfx_forbid = state
                    .sfx
                    .map(|s| {
                        self.aff
                            .compound_forbid
                            .is_some_and(|f| self.sfx_entry(s).cont.contains(&f))
                    })
                    .unwrap_or(false);
                let pfx_forbid = state
                    .pfx
                    .map(|p| {
                        self.aff
                            .compound_forbid
                            .is_some_and(|f| self.pfx_entry(p).cont.contains(&f))
                    })
                    .unwrap_or(false);
                if sfx_forbid || pfx_forbid {
                    i += 1;
                    continue;
                }
                let sfx_end = state
                    .sfx
                    .map(|s| {
                        self.aff
                            .compound_end
                            .is_some_and(|f| self.sfx_entry(s).cont.contains(&f))
                    })
                    .unwrap_or(false);
                let pfx_end = state
                    .pfx
                    .map(|p| {
                        self.aff
                            .compound_end
                            .is_some_and(|f| self.pfx_entry(p).cont.contains(&f))
                    })
                    .unwrap_or(false);
                if !checked_prefix && self.aff.compound_end.is_some() && (sfx_end || pfx_end) {
                    i += 1;
                    continue;
                }
                let sfx_mid = state
                    .sfx
                    .map(|s| {
                        self.aff
                            .compound_middle
                            .is_some_and(|f| self.sfx_entry(s).cont.contains(&f))
                    })
                    .unwrap_or(false);
                let pfx_mid = state
                    .pfx
                    .map(|p| {
                        self.aff
                            .compound_middle
                            .is_some_and(|f| self.pfx_entry(p).cont.contains(&f))
                    })
                    .unwrap_or(false);
                if !checked_prefix
                    && wordnum == 0
                    && self.aff.compound_middle.is_some()
                    && (sfx_mid || pfx_mid)
                {
                    i += 1;
                    continue;
                }
                if self.has_flag(entry, self.aff.forbidden_word) || entry.only_upcase {
                    return None;
                }
                if self
                    .aff
                    .compound_root
                    .is_some_and(|f| entry.flags.contains(&f))
                {
                    wordnum += 1;
                }
                // is the first word acceptable in compounds?
                let role_ok = checked_prefix
                    || self
                        .aff
                        .compound_flag
                        .is_some_and(|f| entry.flags.contains(&f))
                    || (oldwordnum == 0
                        && self
                            .aff
                            .compound_begin
                            .is_some_and(|f| entry.flags.contains(&f)))
                    || (oldwordnum > 0
                        && self
                            .aff
                            .compound_middle
                            .is_some_and(|f| entry.flags.contains(&f)));
                if !role_ok {
                    i += 1;
                    continue;
                }
                // --- SECOND WORD ---
                let rv_first = entry;
                let second_start = i;
                if second_start >= st.len() {
                    i += 1;
                    continue;
                }
                let second = &st[second_start..];
                let mut rv2: Option<DicEntry<'_>> = None;
                let entries = self.index.entries_for(second);
                if !entries.is_empty() {
                    for packed in entries {
                        let e = self.index.view(packed);
                        let skip = self.aff.need_affix.is_some_and(|f| e.flags.contains(&f))
                            || !(self.aff.compound_flag.is_some_and(|f| e.flags.contains(&f))
                                || self.aff.compound_end.is_some_and(|f| e.flags.contains(&f)));
                        if !skip {
                            rv2 = Some(e);
                            break;
                        }
                    }
                }
                if let Some(entry2) = rv2 {
                    if self.has_flag(entry2, self.aff.forbidden_word) || entry2.only_upcase {
                        return None;
                    }
                    if (self
                        .aff
                        .compound_flag
                        .is_some_and(|f| entry2.flags.contains(&f))
                        || self
                            .aff
                            .compound_end
                            .is_some_and(|f| entry2.flags.contains(&f)))
                        && wordnum + 1 < 100
                    {
                        if self.cpdwordpair_check(word) {
                            return None;
                        }
                        return Some(rv_first);
                    }
                }
                // second word with affixes
                if let Some(nf) = self.aff.compound_end {
                    let mut state2 = AffixState::default();
                    if let Some(entry2) = self.affix_check(
                        &st,
                        second_start,
                        st.len() - second_start,
                        Some(nf),
                        InCompound::End,
                        &mut state2,
                    ) {
                        let forbid = state2
                            .sfx
                            .map(|s| {
                                self.aff
                                    .compound_forbid
                                    .is_some_and(|f| self.sfx_entry(s).cont.contains(&f))
                            })
                            .unwrap_or(false)
                            || state2
                                .pfx
                                .map(|p| {
                                    self.aff
                                        .compound_forbid
                                        .is_some_and(|f| self.pfx_entry(p).cont.contains(&f))
                                })
                                .unwrap_or(false);
                        if !forbid {
                            if self.has_flag(entry2, self.aff.forbidden_word) || entry2.only_upcase
                            {
                                return None;
                            }
                            if self.cpdwordpair_check(word) {
                                return None;
                            }
                            return Some(rv_first);
                        }
                    }
                }
                // recursive compound of the rest
                if !info.compound_2 && wordnum + 2 < maxwordnum {
                    let mut rec_info = *info;
                    if let Some(rec) = self.compound_check(
                        second,
                        wordnum + 1,
                        numsyllable,
                        maxwordnum,
                        0,
                        &mut rec_info,
                    ) {
                        let end = second_start + rec.word_len;
                        if end <= st.len() && self.cpdwordpair_check(&st[..end]) {
                            return None;
                        }
                        // first-part check with forbidden dictionary entries
                        if let Some(forbidden) = self.aff.forbidden_word {
                            if end <= st.len() {
                                if let Some(rv2) = self.lookup_first(&st[..end]) {
                                    if rv2.flags.contains(&forbidden) {
                                        return None;
                                    }
                                }
                            }
                        }
                        return Some(rv_first);
                    }
                }
            }
            i += 1;
        }
        None
    }

    /// `AffixMgr::cpdwordpair_check` + `candidate_check`.
    fn cpdwordpair_check(&self, word: &[u8]) -> bool {
        if word.len() <= 2 {
            return false;
        }
        let mut candidate = word.to_vec();
        let mut i = 1;
        while i < candidate.len() {
            if candidate[i] & 0xC0 == 0x80 {
                i += 1;
                continue;
            }
            candidate.insert(i, b' ');
            let hit = !self.index.entries_for(&candidate).is_empty() || {
                let mut st = AffixState::default();
                self.affix_check(
                    &candidate,
                    0,
                    candidate.len(),
                    None,
                    InCompound::Not,
                    &mut st,
                )
                .is_some()
            };
            if hit {
                return true;
            }
            candidate.remove(i);
            i += 1;
        }
        false
    }

    /// `spell_internal`'s `BREAK` recursion.
    fn spell_break(&self, scw: &[u8]) -> bool {
        if self.aff.break_patterns.is_empty() || scw.is_empty() {
            return false;
        }
        // break points can exceed the recursion limit of 10 per hunspell
        let mut nbr = 0;
        for j in &self.aff.break_patterns {
            let mut pos = 0;
            while let Some(found) = find_sub(scw, j, pos) {
                nbr += 1;
                pos = found + j.len();
            }
        }
        if nbr >= 10 {
            return false;
        }
        // boundary patterns (^begin and end$)
        for j in &self.aff.break_patterns {
            let plen = j.len();
            if plen == 1 || plen > scw.len() {
                continue;
            }
            if j[0] == b'^' && scw.starts_with(&j[1..]) {
                let rest = self.spell_internal(&scw[plen - 1..]);
                if rest {
                    return true;
                }
            }
            if j[plen - 1] == b'$' && scw.ends_with(&j[..plen - 1]) {
                let prefix = &scw[..scw.len() - plen + 1];
                if self.spell_internal(prefix) {
                    return true;
                }
            }
        }
        // other patterns: try the second occurrence first
        for j in &self.aff.break_patterns {
            let plen = j.len();
            let Some(mut found) = find_sub(scw, j, 0) else {
                continue;
            };
            if found > 0 && found < scw.len().saturating_sub(plen) {
                if let Some(found2) = find_sub(scw, j, found + 1) {
                    if found2 > 0 && found2 < scw.len() - plen {
                        found = found2;
                    }
                }
                let rest = &scw[found + plen..];
                if !self.spell_internal(rest) {
                    continue;
                }
                if self.spell_internal(&scw[..found]) {
                    return true;
                }
            }
        }
        // other patterns: first occurrence
        for j in &self.aff.break_patterns {
            let plen = j.len();
            let Some(found) = find_sub(scw, j, 0) else {
                continue;
            };
            if found > 0 && found < scw.len() - plen {
                if !self.spell_internal(&scw[found + plen..]) {
                    continue;
                }
                if self.spell_internal(&scw[..found]) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
enum Captype {
    NoCap,
    InitCap,
    AllCap,
    HuhCap,
    HuhInitCap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InCompound {
    Not,
    Begin,
    End,
}

#[derive(Debug, Clone, Copy)]
enum PfxId {
    Entry(usize),
    Empty(usize),
}

#[derive(Debug, Clone, Copy)]
enum SfxId {
    Entry(usize),
    Empty(usize),
}

const AFFIX_XPRODUCT: u32 = 1;

#[derive(Debug, Clone, Copy, Default)]
struct Info {
    forbidden: bool,
    init_cap: bool,
    orig_cap: bool,
    compound_2: bool,
    compound: bool,
}

#[derive(Debug, Default)]
struct AffixState {
    pfx: Option<PfxId>,
    sfx: Option<SfxId>,
    sfxappnd: Option<Vec<u8>>,
    sfxflag: Option<u8>,
}

fn find_sub(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > haystack.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find_sub(haystack, needle, 0).is_some()
}

fn skip_leading_bytes(word: &[u8], b: u8) -> &[u8] {
    let mut i = 0;
    while i < word.len() && word[i] == b {
        i += 1;
    }
    &word[i..]
}

fn char_len(word: &[u8]) -> usize {
    if word.is_empty() {
        return 0;
    }
    1 + word[1..].iter().take_while(|&&c| c & 0xC0 == 0x80).count()
}

/// `setcminmax` (UTF-8 path).
fn setcminmax(cpdmin: usize, word: &[u8]) -> (usize, usize) {
    let len = word.len();
    let mut cmin = 0;
    let mut i = 0;
    while i < cpdmin && cmin < len {
        cmin += 1;
        while cmin < len && word[cmin] & 0xC0 == 0x80 {
            cmin += 1;
        }
        i += 1;
    }
    let mut cmax = len;
    let mut i = 0;
    while i < cpdmin.saturating_sub(1) && cmax > 0 {
        cmax -= 1;
        while cmax > 0 && word[cmax] & 0xC0 == 0x80 {
            cmax -= 1;
        }
        i += 1;
    }
    (cmin, cmax)
}

/// `isSubset` (dots match any byte).
fn is_subset(s1: &[u8], s2: &[u8]) -> bool {
    let mut i = 0;
    while i < s1.len() && i < s2.len() && (s1[i] == s2[i] || s1[i] == b'.') {
        i += 1;
    }
    i == s1.len()
}

/// `isRevSubset` against the *reversed* affix key: the affix string matches
/// the end of `s2` (with '.' matching any byte).
fn appnd_matches_word_end(appnd: &[u8], s2: &[u8]) -> bool {
    let mut ai = appnd.len();
    let mut si = s2.len();
    while ai > 0 && si > 0 && (appnd[ai - 1] == s2[si - 1] || appnd[ai - 1] == b'.') {
        ai -= 1;
        si -= 1;
    }
    ai == 0
}

/// `PfxEntry::test_condition`: forward walk over `word`.
fn test_condition_prefix(cond: &[u8], numconds: usize, word: &[u8]) -> bool {
    if numconds == 0 {
        return true;
    }
    let mut st = 0usize;
    let mut pos: Option<usize> = None;
    let mut neg = false;
    let mut ingroup = false;
    let mut p = 0usize;
    loop {
        let Some(&c) = cond.get(p) else { return true };
        match c {
            b'[' => {
                neg = false;
                ingroup = false;
                p += 1;
                pos = Some(st);
            }
            b'^' => {
                p += 1;
                neg = true;
            }
            b']' => {
                if neg == ingroup {
                    return false;
                }
                pos = None;
                p += 1;
                if !ingroup && st < word.len() {
                    st += 1;
                    while st < word.len() && word[st] & 0xC0 == 0x80 {
                        st += 1;
                    }
                }
                if st == word.len() && p < cond.len() {
                    return false;
                }
            }
            b'.' if pos.is_none() => {
                p += 1;
                st += 1;
                while st < word.len() && word[st] & 0xC0 == 0x80 {
                    st += 1;
                }
                if st == word.len() && p < cond.len() {
                    return false;
                }
            }
            _ => {
                if st < word.len() && word[st] == c {
                    st += 1;
                    p += 1;
                    if word[st - 1] & 0x80 != 0 {
                        while p < cond.len() && cond[p] & 0xC0 == 0x80 {
                            if cond[p] != word.get(st).copied().unwrap_or(0) {
                                if pos.is_none() {
                                    return false;
                                }
                                st = pos.unwrap();
                                break;
                            }
                            p += 1;
                            st += 1;
                        }
                        if pos.is_some() && Some(st) != pos {
                            ingroup = true;
                            while p < cond.len() && cond[p] != b']' {
                                p += 1;
                            }
                        }
                    } else if pos.is_some() {
                        ingroup = true;
                        while p < cond.len() && cond[p] != b']' {
                            p += 1;
                        }
                    }
                } else if pos.is_some() {
                    p += 1;
                } else {
                    return false;
                }
            }
        }
        if p >= cond.len() {
            return true;
        }
    }
}

/// `SfxEntry::test_condition`: backward walk over `word`.
fn test_condition_suffix(cond: &[u8], numconds: usize, word: &[u8]) -> bool {
    if numconds == 0 {
        return true;
    }
    if word.is_empty() {
        return false;
    }
    let beg: isize = 0;
    let mut st: isize = word.len() as isize - 1;
    let mut pos: Option<isize> = None;
    let mut neg = false;
    let mut ingroup = false;
    let mut p = 0usize;
    loop {
        let Some(&c) = cond.get(p) else { return true };
        match c {
            b'[' => {
                p += 1;
                pos = Some(st);
            }
            b'^' => {
                p += 1;
                neg = true;
            }
            b']' => {
                if !neg && !ingroup {
                    return false;
                }
                if !ingroup {
                    while st >= beg && word[st as usize] & 0xC0 == 0x80 {
                        st -= 1;
                    }
                    st -= 1;
                }
                pos = None;
                neg = false;
                ingroup = false;
                p += 1;
                if st < beg && p < cond.len() {
                    return false;
                }
            }
            b'.' if pos.is_none() => {
                p += 1;
                st -= 1;
                while st >= beg && word[st as usize] & 0xC0 == 0x80 {
                    st -= 1;
                }
                if st < beg {
                    return p >= cond.len();
                }
                if word[st as usize] & 0x80 != 0 {
                    st -= 1;
                    if st < beg {
                        return p >= cond.len();
                    }
                }
            }
            _ => {
                if st >= beg && word[st as usize] == c {
                    p += 1;
                    if word[st as usize] & 0x80 != 0 {
                        st -= 1;
                        while p < cond.len() && cond[p] & 0xC0 == 0x80 && st >= beg {
                            if cond[p] != word[st as usize] {
                                if pos.is_none() {
                                    return false;
                                }
                                st = pos.unwrap();
                                break;
                            }
                            p += 1;
                            st -= 1;
                        }
                        if pos.is_some() && Some(st) != pos {
                            if neg {
                                return false;
                            } else if p == cond.len() {
                                return true;
                            }
                            ingroup = true;
                            while p < cond.len() && cond[p] != b']' {
                                p += 1;
                            }
                            st -= 1;
                        }
                    } else if pos.is_some() {
                        if neg {
                            return false;
                        } else if p == cond.len() {
                            return true;
                        }
                        ingroup = true;
                        while p < cond.len() && cond[p] != b']' {
                            p += 1;
                        }
                        st -= 1;
                    }
                    if pos.is_none() {
                        st -= 1;
                    }
                    if st < beg && p < cond.len() && cond.get(p) != Some(&b']') {
                        return false;
                    }
                } else if pos.is_some() {
                    p += 1;
                } else {
                    return false;
                }
            }
        }
        if p >= cond.len() {
            return true;
        }
    }
}

/// `get_captype_utf8` with hunspell's simple (single-code-point) case
/// mapping: multi-character mappings keep the original character, matching
/// hunspell's `utf_tbl` (e.g. ß has no uppercase form).
fn captype_utf8(word: &[u8]) -> Captype {
    let s = String::from_utf8_lossy(word);
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return Captype::NoCap;
    }
    let mut ncap = 0usize;
    let mut nneutral = 0usize;
    for &c in &chars {
        let lower = simple_lower(c);
        if c != lower {
            ncap += 1;
        }
        if simple_upper(c) == lower {
            nneutral += 1;
        }
    }
    let firstcap = chars[0] != simple_lower(chars[0]);
    if ncap == 0 {
        Captype::NoCap
    } else if ncap == 1 && firstcap {
        Captype::InitCap
    } else if ncap == chars.len() || ncap + nneutral == chars.len() {
        Captype::AllCap
    } else if ncap > 1 && firstcap {
        Captype::HuhInitCap
    } else {
        Captype::HuhCap
    }
}

fn simple_lower(c: char) -> char {
    let mut it = c.to_lowercase();
    let first = it.next().unwrap_or(c);
    if it.next().is_some() {
        c
    } else {
        first
    }
}

fn simple_upper(c: char) -> char {
    let mut it = c.to_uppercase();
    let first = it.next().unwrap_or(c);
    if it.next().is_some() {
        c
    } else {
        first
    }
}

fn lowercase_bytes(word: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(word)
        .chars()
        .map(simple_lower)
        .collect::<String>()
        .into_bytes()
}

fn upper_bytes(word: &[u8]) -> Vec<u8> {
    String::from_utf8_lossy(word)
        .chars()
        .map(simple_upper)
        .collect::<String>()
        .into_bytes()
}

fn initcap_bytes(word: &mut [u8]) {
    if word.is_empty() {
        return;
    }
    let s = String::from_utf8_lossy(word).into_owned();
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        let up = simple_upper(first).to_string() + chars.as_str();
        let bytes = up.into_bytes();
        let n = bytes.len().min(word.len());
        word[..n].copy_from_slice(&bytes[..n]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subset_helpers() {
        assert!(is_subset(b"ab", b"abc"));
        assert!(is_subset(b"a.", b"ab"));
        assert!(!is_subset(b"ab", b"a"));
        assert!(appnd_matches_word_end(b"bc", b"abc"));
        assert!(appnd_matches_word_end(b".c", b"abc"));
        assert!(!appnd_matches_word_end(b"ac", b"abc"));
    }
}
