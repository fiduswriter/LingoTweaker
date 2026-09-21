//! Faithful port of the hunspell 1.7.2 spelling checker (`spell()` +
//! affix/compound/break logic) as used by the legacy German speller
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
//! `ICONV`/`OCONV`, `IGNORE`, `COMPOUNDRULE`,
//! `CHECKCOMPOUNDPATTERN`/`CHECKCOMPOUNDREP`/`CHECKCOMPOUNDTRIPLE`/
//! `CHECKCOMPOUNDCASE`/`CHECKCOMPOUNDDUP`/`COMPOUNDWORDMAX`. None of them
//! occur (uncommented) in the German `.aff` files. The `FLAG` modes `char`
//! (default), `long`, `num` and `UTF-8` are supported (`Flag = u16`).

use std::path::Path;

use lt_core::{CoreError, Result};

const MAXWORDLEN: usize = 100;
const MAXWORDUTF8LEN: usize = MAXWORDLEN * 3;
const MAXSHARPS: usize = 5;

/// One affix/dictionary flag (`unsigned short` in hunspell).
type Flag = u16;
/// Hunspell's `DEFAULTFLAGS` (flag ids at or above this are dropped).
const DEFAULTFLAGS: u32 = 65510;
/// `AffixMgr::contclasses[CONTSIZE]` (`atypes.hxx`).
const CONTSIZE: usize = 65536;

/// Join two byte slices into a stack buffer (falling back to a heap `Vec`
/// only for pathologically long words). Mirrors hunspell's per-entry
/// `std::string` without Rust's unconditional heap allocation, which made the
/// suffix scan ~100x slower than the reference.
macro_rules! joined_slice {
    ($buf:ident, $heap:ident, $a:expr, $b:expr) => {{
        let a: &[u8] = $a;
        let b: &[u8] = $b;
        let total = a.len() + b.len();
        if total <= $buf.len() {
            $buf[..a.len()].copy_from_slice(a);
            $buf[a.len()..total].copy_from_slice(b);
            &$buf[..total]
        } else {
            $heap = {
                let mut v = a.to_vec();
                v.extend_from_slice(b);
                v
            };
            &$heap
        }
    }};
}

/// The suggestion manager state for one checker (UTF-8 dictionaries only).
struct SuggestMgr<'a> {
    c: &'a HunspellChecker,
    ctry: &'a [u8],
    ckey: &'a [u8],
    max_sug: usize,
    nosplitsugs: bool,
    maxcpdsugs: i32,
    lang_with_dash_usage: bool,
    /// `info` bit field (`SPELL_COMPOUND` only, as the caller uses it).
    info: u32,
    /// Remaining `map_related` nodes for the current `MAP` generator run.
    /// The reference bounds this exponentially branching generator with a
    /// wall clock (`MINTIMER`/`TIMELIMIT`); we use a deterministic node budget
    /// so the parity gate is reproducible (documented in the internal notes).
    map_budget: u64,
}

/// `MAP` generator node budget (see [`SuggestMgr::map_budget`]).
const MAP_NODE_BUDGET: u64 = 5_000;

fn bytes_to_chars(b: &[u8]) -> Vec<char> {
    String::from_utf8_lossy(b).chars().collect()
}

fn chars_to_bytes(c: &[char]) -> Vec<u8> {
    c.iter().collect::<String>().into_bytes()
}

fn mkinitcap_str(s: &str) -> String {
    let mut it = s.chars();
    match it.next() {
        Some(first) => simple_upper(first).to_string() + it.as_str(),
        None => String::new(),
    }
}

fn mkinitsmall_str(s: &str) -> String {
    let mut it = s.chars();
    match it.next() {
        Some(first) => simple_lower(first).to_string() + it.as_str(),
        None => String::new(),
    }
}

fn mkallcap_str(s: &str) -> String {
    s.chars().map(simple_upper).collect()
}

fn mkallsmall_str(s: &str) -> String {
    s.chars().map(simple_lower).collect()
}

fn find_byte(hay: &[u8], needle: u8) -> Option<usize> {
    hay.iter().position(|&b| b == needle)
}

// --- n-gram suggestion scoring (`suggestmgr.cxx`, UTF-16 path on `char`) ---

const NGRAM_LONGER_WORSE: i32 = 1 << 0;
const NGRAM_ANY_MISMATCH: i32 = 1 << 1;
const NGRAM_WEIGHTED: i32 = 1 << 3;
/// `MAX_ROOTS` / `MAX_GUESS` / `MAX_WORDS` / `MAXNGRAMSUGS` (`suggestmgr.hxx`).
const MAX_ROOTS: usize = 100;
const MAX_GUESS: usize = 200;
const MAX_WORDS: usize = 100;
const MAXNGRAMSUGS: i32 = 4;

/// `SuggestMgr::ngram` (the `w_char` overload; BMP code points as `char`).
fn ngram_score(n: usize, su1: &[char], su2: &[char], opt: i32) -> i32 {
    let l1 = su1.len() as i32;
    let l2 = su2.len() as i32;
    if l2 == 0 {
        return 0;
    }
    let mut nscore = 0i32;
    let mut j = 1i32;
    while j <= n as i32 {
        let mut ns = 0i32;
        let mut i = 0i32;
        while i <= l1 - j {
            let mut found = false;
            let mut l = 0i32;
            while l <= l2 - j {
                let mut k = 0i32;
                while k < j {
                    if su1[(i + k) as usize] != su2[(l + k) as usize] {
                        break;
                    }
                    k += 1;
                }
                if k == j {
                    ns += 1;
                    found = true;
                    break;
                }
                l += 1;
            }
            if !found && (opt & NGRAM_WEIGHTED) != 0 {
                ns -= 1;
                if i == 0 || i == l1 - j {
                    ns -= 1; // side weight
                }
            }
            i += 1;
        }
        nscore += ns;
        if ns < 2 && (opt & NGRAM_WEIGHTED) == 0 {
            break;
        }
        j += 1;
    }
    let mut ns = 0i32;
    if opt & NGRAM_LONGER_WORSE != 0 {
        ns = (l2 - l1) - 2;
    }
    if opt & NGRAM_ANY_MISMATCH != 0 {
        ns = (l2 - l1).abs() - 2;
    }
    nscore - if ns > 0 { ns } else { 0 }
}

/// `SuggestMgr::leftcommonsubstring` (UTF-16 path): length of the left common
/// substring of `su1` and the decapitalized `su2`.
fn leftcommonsubstring(su1: &[char], su2: &[char], complexprefixes: bool) -> i32 {
    let l1 = su1.len();
    let l2 = su2.len();
    if complexprefixes {
        if l1 > 0 && l2 > 0 && su1[l1 - 1] == su2[l2 - 1] {
            return 1;
        }
        return 0;
    }
    let idx = if l2 > 0 { su2[0] } else { '\0' };
    let otheridx = if l1 > 0 { su1[0] } else { '\0' };
    if otheridx != idx && otheridx != simple_lower(idx) {
        return 0;
    }
    let mut i = 1;
    while i < l1 && i < l2 && su1[i] == su2[i] {
        i += 1;
    }
    i as i32
}

/// `SuggestMgr::commoncharacterpositions` (UTF-16 path): returns the number of
/// equal positions and whether the two differences are a transposition.
fn commoncharacterpositions(su1: &[char], su2: &[char]) -> (i32, bool) {
    let l1 = su1.len();
    let l2 = su2.len();
    if l1 == 0 || l2 == 0 {
        return (0, false);
    }
    let mut s2: Vec<char> = su2.to_vec();
    s2[0] = simple_lower(s2[0]);
    let mut num = 0i32;
    let mut diff = 0usize;
    let mut diffpos = [0usize; 2];
    let m = l1.min(l2);
    for i in 0..m {
        if su1[i] == s2[i] {
            num += 1;
        } else {
            if diff < 2 {
                diffpos[diff] = i;
            }
            diff += 1;
        }
    }
    let is_swap = diff == 2
        && l1 == l2
        && su1[diffpos[0]] == s2[diffpos[1]]
        && su1[diffpos[1]] == s2[diffpos[0]];
    (num, is_swap)
}

/// `SuggestMgr::lcslen`: longest common subsequence length.
fn lcslen(s1: &[char], s2: &[char]) -> i32 {
    let m = s1.len();
    let n = s2.len();
    let mut prev = vec![0i32; n + 1];
    let mut cur = vec![0i32; n + 1];
    for i in 1..=m {
        for j in 1..=n {
            cur[j] = if s1[i - 1] == s2[j - 1] {
                prev[j - 1] + 1
            } else {
                prev[j].max(cur[j - 1])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[n]
}

/// One `AffixMgr::expand_rootword` result (`struct guessword`); `orig` is only
/// used by the (unported) `PHONE` path.
struct GuessWord {
    word: Vec<u8>,
    allow: bool,
}

fn insert_sug(slst: &mut Vec<String>, word: Vec<u8>) {
    slst.insert(0, String::from_utf8_lossy(&word).into_owned());
}

/// `HunspellImpl::cleanword2`: skip leading blanks, strip trailing periods
/// (recording the count), and compute the capitalization type.
fn cleanword2(src: &[u8]) -> (Vec<u8>, Captype, usize) {
    let mut start = 0;
    while start < src.len() && src[start] == b' ' {
        start += 1;
    }
    let mut end = src.len();
    let mut abbv = 0;
    while end > start && src[end - 1] == b'.' {
        end -= 1;
        abbv += 1;
    }
    if end <= start {
        return (Vec::new(), Captype::NoCap, abbv);
    }
    let cleaned = src[start..end].to_vec();
    let captype = captype_utf8(&cleaned);
    (cleaned, captype, abbv)
}

/// `enum flag { FLAG_CHAR, FLAG_LONG, FLAG_NUM, FLAG_UNI }` (`hashmgr.hxx`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlagMode {
    /// Ispell's one-character flags (`erfg -> e r f g`).
    Char,
    /// Two-character flags (`1x2yZz -> 1x 2y Zz`).
    Long,
    /// Decimal numbers separated by commas (`4521,23,233`).
    Num,
    /// UTF-8 characters (stored as UTF-16 code units, like `w_char`).
    Uni,
}

/// One dictionary entry, decoded on demand from the raw `.dic` bytes
/// (`hentry` view: flags, ONLYUPCASE marker, byte length).
#[derive(Clone, Copy)]
struct DicEntry<'a> {
    flags: &'a [Flag],
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
    /// `hentry.var & H_OPT_INITCAP`: the entry's surface was `INITCAP` at
    /// `add_word` time (used by the n-gram generator's capitalization skip).
    initcap: bool,
    /// Creation order (`add_word` call order), used to reproduce
    /// `walk_hashtable`'s per-bucket order.
    seq: u32,
}

/// The dictionary kept in its raw (compressed) form: the `.dic` bytes are
/// stored as-is, an entry costs a 20-byte index record, and words/flags are
/// decoded per lookup (expanding `de_DE.dic` into a hash map cost ~90 MB,
/// D-033).
struct WordIndex {
    flags: Vec<Flag>,
    /// sorted by word bytes (stable: insertion order per word)
    entries: Vec<PackedEntry>,
    /// `word -> (start, end)` range in `entries`, so the hot
    /// `entries_for` lookup is a single hash map probe instead of a binary
    /// search (the suffix/affix candidate scan calls it millions of times).
    by_word: std::collections::HashMap<Vec<u8>, (u32, u32)>,
    /// Raw `.dic` bytes, for reconstructing entry words.
    dic: Vec<u8>,
    /// Indices into `entries` in `walk_hashtable` order: by hash bucket, then
    /// by creation order within the bucket.
    ngram_order: Vec<u32>,
}

impl WordIndex {
    fn word<'a>(dic: &'a [u8], e: &PackedEntry) -> &'a [u8] {
        let start = e.word_off as usize;
        &dic[start..start + e.word_len as usize]
    }

    /// Entries for `word` in insertion order (empty when absent).
    fn entries_for(&self, word: &[u8]) -> &[PackedEntry] {
        match self.by_word.get(word) {
            Some(&(a, b)) => &self.entries[a as usize..b as usize],
            None => &[],
        }
    }

    fn view(&self, e: &PackedEntry) -> DicEntry<'_> {
        let off = e.flags_off as usize;
        DicEntry {
            flags: &self.flags[off..off + e.flags_len as usize],
            only_upcase: e.only_upcase,
            word_len: e.word_len as usize,
        }
    }

    /// The entry's surface word (from the stored `.dic` bytes).
    fn entry_word<'a>(&'a self, e: &PackedEntry) -> &'a [u8] {
        Self::word(&self.dic, e)
    }
}

/// `HashMgr::hash`: `unsigned long` with the 5-bit rotate and hunspell's
/// signed-`char` XOR (`word[i]` is sign-extended on the reference platform).
fn hunspell_hash(word: &[u8], tablesize: usize) -> usize {
    let mut hv: u64 = 0;
    let mut i = 0;
    while i < 4 && i < word.len() {
        hv = (hv << 8) | (word[i] as i8 as i64 as u64);
        i += 1;
    }
    while i < word.len() {
        hv = (hv << 5) | ((hv >> 27) & 0x1f);
        hv ^= word[i] as i8 as i64 as u64;
        i += 1;
    }
    (hv % tablesize as u64) as usize
}

#[derive(Debug, Clone)]
struct AffixEntry {
    flag: Flag,
    cross: bool,
    strip: Vec<u8>,
    appnd: Vec<u8>,
    cont: Vec<Flag>,
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
    flag_mode: FlagMode,
    compound_flag: Option<Flag>,
    compound_begin: Option<Flag>,
    compound_middle: Option<Flag>,
    compound_end: Option<Flag>,
    compound_permit: Option<Flag>,
    compound_forbid: Option<Flag>,
    compound_root: Option<Flag>,
    compound_min: usize,
    forbidden_word: Option<Flag>,
    keep_case: Option<Flag>,
    need_affix: Option<Flag>,
    only_in_compound: Option<Flag>,
    circumfix: Option<Flag>,
    nosuggest: Option<Flag>,
    substandard: Option<Flag>,
    /// `NONGRAMSUGGEST`: reserved for the (deliberately unported) n-gram path.
    #[allow(dead_code)]
    nongramsuggest: Option<Flag>,
    forceucase: Option<Flag>,
    checksharps: bool,
    fullstrip: bool,
    have_cont_class: bool,
    cont_classes: Box<[bool; CONTSIZE]>,
    break_patterns: Vec<Vec<u8>>,
    /// `AffixMgr::parsedbreaktable`: a `BREAK` directive was present (even
    /// `BREAK 0`), suppressing the default hyphen break table.
    parsed_break: bool,
    /// `AffixMgr::try_string` (`TRY`): extra characters for candidate
    /// generation.
    try_string: Vec<u8>,
    /// `AffixMgr::key_string` (`KEY`): neighbouring keys for typo candidates.
    key_string: Vec<u8>,
    /// `HashMgr::reptable` (`REP`), in file order only (the dictionary-driven
    /// `ph:` entries are not loaded; the target dictionaries ship none).
    rep_table: Vec<RepEntry>,
    /// `AffixMgr::maptable` (`MAP`), each row a set of equivalent strings.
    map_table: Vec<Vec<Vec<u8>>>,
    /// Suffix lookup by exact `appnd`, mapping to indices into `suffixes`
    /// (ascending, preserving the flattened-tree order). Lets `suffix_check`
    /// test only the suffixes that can actually match the word ending instead
    /// of the whole last-byte bucket (`gl_ES` has 3946 `-s` suffixes).
    suffix_by_appnd: std::collections::HashMap<Vec<u8>, Vec<usize>>,
    /// Suffixes whose `appnd` contains a `.` wildcard (tested by full match).
    suffix_wild: Vec<usize>,
    /// Affixes in `.aff` file order, for `AffixMgr::expand_rootword`.
    suffixes_file: Vec<AffixEntry>,
    prefixes_file: Vec<AffixEntry>,
    /// `sFlag`/`pFlag` chains: affix indices per flag, in reverse file order
    /// (head insertion in `build_sfxtree`/`build_pfxtree`).
    sfx_by_flag: std::collections::HashMap<Flag, Vec<usize>>,
    pfx_by_flag: std::collections::HashMap<Flag, Vec<usize>>,
    /// `AffixMgr::nosplitsugs` (`NOSPLITSUGS`).
    nosplitsugs: bool,
    /// `AffixMgr::maxngramsugs` (`-1` = unset, hunspell's `MAXNGRAMSUGS`).
    maxngramsugs: i32,
    /// `AffixMgr::maxcpdsugs` (`-1` = unset, hunspell's `MAXCOMPOUNDSUGS`).
    maxcpdsugs: i32,
    /// `AffixMgr::onlymaxdiff` (`ONLYMAXDIFF`).
    onlymaxdiff: bool,
    /// `AffixMgr::maxdiff` (`-1` = unset).
    maxdiff: i32,
    /// `AffixMgr::sugswithdots` (`SUGSWITHDOTS`).
    sugswithdots: bool,
    /// `AffixMgr::langnum` (`LANG`); `0` when unset/unknown.
    langnum: u16,
}

/// One `REP` entry (`replentry`): `pattern` with `^`/`$` anchoring stripped,
/// and only the position-selected `outstrings[type]` populated.
#[derive(Debug, Clone, Default)]
struct RepEntry {
    pattern: Vec<u8>,
    out: [Vec<u8>; 4],
}

impl Default for Aff {
    fn default() -> Self {
        Aff {
            prefixes: Vec::new(),
            empty_prefixes: Vec::new(),
            suffixes: Vec::new(),
            empty_suffixes: Vec::new(),
            compound: false,
            flag_mode: FlagMode::Char,
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
            nosuggest: None,
            substandard: None,
            nongramsuggest: None,
            forceucase: None,
            checksharps: false,
            fullstrip: false,
            have_cont_class: false,
            cont_classes: Box::new([false; CONTSIZE]),
            break_patterns: Vec::new(),
            parsed_break: false,
            try_string: Vec::new(),
            key_string: Vec::new(),
            rep_table: Vec::new(),
            map_table: Vec::new(),
            suffix_by_appnd: std::collections::HashMap::new(),
            suffix_wild: Vec::new(),
            suffixes_file: Vec::new(),
            prefixes_file: Vec::new(),
            sfx_by_flag: std::collections::HashMap::new(),
            pfx_by_flag: std::collections::HashMap::new(),
            nosplitsugs: false,
            maxngramsugs: -1,
            maxcpdsugs: -1,
            onlymaxdiff: false,
            maxdiff: -1,
            sugswithdots: false,
            langnum: 0,
        }
    }
}

impl Aff {
    fn parse(text: &str) -> Result<Aff> {
        let mut aff = Aff::default();
        let mut pending: Vec<(bool, Flag, bool, AffixEntry)> = Vec::new();
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
                    let mode = aff.flag_mode;
                    let flag = decode_flag(mode, it.next().ok_or_else(|| parse_err(line))?);
                    let cross = it.next().map(|f| f == "Y").unwrap_or(false);
                    let count: usize = it
                        .next()
                        .and_then(|c| c.parse().ok())
                        .ok_or_else(|| parse_err(line))?;
                    for _ in 0..count {
                        let eline = lines.next().ok_or_else(|| parse_err(line))?;
                        let eline = eline.trim_end_matches('\r');
                        let entry = parse_affix_entry(eline, is_prefix, flag, cross, mode)?;
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
                aff.prefixes_file.push(entry.clone());
            } else {
                aff.suffixes_file.push(entry.clone());
            }
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
        // Index suffixes by exact `appnd` (and collect wildcard entries), so
        // `suffix_check` does not scan the whole first-byte bucket.
        for (i, e) in aff.suffixes.iter().enumerate() {
            if e.appnd.contains(&b'.') {
                aff.suffix_wild.push(i);
            } else {
                aff.suffix_by_appnd
                    .entry(e.appnd.clone())
                    .or_default()
                    .push(i);
            }
        }
        // `sFlag`/`pFlag` chains (reverse file order).
        for (i, e) in aff.suffixes_file.iter().enumerate() {
            aff.sfx_by_flag.entry(e.flag).or_default().push(i);
        }
        for v in aff.sfx_by_flag.values_mut() {
            v.reverse();
        }
        for (i, e) in aff.prefixes_file.iter().enumerate() {
            aff.pfx_by_flag.entry(e.flag).or_default().push(i);
        }
        for v in aff.pfx_by_flag.values_mut() {
            v.reverse();
        }
        // `AffixMgr::parse_file`: without a BREAK directive hunspell installs
        // the default hyphen break table.
        if !aff.parsed_break {
            aff.break_patterns = vec![b"-".to_vec(), b"^-".to_vec(), b"-$".to_vec()];
        }
        Ok(aff)
    }
}

fn parse_err(line: &str) -> CoreError {
    CoreError::Data(format!("hunspell affix parse error: {line:?}"))
}

/// `atoi` (skips leading whitespace, optional sign, stops at the first
/// non-digit; empty/no-digits -> 0).
fn parse_atoi(s: &str) -> u32 {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    let mut sign = 1i64;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        if b[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let mut value: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add((b[i] - b'0') as i64);
        i += 1;
    }
    (value * sign).clamp(0, u32::MAX as i64) as u32
}

/// `HashMgr::decode_flag` for one flag token.
fn decode_flag(mode: FlagMode, token: &str) -> Flag {
    let value = match mode {
        FlagMode::Long => {
            let b = token.as_bytes();
            let hi = b.first().copied().unwrap_or(0) as u32;
            let lo = b.get(1).copied().unwrap_or(0) as u32;
            (hi << 8) | lo
        }
        FlagMode::Num => parse_atoi(token),
        FlagMode::Uni => token.encode_utf16().next().map(|u| u as u32).unwrap_or(0),
        FlagMode::Char => token.as_bytes().first().copied().unwrap_or(0) as u32,
    };
    if value >= DEFAULTFLAGS {
        0
    } else {
        value as Flag
    }
}

/// `HashMgr::decode_flags` (`ap` is the text after the `/`).
fn decode_flags(mode: FlagMode, flags: &str) -> Vec<Flag> {
    if flags.is_empty() {
        return Vec::new();
    }
    match mode {
        FlagMode::Long => flags
            .as_bytes()
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (((c[0] as u32) << 8) | c[1] as u32) as u16)
            .collect(),
        FlagMode::Num => flags
            .split(',')
            .map(parse_atoi)
            .map(|v| v as Flag)
            .collect(),
        FlagMode::Uni => flags.encode_utf16().collect(),
        FlagMode::Char => flags.as_bytes().iter().map(|&b| b as Flag).collect(),
    }
}

fn parse_affix_entry(
    line: &str,
    is_prefix: bool,
    flag: Flag,
    cross: bool,
    mode: FlagMode,
) -> Result<AffixEntry> {
    let mut it = line.split_whitespace();
    let _type = it.next().ok_or_else(|| parse_err(line))?;
    let f = it
        .next()
        .map(|token| decode_flag(mode, token))
        .ok_or_else(|| parse_err(line))?;
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
    let mut cont = match add_field.split_once('/') {
        Some((_, c)) => decode_flags(mode, c),
        None => Vec::new(),
    };
    // `AffixMgr::parse_affix` sorts the continuation classes.
    cont.sort_unstable();
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
    let mode = aff.flag_mode;
    let next_flag = |it: &mut std::str::SplitWhitespace<'a>| -> Result<Flag> {
        let token = it.next().ok_or_else(|| parse_err(line))?;
        Ok(decode_flag(mode, token))
    };
    match kind {
        "SET" | "PHONE" | "WARN" | "SYLLABLENUM" | "WORDCHARS" | "MAXSUGS" | "LEMMA_PRESENT"
        | "OCONV" | "ICONV" => {}
        "LANG" => {
            let token = it.next().unwrap_or("");
            let short = token.split(['_', '-']).next().unwrap_or("");
            aff.langnum = match short.to_ascii_lowercase().as_str() {
                "ar" => 96,
                "az" => 100,
                "bg" => 41,
                "ca" => 37,
                "crh" => 102,
                "cs" => 42,
                "da" => 45,
                "de" => 49,
                "el" => 30,
                "en" => 1,
                "es" => 34,
                "eu" => 10,
                "fr" => 2,
                "gl" => 38,
                "hr" => 78,
                "hu" => 36,
                "it" => 39,
                "la" => 99,
                "lv" => 101,
                "nl" => 31,
                "pl" => 48,
                "pt" => 3,
                "ru" => 7,
                "sv" => 50,
                "tr" => 90,
                "uk" => 80,
                _ => 0,
            };
        }
        "TRY" => {
            let token = it.next().unwrap_or("");
            aff.try_string = token.as_bytes().to_vec();
        }
        "KEY" => {
            let token = it.next().unwrap_or("");
            aff.key_string = token.as_bytes().to_vec();
        }
        "REP" => {
            let count: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            for _ in 0..count {
                let Some(rline) = lines.next() else { break };
                parse_rep_line(rline.trim_end_matches('\r'), &mut aff.rep_table)?;
            }
        }
        "MAP" => {
            let count: usize = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            for _ in 0..count {
                let Some(mline) = lines.next() else { break };
                let mline = mline.trim_end_matches('\r');
                let rest = mline
                    .split_once(char::is_whitespace)
                    .map(|(_, r)| r)
                    .unwrap_or("");
                let row = parse_map_row(rest);
                if row.is_empty() {
                    return Err(parse_err(mline));
                }
                aff.map_table.push(row);
            }
        }
        "NOSPLITSUGS" => aff.nosplitsugs = true,
        "SUGSWITHDOTS" => aff.sugswithdots = true,
        "ONLYMAXDIFF" => aff.onlymaxdiff = true,
        "MAXDIFF" => aff.maxdiff = it.next().and_then(|v| v.parse().ok()).unwrap_or(-1),
        "MAXNGRAMSUGS" => aff.maxngramsugs = it.next().and_then(|v| v.parse().ok()).unwrap_or(-1),
        "MAXCPDSUGS" => aff.maxcpdsugs = it.next().and_then(|v| v.parse().ok()).unwrap_or(-1),
        "NOSUGGEST" => aff.nosuggest = Some(next_flag(it)?),
        "NONGRAMSUGGEST" => aff.nongramsuggest = Some(next_flag(it)?),
        "SUBSTANDARD" => aff.substandard = Some(next_flag(it)?),
        "FORCEUCASE" => aff.forceucase = Some(next_flag(it)?),
        "FLAG" => {
            let value = it.next().ok_or_else(|| parse_err(line))?;
            aff.flag_mode = match value {
                "long" => FlagMode::Long,
                "num" => FlagMode::Num,
                v if v.eq_ignore_ascii_case("utf-8") || v.eq_ignore_ascii_case("utf8") => {
                    FlagMode::Uni
                }
                other => {
                    return Err(CoreError::Data(format!(
                        "hunspell FLAG mode {other:?} is not supported ({line:?})"
                    )))
                }
            };
        }
        "AF"
        | "AM"
        | "COMPLEXPREFIXES"
        | "IGNORE"
        | "CHECKCOMPOUNDPATTERN"
        | "CHECKCOMPOUNDCASE"
        | "COMPOUNDMORESUFFIXES" => {
            return Err(CoreError::Data(format!(
                "hunspell directive {kind:?} is not supported by the in-tree checker ({line:?})"
            )));
        }
        // `COMPOUNDWORDMAX`: the in-tree `compound_check` does not implement
        // it (same as the other compound rules below), but the directive only
        // lowers the accepted compound length, so ignoring it cannot cause
        // false positives; the Danish `da_DK` dictionary declares
        // `COMPOUNDWORDMAX 2`. Tracked as a known fidelity gap in the
        // internal notes.
        "COMPOUNDWORDMAX" => {}
        // The remaining compound directives are parsed and ignored: the
        // in-tree `compound_check` implements only the German flag-based
        // subset, and the Swedish `sv_SE` dictionary relies on
        // `COMPOUNDRULE`/`CHECKCOMPOUND*`/`SIMPLIFIEDTRIPLE` for some of its
        // compounds. Ignoring them can only miss accepted compounds (spelling
        // false positives), never accept an unknown word; tracked as a known
        // fidelity gap in the internal notes.
        "COMPOUNDRULE"
        | "CHECKCOMPOUNDTRIPLE"
        | "SIMPLIFIEDTRIPLE"
        | "CHECKCOMPOUNDDUP"
        | "CHECKCOMPOUNDREP" => {}
        "FULLSTRIP" => aff.fullstrip = true,
        "CHECKSHARPS" => aff.checksharps = true,
        "COMPOUNDFLAG" => aff.compound_flag = Some(next_flag(it)?),
        "COMPOUNDBEGIN" => aff.compound_begin = Some(next_flag(it)?),
        "COMPOUNDMIDDLE" => aff.compound_middle = Some(next_flag(it)?),
        "COMPOUNDEND" => aff.compound_end = Some(next_flag(it)?),
        "COMPOUNDPERMITFLAG" => aff.compound_permit = Some(next_flag(it)?),
        "COMPOUNDFORBIDFLAG" => aff.compound_forbid = Some(next_flag(it)?),
        "COMPOUNDROOT" => aff.compound_root = Some(next_flag(it)?),
        "FORBIDDENWORD" => aff.forbidden_word = Some(next_flag(it)?),
        "KEEPCASE" => aff.keep_case = Some(next_flag(it)?),
        "NEEDAFFIX" | "PSEUDOROOT" => aff.need_affix = Some(next_flag(it)?),
        "ONLYINCOMPOUND" => aff.only_in_compound = Some(next_flag(it)?),
        "CIRCUMFIX" => aff.circumfix = Some(next_flag(it)?),
        "COMPOUNDMIN" => aff.compound_min = it.next().and_then(|v| v.parse().ok()).unwrap_or(1),
        "BREAK" => {
            aff.parsed_break = true;
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

/// `HashMgr::parse_reptable` one `REP <pattern> <out>` line: `_` becomes a
/// space, `^`/`$` select the 4-way position index, and only
/// `outstrings[type]` is populated.
fn parse_rep_line(line: &str, out: &mut Vec<RepEntry>) -> Result<()> {
    let mut it = line.split_whitespace();
    if it.next() != Some("REP") {
        return Err(parse_err(line));
    }
    let pattern_tok = it.next().ok_or_else(|| parse_err(line))?;
    let out_tok = it.next().ok_or_else(|| parse_err(line))?;
    let mut type_ = 0usize;
    let mut pattern = pattern_tok.as_bytes().to_vec();
    if pattern.first() == Some(&b'^') {
        type_ = 1;
        pattern.remove(0);
    }
    for b in pattern.iter_mut() {
        if *b == b'_' {
            *b = b' ';
        }
    }
    if pattern.last() == Some(&b'$') {
        type_ += 2;
        pattern.pop();
    }
    let mut outs = out_tok.as_bytes().to_vec();
    for b in outs.iter_mut() {
        if *b == b'_' {
            *b = b' ';
        }
    }
    if pattern.is_empty() || outs.is_empty() {
        return Err(parse_err(line));
    }
    let mut entry = RepEntry {
        pattern,
        ..Default::default()
    };
    entry.out[type_] = outs;
    out.push(entry);
    Ok(())
}

/// `AffixMgr::parse_maptable` data-element splitting: a `(group)` is one
/// element (possibly several characters), otherwise one UTF-8 character is
/// one element.
fn parse_map_row(rest: &str) -> Vec<Vec<u8>> {
    let b = rest.as_bytes();
    let mut row = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'(' {
            if let Some(rel) = rest[i..].find(')') {
                let end = i + rel;
                if end > i + 1 {
                    row.push(b[i + 1..end].to_vec());
                }
                i = end + 1;
                continue;
            }
        }
        let len = if b[i] & 0x80 == 0 {
            1
        } else if b[i] & 0xE0 == 0xC0 {
            2
        } else if b[i] & 0xF0 == 0xE0 {
            3
        } else {
            4
        };
        let len = len.min(b.len() - i);
        row.push(b[i..i + len].to_vec());
        i += len;
    }
    row
}

/// `HashMgr::load_tables`: split off the morphological description. Hunspell
/// looks for a `:` whose two-character type prefix is preceded by whitespace
/// (e.g. `word/flags po:noun`), backing up over the spaces; a tab always acts
/// as the old morphological separator.
fn strip_morphology(ts: &str) -> &str {
    let bytes = ts.as_bytes();
    let mut dp: Option<usize> = None;
    let mut search = 0usize;
    while let Some(rel) = ts[search..].find(':') {
        let pos = search + rel;
        if pos > 3 && (bytes[pos - 3] == b' ' || bytes[pos - 3] == b'\t') {
            let mut p = pos - 3;
            while p > 0 && (bytes[p - 1] == b' ' || bytes[p - 1] == b'\t') {
                p -= 1;
            }
            if p > 0 {
                dp = Some(p + 1);
            }
            break;
        }
        search = pos + 1;
    }
    if let Some(tab) = ts.find('\t') {
        if dp.is_none() || tab < dp.unwrap() {
            dp = Some(tab + 1);
        }
    }
    match dp {
        Some(p) => &ts[..p - 1],
        None => ts,
    }
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
        let mut flag_arena: Vec<Flag> = Vec::new();
        let mut raw: Vec<PackedEntry> = Vec::new();
        let mut seq: u32 = 0;
        let mut lines = dic_text.lines();
        let declared_count: usize = lines
            .next()
            .map(|l| {
                l.trim_start_matches('\u{feff}')
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        for line in lines {
            let line = line.trim_end_matches('\r');
            if line.starts_with('#') {
                continue;
            }
            // `HashMgr::load_tables`: split off the morphological description
            // (the `:`-with-two-char-prefix heuristic, else a tab).
            let word_part = strip_morphology(line);
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
                Some(pos) if pos > 0 => (&bytes[..pos], Some(&word_part[pos + 1..])),
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
            let mut flags: Vec<Flag> = flags_raw
                .map(|f| decode_flags(aff.flag_mode, f))
                .unwrap_or_default();
            flags.sort_unstable();
            let flags_off = flag_arena.len() as u32;
            flag_arena.extend_from_slice(&flags);
            let flags_len = flags.len() as u32;
            let mut push = |dic: &mut Vec<u8>,
                            raw: &mut Vec<PackedEntry>,
                            word: &[u8],
                            only_upcase: bool,
                            initcap: bool| {
                let word_off = dic.len() as u32;
                dic.extend_from_slice(word);
                raw.push(PackedEntry {
                    word_off,
                    word_len: word.len() as u32,
                    flags_off,
                    flags_len,
                    only_upcase,
                    initcap,
                    seq,
                });
                seq += 1;
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
                    push(&mut dic, &mut raw, &hidden, true, true);
                }
            }
            push(
                &mut dic,
                &mut raw,
                &word,
                false,
                captype == Captype::InitCap,
            );
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
        let mut by_word: std::collections::HashMap<Vec<u8>, (u32, u32)> =
            std::collections::HashMap::with_capacity(entries.len());
        let mut k = 0usize;
        while k < entries.len() {
            let first = k;
            let key = WordIndex::word(&dic, &entries[k]).to_vec();
            k += 1;
            while k < entries.len() && WordIndex::word(&dic, &entries[k]) == key.as_slice() {
                k += 1;
            }
            by_word.insert(key, (first as u32, k as u32));
        }
        // `HashMgr` hash table size (`tablesize += nExtra`, `nExtra = 5 +
        // USERWORD`, and `USERWORD` is 1000).
        let hash_tablesize = if declared_count == 0 {
            1
        } else {
            let mut t = declared_count + 1005;
            if t.is_multiple_of(2) {
                t += 1;
            }
            t
        };
        // `walk_hashtable` visits bucket 0..tablesize, and within a bucket the
        // hentries in creation order.
        let mut ngram_order: Vec<u32> = (0..entries.len() as u32).collect();
        ngram_order.sort_by_key(|&i| {
            let e = &entries[i as usize];
            (
                hunspell_hash(WordIndex::word(&dic, e), hash_tablesize) as u64,
                e.seq,
            )
        });
        Ok(Self {
            aff,
            index: WordIndex {
                flags: flag_arena,
                entries,
                by_word,
                dic,
                ngram_order,
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

    fn has_flag(&self, entry: DicEntry<'_>, flag: Option<Flag>) -> bool {
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

    /// Range of `aff.prefixes` whose `appnd` starts with `first` (the slice is
    /// sorted by `appnd`, so this is `AffixMgr`'s per-first-byte tree walk).
    fn prefix_range(&self, first: u8) -> (usize, usize) {
        let lo = self
            .aff
            .prefixes
            .partition_point(|e| e.appnd.first().copied().unwrap_or(0) < first);
        let hi = self
            .aff
            .prefixes
            .partition_point(|e| e.appnd.first().copied().unwrap_or(0) <= first);
        (lo, hi)
    }

    /// Suffix indices whose `appnd` can match the ending of `word[start..start+len]`,
    /// in the flattened-tree order (ascending index).
    fn suffix_matches(&self, word: &[u8], start: usize, len: usize) -> Vec<usize> {
        let mut out: Vec<usize> = Vec::new();
        let mut l = 1usize;
        while l <= len {
            let key = &word[start + len - l..start + len];
            if let Some(v) = self.aff.suffix_by_appnd.get(key) {
                out.extend_from_slice(v);
            }
            l += 1;
        }
        out.extend_from_slice(&self.aff.suffix_wild);
        out.sort_unstable();
        out
    }

    /// `AffixMgr::affix_check`.
    fn affix_check(
        &self,
        word: &[u8],
        start: usize,
        len: usize,
        needflag: Option<Flag>,
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
        needflag: Option<Flag>,
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
        let (lo, hi) = self.prefix_range(first);
        for idx in lo..hi {
            let e = &self.aff.prefixes[idx];
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
        needflag: Option<Flag>,
        state: &mut AffixState,
    ) -> Option<DicEntry<'_>> {
        if len < e.appnd.len() {
            return None;
        }
        let tmpl = len - e.appnd.len();
        if !(tmpl > 0 || (tmpl == 0 && self.aff.fullstrip)) {
            return None;
        }
        let mut joinbuf = [0u8; 512];
        let joinheap;
        let tmpword = joined_slice!(
            joinbuf,
            joinheap,
            &e.strip,
            word.get(start + e.appnd.len()..start + len)?
        );
        if !test_condition_prefix(&e.cond, e.numconds, tmpword) {
            return None;
        }
        let tmpl = tmpl + e.strip.len();
        let entries = self.index.entries_for(tmpword);
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
                tmpword,
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
        needflag: Option<Flag>,
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
        let (lo, hi) = self.prefix_range(first);
        for idx in lo..hi {
            let e = &self.aff.prefixes[idx];
            if !is_subset(&e.appnd, &word[start..]) {
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
        needflag: Option<Flag>,
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
        let mut joinbuf = [0u8; 512];
        let joinheap;
        let tmpword = joined_slice!(
            joinbuf,
            joinheap,
            &e.strip,
            word.get(start + e.appnd.len()..start + len)?
        );
        if !test_condition_prefix(&e.cond, e.numconds, tmpword) {
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
                tmpword,
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
        cclass: Option<Flag>,
        needflag: Option<Flag>,
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
        for idx in self.suffix_matches(word, start, len) {
            let e = &self.aff.suffixes[idx];
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
        cclass: Option<Flag>,
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
        cclass: Option<Flag>,
        needflag: Option<Flag>,
        badflag: Option<Flag>,
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
        let mut joinbuf = [0u8; 512];
        let joinheap;
        let tmpstring = joined_slice!(joinbuf, joinheap, &word[start..start + tmpl], &e.strip);
        if !test_condition_suffix(&e.cond, e.numconds, tmpstring) {
            return None;
        }
        let ep_flag = ppfx.map(|p| self.pfx_entry(p).flag);
        let ep_cont = ppfx.map(|p| self.pfx_entry(p).cont.clone());
        let entries = self.index.entries_for(tmpstring);
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
        needflag: Option<Flag>,
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
        for idx in self.suffix_matches(word, start, len) {
            let e = &self.aff.suffixes[idx];
            if !appnd_matches_word_end(&e.appnd, &word[start..start + len]) {
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
        needflag: Option<Flag>,
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
        let mut joinbuf = [0u8; 512];
        let joinheap;
        let tmpword = joined_slice!(joinbuf, joinheap, &word[start..start + tmpl], &e.strip);
        let tmpl = tmpl + e.strip.len();
        if !test_condition_suffix(&e.cond, e.numconds, tmpword) {
            return None;
        }
        if let Some(p) = ppfx {
            let conditional = e.cont.contains(&self.pfx_entry(p).flag);
            if conditional {
                self.suffix_check(
                    tmpword,
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
                    tmpword,
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
                tmpword,
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
    sfxflag: Option<Flag>,
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
                        while p < cond.len() && st >= beg {
                            if cond[p] != word[st as usize] {
                                if pos.is_none() {
                                    return false;
                                }
                                st = pos.unwrap();
                                break;
                            }
                            // First byte of the UTF-8 multibyte character:
                            // stop here (the byte-reversed condition holds the
                            // lead byte last), the advance below moves past it.
                            if cond[p] & 0xC0 != 0x80 {
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
                        if p < cond.len() && cond[p] != b']' {
                            p += 1;
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

// ---------------------------------------------------------------------------
// Suggestion engine: a faithful port of hunspell 1.7.2 `SuggestMgr`
// (`suggestmgr.cxx`) plus the `Hunspell::suggest`/`suggest_internal` driver
// (`hunspell.cxx`).
//
// Scope: the UTF-8 candidate generators and their iteration order, the
// compound-aware suggestion `checkword`, the per-capitalization driver, the
// wrapper post-processing (case restoration, `SUGSWITHDOTS`, keepcase
// filtering, dedup) and the dash suggestion.
//
// Deliberately not ported:
// - `SuggestMgr::ngsuggest` (the n-gram fallback). It does not use an
//   `.ngram` file (it walks the whole dictionary), but the mission scope
//   gates it on the languages shipping n-gram data; the vendored gl/da/sv
//   dictionaries ship none. When the generators find nothing the n-gram
//   list therefore stays empty (see the parity allowances in
//   `docs/differences.md`).
// - `suggest_ph`/`PHONE`: no target dictionary ships a `PHONE` table.
// - `suggest_gen`/`morphgen`: morphological generation, unused by `suggest`.
// - `COMPLEXPREFIXES`, `ICONV`/`OCONV`, `IGNORE`: never occur in the target
//   dictionaries (the loader rejects/handles them elsewhere).
// ---------------------------------------------------------------------------

/// `MAXSUGGESTION` (`hunspell.hxx`).
const MAXSUGGESTION: usize = 15;
/// `MAXCOMPOUNDSUGS` (`suggestmgr.hxx`).
const MAXCOMPOUNDSUGS: i32 = 3;
/// `MAX_CHAR_DISTANCE` (`suggestmgr.cxx`).
const MAX_CHAR_DISTANCE: usize = 4;
/// `SPELL_COMPOUND` (`atypes.hxx`).
const SPELL_COMPOUND: u32 = 1 << 0;

impl<'a> SuggestMgr<'a> {
    fn new(c: &'a HunspellChecker) -> Self {
        let ctry: &[u8] = &c.aff.try_string;
        let maxcpdsugs = if c.aff.maxcpdsugs >= 0 {
            c.aff.maxcpdsugs
        } else {
            MAXCOMPOUNDSUGS
        };
        // `AffixMgr::get_key_string` falls back to the QWERTY default when no
        // `KEY` directive is present.
        let ckey: &[u8] = if c.aff.key_string.is_empty() {
            b"qwertyuiop|asdfghjkl|zxcvbnm"
        } else {
            &c.aff.key_string
        };
        Self {
            c,
            ctry,
            ckey,
            max_sug: MAXSUGGESTION,
            nosplitsugs: c.aff.nosplitsugs,
            maxcpdsugs,
            lang_with_dash_usage: ctry.contains(&b'-') || ctry.contains(&b'a'),
            info: 0,
            map_budget: MAP_NODE_BUDGET,
        }
    }

    /// `SuggestMgr::testsug`.
    fn testsug(&mut self, wlst: &mut Vec<String>, candidate: Vec<u8>, cpdsuggest: i32) {
        if wlst.len() == self.max_sug {
            return;
        }
        let as_str = String::from_utf8_lossy(&candidate).into_owned();
        if wlst.iter().any(|w| w == &as_str) {
            return;
        }
        let result = self.c.sm_checkword(&candidate, cpdsuggest);
        if result != 0 {
            if cpdsuggest == 0 && result >= 2 {
                self.info |= SPELL_COMPOUND;
            }
            wlst.push(as_str);
        }
    }

    /// `SuggestMgr::suggest`: the three compound loops over the generators.
    /// Returns `(good_suggestion, only_compound_suggestion)`.
    fn suggest(&mut self, slst: &mut Vec<String>, w: &str) -> (bool, bool) {
        let word = w.as_bytes();
        let word_utf = bytes_to_chars(word);
        let mut nocompoundtwowords = false;
        let nsugorig = slst.len();
        let mut old_sug = 0usize;
        let mut good = false;

        let mut cpdsuggest = 0i32;
        while cpdsuggest < 3 && !nocompoundtwowords {
            if cpdsuggest > 0 {
                old_sug = slst.len();
            }
            let guard =
                cpdsuggest == 0 || (slst.len() as i64) < old_sug as i64 + self.maxcpdsugs as i64;

            // suggestions for an uppercase word (html -> HTML)
            if slst.len() < self.max_sug {
                let i = slst.len();
                self.capchars(slst, &word_utf, cpdsuggest);
                if slst.len() > i {
                    good = true;
                }
            }
            // perhaps we made a typical fault of spelling
            if slst.len() < self.max_sug && guard {
                let i = slst.len();
                self.replchars(slst, word, cpdsuggest);
                if slst.len() > i {
                    good = true;
                }
            }
            // perhaps we chose the wrong char from a related set
            if slst.len() < self.max_sug && guard {
                self.mapchars(slst, word, cpdsuggest);
            }
            // only suggest compound words when no other ~good suggestion
            if cpdsuggest == 0 && slst.len() > nsugorig {
                nocompoundtwowords = true;
            }
            // did we swap the order of chars by mistake
            if slst.len() < self.max_sug && guard {
                self.swapchar(slst, &word_utf, cpdsuggest);
            }
            // did we swap the order of non adjacent chars by mistake
            if slst.len() < self.max_sug && guard {
                self.longswapchar(slst, &word_utf, cpdsuggest);
            }
            // did we just hit the wrong key in place of a good char
            if slst.len() < self.max_sug && guard {
                self.badcharkey(slst, &word_utf, cpdsuggest);
            }
            // did we add a char that should not be there
            if slst.len() < self.max_sug && guard {
                self.extrachar(slst, &word_utf, cpdsuggest);
            }
            // did we forget a char
            if slst.len() < self.max_sug && guard {
                self.forgotchar(slst, &word_utf, cpdsuggest);
            }
            // did we move a char
            if slst.len() < self.max_sug && guard {
                self.movechar(slst, &word_utf, cpdsuggest);
            }
            // did we just hit the wrong key in place of a good char
            if slst.len() < self.max_sug && guard {
                self.badchar(slst, &word_utf, cpdsuggest);
            }
            // did we double two characters
            if slst.len() < self.max_sug && guard {
                self.doubletwochars(slst, &word_utf, cpdsuggest);
            }
            // two words ran together
            if cpdsuggest == 0
                || (!self.nosplitsugs
                    && (slst.len() as i64) < old_sug as i64 + self.maxcpdsugs as i64)
            {
                good = self.twowords(slst, word, cpdsuggest, good);
            }
            if cpdsuggest == 1 && (slst.len() > old_sug || (self.info & SPELL_COMPOUND) != 0) {
                nocompoundtwowords = true;
            }
            cpdsuggest += 1;
        }

        let onlycmpdsug = !nocompoundtwowords && !slst.is_empty();
        (good, onlycmpdsug)
    }

    /// `SuggestMgr::capchars_utf`: html -> HTML.
    fn capchars(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let candidate: Vec<char> = word.iter().map(|&c| simple_upper(c)).collect();
        self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
    }

    /// `SuggestMgr::replchars`.
    fn replchars(&mut self, wlst: &mut Vec<String>, word: &[u8], cpdsuggest: i32) {
        if word.len() < 2 {
            return;
        }
        let c = self.c;
        for entry in &c.aff.rep_table {
            let mut r = 0usize;
            while let Some(pos) = find_sub(word, &entry.pattern, r) {
                let mut type_ = if pos == 0 { 1usize } else { 0 };
                if pos + entry.pattern.len() == word.len() {
                    type_ += 2;
                }
                while type_ != 0 && entry.out[type_].is_empty() {
                    type_ = if type_ == 2 && pos != 0 { 0 } else { type_ - 1 };
                }
                let out = &entry.out[type_];
                if out.is_empty() {
                    r = pos + 1;
                    continue;
                }
                let mut candidate = Vec::with_capacity(word.len() + out.len());
                candidate.extend_from_slice(&word[..pos]);
                candidate.extend_from_slice(out);
                candidate.extend_from_slice(&word[pos + entry.pattern.len()..]);
                self.testsug(wlst, candidate.clone(), cpdsuggest);
                // check REP suggestions with space
                if let Some(mut sp) = find_byte(&candidate, b' ') {
                    let mut prev = 0usize;
                    loop {
                        let prev_chunk = &candidate[prev..sp];
                        if self.c.sm_checkword(prev_chunk, 0) != 0 {
                            let oldns = wlst.len();
                            let post = candidate[sp + 1..].to_vec();
                            self.testsug(wlst, post, cpdsuggest);
                            if wlst.len() > oldns {
                                let last = wlst.len() - 1;
                                wlst[last] = String::from_utf8_lossy(&candidate).into_owned();
                            }
                        }
                        prev = sp + 1;
                        match find_byte(&candidate[prev..], b' ') {
                            Some(rel) => sp = prev + rel,
                            None => break,
                        }
                    }
                }
                r = pos + 1;
            }
        }
    }

    /// `SuggestMgr::mapchars`.
    fn mapchars(&mut self, wlst: &mut Vec<String>, word: &[u8], cpdsuggest: i32) {
        if word.len() < 2 || self.c.aff.map_table.is_empty() {
            return;
        }
        let c = self.c;
        let maptable: &'a [Vec<Vec<u8>>] = &c.aff.map_table;
        let mut candidate = Vec::new();
        self.map_budget = MAP_NODE_BUDGET;
        self.map_related(maptable, word, &mut candidate, 0, wlst, cpdsuggest, 0);
    }

    #[allow(clippy::too_many_arguments)]
    fn map_related(
        &mut self,
        maptable: &'a [Vec<Vec<u8>>],
        word: &[u8],
        candidate: &mut Vec<u8>,
        wn: usize,
        wlst: &mut Vec<String>,
        cpdsuggest: i32,
        depth: usize,
    ) {
        if word.len() == wn {
            let as_str = String::from_utf8_lossy(candidate).into_owned();
            if !wlst.iter().any(|w| w == &as_str)
                && self.c.sm_checkword(candidate, cpdsuggest) != 0
                && wlst.len() < self.max_sug
            {
                wlst.push(as_str);
            }
            return;
        }
        if depth > 16384 || self.map_budget == 0 {
            return;
        }
        self.map_budget -= 1;
        let mut in_map = false;
        for row in maptable {
            for entry in row {
                let len = entry.len();
                if len > 0 && word[wn..].starts_with(entry.as_slice()) {
                    in_map = true;
                    let cn = candidate.len();
                    for alt in row {
                        candidate.truncate(cn);
                        candidate.extend_from_slice(alt);
                        self.map_related(
                            maptable,
                            word,
                            candidate,
                            wn + len,
                            wlst,
                            cpdsuggest,
                            depth + 1,
                        );
                    }
                }
            }
        }
        if !in_map {
            candidate.push(word[wn]);
            self.map_related(
                maptable,
                word,
                candidate,
                wn + 1,
                wlst,
                cpdsuggest,
                depth + 1,
            );
        }
    }

    /// `SuggestMgr::swapchar_utf`.
    fn swapchar(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        if word.len() < 2 {
            return;
        }
        let mut candidate = word.to_vec();
        let n = candidate.len();
        for i in 0..n - 1 {
            candidate.swap(i, i + 1);
            self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
            candidate.swap(i, i + 1);
        }
        if n == 4 || n == 5 {
            candidate[0] = word[1];
            candidate[1] = word[0];
            candidate[2] = word[2];
            candidate[n - 2] = word[n - 1];
            candidate[n - 1] = word[n - 2];
            self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
            if n == 5 {
                candidate[0] = word[0];
                candidate[1] = word[2];
                candidate[2] = word[1];
                self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
            }
        }
    }

    /// `SuggestMgr::longswapchar_utf`.
    fn longswapchar(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let mut candidate = word.to_vec();
        let n = candidate.len();
        for p in 0..n {
            for q in 0..n {
                let distance = p.abs_diff(q);
                if distance > 1 && distance <= MAX_CHAR_DISTANCE {
                    candidate.swap(p, q);
                    self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                    candidate.swap(p, q);
                }
            }
        }
    }

    /// `SuggestMgr::badcharkey_utf`.
    fn badcharkey(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let mut candidate = word.to_vec();
        let ckey: Vec<char> = bytes_to_chars(self.ckey);
        for i in 0..word.len() {
            let tmpc = candidate[i];
            candidate[i] = simple_upper(candidate[i]);
            if tmpc != candidate[i] {
                self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                candidate[i] = tmpc;
            }
            if ckey.is_empty() {
                continue;
            }
            let mut loc = 0usize;
            while loc < ckey.len() && ckey[loc] != tmpc {
                loc += 1;
            }
            while loc < ckey.len() {
                if loc > 0 && ckey[loc - 1] != '|' {
                    candidate[i] = ckey[loc - 1];
                    self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                }
                if (loc + 1) < ckey.len() && ckey[loc + 1] != '|' {
                    candidate[i] = ckey[loc + 1];
                    self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                }
                loop {
                    loc += 1;
                    if !(loc < ckey.len() && ckey[loc] != tmpc) {
                        break;
                    }
                }
            }
            candidate[i] = tmpc;
        }
    }

    /// `SuggestMgr::extrachar_utf` (a char that should not be there).
    fn extrachar(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let mut candidate = word.to_vec();
        if candidate.len() < 2 {
            return;
        }
        for i in 0..word.len() {
            let index = candidate.len() - 1 - i;
            let tmpc = candidate[index];
            candidate.remove(index);
            self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
            candidate.insert(index, tmpc);
        }
    }

    /// `SuggestMgr::forgotchar_utf` (a missing char).
    fn forgotchar(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let mut candidate = word.to_vec();
        let ctry: Vec<char> = bytes_to_chars(self.ctry);
        for &k in &ctry {
            let mut i = 0usize;
            while i <= candidate.len() {
                let index = candidate.len() - i;
                candidate.insert(index, k);
                self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                candidate.remove(index);
                i += 1;
            }
        }
    }

    /// `SuggestMgr::movechar_utf` (a char was moved).
    fn movechar(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        if word.len() < 2 {
            return;
        }
        let n = word.len();
        let mut candidate = word.to_vec();
        for p in 0..n {
            let mut q = p + 1;
            while q < n && q - p <= MAX_CHAR_DISTANCE {
                candidate.swap(q, q - 1);
                if q - p >= 2 {
                    self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                }
                q += 1;
            }
            candidate.copy_from_slice(word);
        }
        // reverse direction
        for rp in 0..n {
            let mut rq = rp + 1;
            while rq < n && rq - rp <= MAX_CHAR_DISTANCE {
                candidate.swap(n - rq, n - 1 - rq);
                if rq - rp >= 2 {
                    self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                }
                rq += 1;
            }
            candidate.copy_from_slice(word);
        }
    }

    /// `SuggestMgr::badchar_utf`.
    fn badchar(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let mut candidate = word.to_vec();
        let ctry: Vec<char> = bytes_to_chars(self.ctry);
        for &ch in &ctry {
            for i in (0..candidate.len()).rev() {
                let tmpc = candidate[i];
                if ch == tmpc {
                    continue;
                }
                candidate[i] = ch;
                self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                candidate[i] = tmpc;
            }
        }
    }

    /// `SuggestMgr::doubletwochars_utf` (vacation -> vacacation).
    fn doubletwochars(&mut self, wlst: &mut Vec<String>, word: &[char], cpdsuggest: i32) {
        let wl = word.len();
        if wl < 5 {
            return;
        }
        let mut state = 0;
        for i in 2..wl {
            if word[i] == word[i - 2] {
                state += 1;
                if state == 3 || (state == 2 && i >= 4) {
                    let mut candidate: Vec<char> = word[..i - 1].to_vec();
                    candidate.extend_from_slice(&word[i + 1..]);
                    self.testsug(wlst, chars_to_bytes(&candidate), cpdsuggest);
                    state = 0;
                }
            } else {
                state = 0;
            }
        }
    }

    /// `SuggestMgr::twowords`: split a run-together word.
    fn twowords(
        &mut self,
        wlst: &mut Vec<String>,
        word: &[u8],
        cpdsuggest: i32,
        mut good: bool,
    ) -> bool {
        let wl = word.len();
        if wl < 3 {
            return false;
        }
        let mut buf = vec![0u8; wl + 2];
        buf[1..1 + wl].copy_from_slice(word);
        let dash = self.lang_with_dash_usage;
        let hu = self.c.aff.langnum == 36;
        let forbidden = hu && self.check_forbidden(word);
        let mut p = 1usize;
        while buf[p + 1] != 0 {
            buf[p - 1] = buf[p];
            while buf[p + 1] & 0xc0 == 0x80 {
                buf[p] = buf[p + 1];
                p += 1;
            }
            if buf[p + 1] == 0 {
                break;
            }
            let full = |b: &[u8]| b[1..wl + 1].to_vec();
            // word pairs listed in the dictionary
            buf[p] = b' ';
            let cand = full(&buf);
            if cpdsuggest == 0 && self.c.sm_checkword(&cand, 0) != 0 {
                if !good {
                    good = true;
                    wlst.clear();
                }
                wlst.insert(0, String::from_utf8_lossy(&cand).into_owned());
            }
            if dash {
                buf[p] = b'-';
                let cand = full(&buf);
                if cpdsuggest == 0 && self.c.sm_checkword(&cand, 0) != 0 {
                    if !good {
                        good = true;
                        wlst.clear();
                    }
                    wlst.insert(0, String::from_utf8_lossy(&cand).into_owned());
                }
            }
            if wlst.len() < self.max_sug && !self.nosplitsugs && !good {
                let first = buf[1..p].to_vec();
                let c1 = self.c.sm_checkword(&first, cpdsuggest);
                if c1 != 0 {
                    let second = buf[p + 1..wl + 1].to_vec();
                    let c2 = self.c.sm_checkword(&second, cpdsuggest);
                    if c2 != 0 {
                        // spec. Hungarian code: repeat the letter or 6+ syllables -> dash
                        if hu
                            && !forbidden
                            && ((buf[p - 1] == buf[p + 1]
                                && ((p > 1 && buf[p - 1] == buf[p - 2])
                                    || buf[p - 1] == *buf.get(p + 2).unwrap_or(&0)))
                                || (c1 == 3 && c2 >= 2))
                        {
                            buf[p] = b'-';
                        } else {
                            buf[p] = b' ';
                        }
                        let cand = full(&buf);
                        if !wlst.iter().any(|w| w.as_bytes() == cand.as_slice())
                            && wlst.len() < self.max_sug
                        {
                            wlst.push(String::from_utf8_lossy(&cand).into_owned());
                        }
                        // add a two-word suggestion with dash
                        if dash && char_count(&buf[p + 1..wl + 1]) > 1 && char_count(&buf[1..p]) > 1
                        {
                            buf[p] = b'-';
                            let cand = full(&buf);
                            if !wlst.iter().any(|w| w.as_bytes() == cand.as_slice())
                                && wlst.len() < self.max_sug
                            {
                                wlst.push(String::from_utf8_lossy(&cand).into_owned());
                            }
                        }
                    }
                }
            }
            p += 1;
        }
        good
    }

    /// `SuggestMgr::check_forbidden` (Hungarian two-word handling).
    fn check_forbidden(&self, word: &[u8]) -> bool {
        let len = word.len();
        let mut rv = self.c.lookup_first(word);
        if let Some(e) = rv {
            if self.c.has_flag(e, self.c.aff.need_affix)
                || self.c.has_flag(e, self.c.aff.only_in_compound)
            {
                rv = None;
            }
        }
        let mut state = AffixState::default();
        if rv.is_none()
            && self
                .c
                .prefix_check(word, 0, len, InCompound::Not, None, &mut state)
                .is_none()
        {
            let mut state2 = AffixState::default();
            rv = self.c.suffix_check(
                word,
                0,
                len,
                0,
                None,
                None,
                None,
                InCompound::Not,
                &mut state2,
            );
        }
        rv.is_some_and(|e| self.c.has_flag(e, self.c.aff.forbidden_word))
    }
}

impl HunspellChecker {
    /// `SuggestMgr::checkword`: candidate acceptance while suggesting. Returns
    /// `0` (reject), `1`/`2` (dictionary/compound word) or `3` (compound).
    fn sm_checkword(&self, word: &[u8], cpdsuggest: i32) -> i32 {
        if word.is_empty() {
            return 0;
        }
        if cpdsuggest >= 1 {
            if self.aff.compound {
                let mut info = Info {
                    compound_2: cpdsuggest == 1,
                    ..Default::default()
                };
                let rv = self.compound_check(word, 0, 0, 100, 0, &mut info);
                if rv.is_some() {
                    let bad = self.lookup_first(word).is_some_and(|e| {
                        self.has_flag(e, self.aff.forbidden_word)
                            || self.has_flag(e, self.aff.nosuggest)
                    });
                    if !bad {
                        return 3;
                    }
                }
            }
            return 0;
        }

        let first = self.lookup_first(word);
        if let Some(e) = first {
            if self.has_flag(e, self.aff.forbidden_word)
                || self.has_flag(e, self.aff.nosuggest)
                || self.has_flag(e, self.aff.substandard)
            {
                return 0;
            }
        }
        let mut rv: Option<DicEntry<'_>> = None;
        if first.is_some() {
            for packed in self.index.entries_for(word) {
                let e = self.index.view(packed);
                if !(self.has_flag(e, self.aff.need_affix)
                    || e.only_upcase
                    || self.has_flag(e, self.aff.only_in_compound))
                {
                    rv = Some(e);
                    break;
                }
            }
        } else {
            let mut state = AffixState::default();
            rv = self.prefix_check(word, 0, word.len(), InCompound::Not, None, &mut state);
        }

        let mut nosuffix = 0;
        if rv.is_some() {
            nosuffix = 1;
        } else {
            let mut state = AffixState::default();
            rv = self.suffix_check(
                word,
                0,
                word.len(),
                0,
                None,
                None,
                None,
                InCompound::Not,
                &mut state,
            );
        }
        if rv.is_none() && self.aff.have_cont_class {
            let mut state = AffixState::default();
            rv = self.suffix_check_twosfx(word, 0, word.len(), 0, None, None, &mut state);
            if rv.is_none() {
                rv = self.prefix_check_twosfx(
                    word,
                    0,
                    word.len(),
                    InCompound::Not,
                    None,
                    &mut state,
                );
            }
        }

        if let Some(e) = rv {
            if self.has_flag(e, self.aff.forbidden_word)
                || e.only_upcase
                || self.has_flag(e, self.aff.nosuggest)
                || self.has_flag(e, self.aff.only_in_compound)
            {
                return 0;
            }
            if self.aff.compound_flag.is_some_and(|f| e.flags.contains(&f)) {
                return 2 + nosuffix;
            }
            return 1;
        }
        0
    }

    /// `Hunspell::suggest`: suggestions for `word`, in hunspell's order.
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut sm = SuggestMgr::new(self);
        let mut stack: Vec<String> = Vec::new();
        sm.suggest_rec(word, &mut stack)
    }

    /// `SfxEntry::add`: apply the suffix to `word` (used by
    /// `AffixMgr::expand_rootword`).
    fn sfx_add(&self, e: &AffixEntry, word: &[u8]) -> Option<Vec<u8>> {
        let len = word.len();
        if !((len > e.strip.len() || (len == 0 && self.aff.fullstrip))
            && len >= e.numconds
            && test_condition_suffix(&e.cond, e.numconds, word)
            && (e.strip.is_empty() || (len >= e.strip.len() && word.ends_with(e.strip.as_slice()))))
        {
            return None;
        }
        let mut result = word[..len - e.strip.len()].to_vec();
        result.extend_from_slice(&e.appnd);
        Some(result)
    }

    /// `PfxEntry::add`.
    fn pfx_add(&self, e: &AffixEntry, word: &[u8]) -> Option<Vec<u8>> {
        let len = word.len();
        if !((len > e.strip.len() || (len == 0 && self.aff.fullstrip))
            && len >= e.numconds
            && test_condition_prefix(&e.cond, e.numconds, word)
            && (e.strip.is_empty()
                || (len >= e.strip.len() && word.starts_with(e.strip.as_slice()))))
        {
            return None;
        }
        let mut result = e.appnd.clone();
        result.extend_from_slice(&word[e.strip.len()..]);
        Some(result)
    }
}

impl<'a> SuggestMgr<'a> {
    /// The `Hunspell::suggest` wrapper: recursion guard, case restoration,
    /// `SUGSWITHDOTS`, keepcase filtering and dedup.
    fn suggest_rec(&mut self, word: &str, stack: &mut Vec<String>) -> Vec<String> {
        if stack.len() > 2048 || stack.iter().any(|w| w == word) {
            return Vec::new();
        }
        stack.push(word.to_string());
        let (mut slst, capwords, abbv, captype) = self.suggest_internal(word, stack);
        stack.pop();

        if capwords {
            for j in slst.iter_mut() {
                *j = mkinitcap_str(j);
            }
        }

        // expand suggestions with dot(s)
        if abbv > 0 && self.c.aff.sugswithdots && word.len() >= abbv {
            let suffix = word[word.len() - abbv..].to_string();
            for j in slst.iter_mut() {
                j.push_str(&suffix);
            }
        }

        // remove bad capitalized and forbidden forms
        if (self.c.aff.keep_case.is_some() || self.c.aff.forbidden_word.is_some())
            && matches!(captype, Captype::InitCap | Captype::AllCap)
        {
            let mut out: Vec<String> = Vec::new();
            for j in slst {
                if !j.contains(' ') && !self.c.spell(&j) {
                    let lower = mkallsmall_str(&j);
                    if self.c.spell(&lower) {
                        out.push(lower);
                    } else {
                        let init = mkinitcap_str(&lower);
                        if self.c.spell(&init) {
                            out.push(init);
                        }
                    }
                } else {
                    out.push(j);
                }
            }
            slst = out;
        }

        // remove duplications
        let mut out: Vec<String> = Vec::new();
        for j in slst {
            if !out.iter().any(|w| w == &j) {
                out.push(j);
            }
        }
        out
    }

    /// `HunspellImpl::suggest_internal` (UTF-8 only).
    fn suggest_internal(
        &mut self,
        word: &str,
        stack: &mut Vec<String>,
    ) -> (Vec<String>, bool, usize, Captype) {
        let mut slst: Vec<String> = Vec::new();
        let (scw, captype, abbv) = cleanword2(word.as_bytes());
        if scw.is_empty() {
            return (slst, false, abbv, captype);
        }
        let mut capwords = false;

        // check capitalized form for FORCEUCASE
        if captype == Captype::NoCap && self.c.aff.forceucase.is_some() {
            let mut info = Info::default();
            if self.c.checkword(&scw, &mut info).is_some() {
                let form = mkinitcap_str(&String::from_utf8_lossy(&scw));
                slst.push(form);
                return (slst, capwords, abbv, captype);
            }
        }

        let mut good = false;
        let mut onlycmpdsug = false;
        match captype {
            Captype::NoCap => {
                let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&scw));
                good |= g;
                onlycmpdsug |= o;
                if abbv > 0 {
                    let mut wsp = scw.clone();
                    wsp.push(b'.');
                    let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&wsp));
                    good |= g;
                    onlycmpdsug |= o;
                }
            }
            Captype::InitCap => {
                capwords = true;
                let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&scw));
                good |= g;
                onlycmpdsug |= o;
                let wsp = lowercase_bytes(&scw);
                let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&wsp));
                good |= g;
                onlycmpdsug |= o;
            }
            Captype::HuhInitCap | Captype::HuhCap => {
                if captype == Captype::HuhInitCap {
                    capwords = true;
                }
                let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&scw));
                good |= g;
                onlycmpdsug |= o;
                // something.The -> something. The
                if let Some(dot_pos) = find_byte(&scw, b'.') {
                    let postdot = &scw[dot_pos + 1..];
                    if captype_utf8(postdot) == Captype::InitCap {
                        let mut s = scw.clone();
                        s.insert(dot_pos + 1, b' ');
                        insert_sug(&mut slst, s);
                    }
                }
                if captype == Captype::HuhInitCap {
                    // TheOpenOffice.org -> The OpenOffice.org
                    let wsp = mkinitsmall_str(&String::from_utf8_lossy(&scw));
                    let (g, o) = self.suggest(&mut slst, &wsp);
                    good |= g;
                    onlycmpdsug |= o;
                }
                let wsp = lowercase_bytes(&scw);
                if self.c.spell(&String::from_utf8_lossy(&wsp)) {
                    insert_sug(&mut slst, wsp.clone());
                }
                let prevns = slst.len();
                let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&wsp));
                good |= g;
                onlycmpdsug |= o;
                if captype == Captype::HuhInitCap {
                    let wspi = mkinitcap_str(&String::from_utf8_lossy(&wsp));
                    if self.c.spell(&wspi) {
                        insert_sug(&mut slst, wspi.clone().into_bytes());
                    }
                    let (g, o) = self.suggest(&mut slst, &wspi);
                    good |= g;
                    onlycmpdsug |= o;
                }
                // aNew -> "a New" (instead of "a new")
                let wl = scw.len();
                let mut j = prevns;
                while j < slst.len() {
                    if let Some(space) = find_byte(slst[j].as_bytes(), b' ') {
                        let second = slst[j][space + 1..].to_string();
                        let slen = second.len();
                        if slen < wl && scw[wl - slen..] != second.as_bytes()[..] {
                            let first = slst[j][..=space].to_string();
                            let second_init = mkinitcap_str(&second);
                            slst.remove(j);
                            slst.insert(0, first + &second_init);
                        }
                    }
                    j += 1;
                }
            }
            Captype::AllCap => {
                let wsp = lowercase_bytes(&scw);
                let (g, o) = self.suggest(&mut slst, &String::from_utf8_lossy(&wsp));
                good |= g;
                onlycmpdsug |= o;
                if self.c.aff.keep_case.is_some() && self.c.spell(&String::from_utf8_lossy(&wsp)) {
                    insert_sug(&mut slst, wsp.clone());
                }
                let wspi = mkinitcap_str(&String::from_utf8_lossy(&wsp));
                let (g, o) = self.suggest(&mut slst, &wspi);
                good |= g;
                onlycmpdsug |= o;
                for j in slst.iter_mut() {
                    *j = mkallcap_str(j);
                    if self.c.aff.checksharps {
                        *j = j.replace('ß', "SS");
                    }
                }
            }
        }

        // try n-gram approach since no good suggestion was found
        if !good && (slst.is_empty() || onlycmpdsug) && self.c.aff.maxngramsugs != 0 {
            match captype {
                Captype::NoCap => {
                    self.ngsuggest(&mut slst, &String::from_utf8_lossy(&scw), Captype::NoCap);
                }
                Captype::HuhInitCap => {
                    capwords = true;
                    let wsp = lowercase_bytes(&scw);
                    self.ngsuggest(&mut slst, &String::from_utf8_lossy(&wsp), Captype::HuhCap);
                }
                Captype::HuhCap => {
                    let wsp = lowercase_bytes(&scw);
                    self.ngsuggest(&mut slst, &String::from_utf8_lossy(&wsp), Captype::HuhCap);
                }
                Captype::InitCap => {
                    capwords = true;
                    let wsp = lowercase_bytes(&scw);
                    self.ngsuggest(&mut slst, &String::from_utf8_lossy(&wsp), Captype::InitCap);
                }
                Captype::AllCap => {
                    let wsp = lowercase_bytes(&scw);
                    let oldns = slst.len();
                    self.ngsuggest(&mut slst, &String::from_utf8_lossy(&wsp), Captype::AllCap);
                    for j in slst.iter_mut().skip(oldns) {
                        *j = mkallcap_str(j);
                    }
                }
            }
        }

        // try dash suggestion (Afo-American -> Afro-American)
        if let Some(dash_pos0) = find_byte(&scw, b'-') {
            let mut nodashsug = !slst.iter().any(|s| s.contains('-'));
            let mut prev_pos = 0usize;
            let mut last = false;
            let mut dash_pos = dash_pos0;
            while !good && nodashsug && !last {
                if dash_pos == scw.len() {
                    last = true;
                }
                let chunk = &scw[prev_pos..dash_pos];
                if chunk != word.as_bytes() && !self.c.spell(&String::from_utf8_lossy(chunk)) {
                    let nlst = self.suggest_rec(&String::from_utf8_lossy(chunk), stack);
                    for j in nlst.iter().rev() {
                        let mut wspace = scw[..prev_pos].to_vec();
                        wspace.extend_from_slice(j.as_bytes());
                        if !last {
                            wspace.push(b'-');
                            wspace.extend_from_slice(&scw[dash_pos + 1..]);
                        }
                        let mut info = Info::default();
                        if self.c.aff.forbidden_word.is_some() {
                            self.c.checkword(&wspace, &mut info);
                        }
                        if !info.forbidden {
                            insert_sug(&mut slst, wspace);
                        }
                    }
                    nodashsug = false;
                }
                if !last {
                    prev_pos = dash_pos + 1;
                    dash_pos = find_byte(&scw[prev_pos..], b'-')
                        .map(|r| prev_pos + r)
                        .unwrap_or(scw.len());
                }
            }
        }

        (slst, capwords, abbv, captype)
    }

    /// `AffixMgr::expand_rootword`: the root word plus its suffixed,
    /// prefix+suffix cross and pure-prefix forms (capped at `MAX_WORDS`).
    fn expand_rootword(&self, root_word: &[u8], flags: &[Flag], bad: &[u8]) -> Vec<GuessWord> {
        let needaffix = self.c.aff.need_affix;
        let onlyincompound = self.c.aff.only_in_compound;
        let circumfix = self.c.aff.circumfix;
        let cont_special = |e: &AffixEntry| {
            e.cont.iter().any(|c| {
                Some(*c) == needaffix || Some(*c) == circumfix || Some(*c) == onlyincompound
            })
        };
        let mut wlst: Vec<GuessWord> = Vec::new();
        if !flags
            .iter()
            .any(|f| Some(*f) == needaffix || Some(*f) == onlyincompound)
        {
            wlst.push(GuessWord {
                word: root_word.to_vec(),
                allow: false,
            });
        }
        // suffixes
        for &f in flags {
            let Some(chain) = self.c.aff.sfx_by_flag.get(&f) else {
                continue;
            };
            for &idx in chain {
                let e = &self.c.aff.suffixes_file[idx];
                if !(e.appnd.is_empty()
                    || (bad.len() > e.appnd.len() && bad.ends_with(e.appnd.as_slice())))
                {
                    continue;
                }
                if cont_special(e) {
                    continue;
                }
                if let Some(newword) = self.c.sfx_add(e, root_word) {
                    if wlst.len() < MAX_WORDS {
                        wlst.push(GuessWord {
                            word: newword,
                            allow: e.cross,
                        });
                    }
                }
            }
        }
        let n = wlst.len();
        // cross products of prefixes and suffixes
        for j in 1..n {
            if !wlst[j].allow {
                continue;
            }
            for &f in flags {
                let Some(chain) = self.c.aff.pfx_by_flag.get(&f) else {
                    continue;
                };
                for &idx in chain {
                    let e = &self.c.aff.prefixes_file[idx];
                    if !e.cross
                        || !(e.appnd.is_empty()
                            || (bad.len() > e.appnd.len() && bad.starts_with(e.appnd.as_slice())))
                    {
                        continue;
                    }
                    if let Some(newword) = self.c.pfx_add(e, &wlst[j].word) {
                        if wlst.len() < MAX_WORDS {
                            wlst.push(GuessWord {
                                word: newword,
                                allow: e.cross,
                            });
                        }
                    }
                }
            }
        }
        // pure prefixes
        for &f in flags {
            let Some(chain) = self.c.aff.pfx_by_flag.get(&f) else {
                continue;
            };
            for &idx in chain {
                let e = &self.c.aff.prefixes_file[idx];
                if !(e.appnd.is_empty()
                    || (bad.len() > e.appnd.len() && bad.starts_with(e.appnd.as_slice())))
                {
                    continue;
                }
                if cont_special(e) {
                    continue;
                }
                if let Some(newword) = self.c.pfx_add(e, root_word) {
                    if wlst.len() < MAX_WORDS {
                        wlst.push(GuessWord {
                            word: newword,
                            allow: e.cross,
                        });
                    }
                }
            }
        }
        wlst
    }

    /// `SuggestMgr::ngsuggest`: the n-gram fallback, run when the generators
    /// found nothing good. The `PHONE` and non-BMP branches are not ported
    /// (no target dictionary ships a `PHONE` table; the words are BMP).
    fn ngsuggest(&self, wlst: &mut Vec<String>, w: &str, captype: Captype) {
        let word_chars: Vec<char> = w.chars().collect();
        let n = word_chars.len() as i32;
        let low = true;
        let langnum = self.c.aff.langnum;
        let complexprefixes = false;

        // exhaustively walk the dictionary, keeping the MAX_ROOTS best roots
        let mut roots: Vec<Option<u32>> = vec![None; MAX_ROOTS];
        let mut scores: Vec<i32> = (0..MAX_ROOTS).map(|i| -100 * i as i32).collect();
        let mut lp = MAX_ROOTS - 1;
        for &idx in &self.c.index.ngram_order {
            let packed = self.c.index.entries[idx as usize];
            let hp_word = self.c.index.entry_word(&packed);
            let clen = bytes_to_chars(hp_word).len() as i32;
            if (n - clen).abs() > 4 {
                continue;
            }
            // don't suggest capitalized dictionary words for lower-case
            // misspellings (no PHONE, not German)
            if captype == Captype::NoCap && packed.initcap && langnum != 49 {
                continue;
            }
            let e = self.c.index.view(&packed);
            if self.c.has_flag(e, self.c.aff.forbidden_word)
                || self.c.has_flag(e, self.c.aff.nosuggest)
                || self.c.has_flag(e, self.c.aff.nongramsuggest)
                || self.c.has_flag(e, self.c.aff.only_in_compound)
            {
                continue;
            }
            let mut f_chars = bytes_to_chars(hp_word);
            let leftcommon = leftcommonsubstring(&word_chars, &f_chars, complexprefixes);
            if low {
                for ch in f_chars.iter_mut() {
                    *ch = simple_lower(*ch);
                }
            }
            let sc = ngram_score(3, &word_chars, &f_chars, NGRAM_LONGER_WORSE) + leftcommon;
            if sc > scores[lp] {
                scores[lp] = sc;
                roots[lp] = Some(idx);
                let mut lval = sc;
                for (j, &sj) in scores.iter().enumerate() {
                    if sj < lval {
                        lp = j;
                        lval = sj;
                    }
                }
            }
        }

        // minimum acceptable score: mangle the word three ways
        let mut thresh = 0i32;
        for sp in 1..4usize {
            let mut mw = word_chars.clone();
            let mut k = sp;
            while k < word_chars.len() {
                mw[k] = '*';
                k += 4;
            }
            if low {
                for ch in mw.iter_mut() {
                    *ch = simple_lower(*ch);
                }
            }
            thresh += ngram_score(n as usize, &word_chars, &mw, NGRAM_ANY_MISMATCH);
        }
        thresh = thresh / 3 - 1;

        // expand each root and keep the MAX_GUESS best candidates
        let mut guesses: Vec<Option<Vec<u8>>> = vec![None; MAX_GUESS];
        let mut gscore: Vec<i32> = (0..MAX_GUESS).map(|i| -100 * i as i32).collect();
        let mut lp = MAX_GUESS - 1;
        for root in &roots {
            let Some(ridx) = *root else { continue };
            let packed = self.c.index.entries[ridx as usize];
            let root_word = self.c.index.entry_word(&packed).to_vec();
            let flags = self.c.index.view(&packed).flags.to_vec();
            for g in self.expand_rootword(&root_word, &flags, w.as_bytes()) {
                let mut f_chars = bytes_to_chars(&g.word);
                let leftcommon = leftcommonsubstring(&word_chars, &f_chars, complexprefixes);
                if low {
                    for ch in f_chars.iter_mut() {
                        *ch = simple_lower(*ch);
                    }
                }
                let sc =
                    ngram_score(n as usize, &word_chars, &f_chars, NGRAM_ANY_MISMATCH) + leftcommon;
                if sc > thresh && sc > gscore[lp] {
                    guesses[lp] = Some(g.word);
                    gscore[lp] = sc;
                    let mut lval = sc;
                    for (j, &sj) in gscore.iter().enumerate() {
                        if sj < lval {
                            lp = j;
                            lval = sj;
                        }
                    }
                }
            }
        }

        bubble_desc(&mut guesses, &mut gscore);

        // weight with a similarity index based on LCS, then resort
        let mut fact = 1.0f64;
        if self.c.aff.maxdiff >= 0 {
            fact = (10.0 - self.c.aff.maxdiff as f64) / 5.0;
        }
        for i in 0..MAX_GUESS {
            let Some(g) = guesses[i].clone() else {
                continue;
            };
            let gl = mkallsmall_str(&String::from_utf8_lossy(&g));
            let gl_chars = bytes_to_chars(gl.as_bytes());
            let len = gl_chars.len() as i32;
            let lcs = lcslen(&word_chars, &gl_chars);
            if n == len && n == lcs {
                gscore[i] += 2000;
                break;
            }
            let mut re = ngram_score(
                2,
                &word_chars,
                &gl_chars,
                NGRAM_ANY_MISMATCH | NGRAM_WEIGHTED,
            );
            if low {
                let mut f = word_chars.clone();
                for ch in f.iter_mut() {
                    *ch = simple_lower(*ch);
                }
                re += ngram_score(2, &gl_chars, &f, NGRAM_ANY_MISMATCH | NGRAM_WEIGHTED);
            } else {
                re += ngram_score(
                    2,
                    &gl_chars,
                    &word_chars,
                    NGRAM_ANY_MISMATCH | NGRAM_WEIGHTED,
                );
            }
            let ng4 = ngram_score(4, &word_chars, &gl_chars, NGRAM_ANY_MISMATCH);
            let lc = leftcommonsubstring(&word_chars, &gl_chars, complexprefixes);
            let (ccp, is_swap) = commoncharacterpositions(&word_chars, &gl_chars);
            gscore[i] = 2 * lcs - (n - len).abs()
                + lc
                + if ccp > 0 { 1 } else { 0 }
                + if is_swap { 10 } else { 0 }
                + ng4
                + re
                + if (re as f64) < (n + len) as f64 * fact {
                    -1000
                } else {
                    0
                };
        }

        bubble_desc(&mut guesses, &mut gscore);

        // copy over
        let oldns = wlst.len();
        let maxngramsugs = if self.c.aff.maxngramsugs >= 0 {
            self.c.aff.maxngramsugs
        } else {
            MAXNGRAMSUGS
        };
        let onlymaxdiff = self.c.aff.onlymaxdiff;
        let mut same = false;
        for i in 0..MAX_GUESS {
            let Some(g) = guesses[i].clone() else {
                continue;
            };
            if wlst.len() < oldns + maxngramsugs as usize
                && wlst.len() < self.max_sug
                && (!same || gscore[i] > 1000)
            {
                let mut unique = true;
                if gscore[i] > 1000 {
                    same = true;
                } else if gscore[i] < -100 {
                    same = true;
                    // keep the best n-gram suggestions, unless in ONLYMAXDIFF mode
                    if wlst.len() > oldns || onlymaxdiff {
                        continue;
                    }
                }
                let gs = String::from_utf8_lossy(&g);
                for j in wlst.iter() {
                    // don't suggest previous suggestions or a previous
                    // suggestion with prefixes or affixes; check forbidden words
                    if gs.contains(j.as_str()) || self.c.sm_checkword(&g, 0) == 0 {
                        unique = false;
                        break;
                    }
                }
                if unique {
                    wlst.push(gs.into_owned());
                }
            }
        }
    }
}

/// `SuggestMgr::bubblesort`: descending insertion sort (stable for ties).
fn bubble_desc<T>(items: &mut [Option<T>], scores: &mut [i32]) {
    let n = scores.len();
    let mut m = 1;
    while m < n {
        let mut j = m;
        while j > 0 {
            if scores[j - 1] < scores[j] {
                scores.swap(j - 1, j);
                items.swap(j - 1, j);
                j -= 1;
            } else {
                break;
            }
        }
        m += 1;
    }
}

/// UTF-8 character count (`SuggestMgr::mystrlen` in UTF-8 mode).
fn char_count(b: &[u8]) -> usize {
    bytes_to_chars(b).len()
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

    /// Small hand-checked dictionary used by the suggestion tests. The
    /// expected lists were verified against hunspell 1.7.2's `suggest()`
    /// (the same version the Java engine binds); the only hunspell additions
    /// not reproduced here are the n-gram fallback ones (`cat` itself for the
    /// correctly-spelled `cat`), which is deliberately unported.
    fn suggest_test_checker() -> HunspellChecker {
        let aff = "SET UTF-8\nTRY abcdefghijklmnopqrstuvwxyz\nREP 1\nREP ph f\nNOSPLITSUGS\n";
        let dic = "7\ncat\ncar\ncart\ncut\nfish\nphone\nplanet\n";
        HunspellChecker::from_strs(aff, dic).unwrap()
    }

    #[test]
    fn suggest_generators_and_order() {
        let c = suggest_test_checker();
        // badchar (t -> r) then forgotchar (insert r): hunspell's order; the
        // trailing `cat` is the n-gram fallback (the word itself).
        assert_eq!(c.suggest("cat"), vec!["car", "cart", "cut", "cat"]);
        // forgotchar inserts at the end first (cat -> cut, then cat -> cat? no).
        assert_eq!(c.suggest("ct"), vec!["cat", "cut"]);
        // adjacent-swap generator.
        assert_eq!(c.suggest("cta"), vec!["cat"]);
        // REP "ph" -> "f".
        assert_eq!(c.suggest("phish"), vec!["fish"]);
        assert!(c.suggest("hellp").is_empty());
    }

    #[test]
    fn suggest_ngram_fallback() {
        let c = suggest_test_checker();
        // `plnt` -> `planet` is two edits away, so only the n-gram generator
        // can find it.
        assert_eq!(c.suggest("plnt"), vec!["planet"]);
        assert_eq!(c.suggest("ctt"), vec!["cat", "cut"]);
        assert_eq!(c.suggest("fsh"), vec!["fish"]);
    }

    #[test]
    fn suggest_capitalization_wrapper() {
        let c = suggest_test_checker();
        // INITCAP: suggestions from the lowercase pass are re-capitalized.
        assert_eq!(c.suggest("Cat"), vec!["Cat", "Car", "Cart", "Cut"]);
        // ALLCAP: keepcase/checksharps filtering, all caps restored.
        assert_eq!(c.suggest("PLANET"), vec!["PLANET"]);
    }

    #[test]
    fn suggest_sugswithdots() {
        let aff = "SET UTF-8\nTRY abcdefghijklmnopqrstuvwxyz\nNOSPLITSUGS\nSUGSWITHDOTS\n";
        let dic = "3\ncat\ncar\ncart\n";
        let c = HunspellChecker::from_strs(aff, dic).unwrap();
        // the abbreviation's trailing dot is re-appended to every candidate.
        assert_eq!(c.suggest("cta."), vec!["cat."]);
    }
}
