//! Dictionary charset support for the Morfologik `.info`
//! `fsa.dict.encoding` property.
//!
//! Most LT dictionaries are UTF-8; the Italian tagger/speller dictionaries
//! are ISO-8859-15 (Latin-9, D-100) and the Slovenian speller dictionary is
//! ISO-8859-2 (Latin-2). Morfologik's `DictionaryLookup` encodes the query
//! word with the metadata charset and decodes hits the same way, so the
//! tagger/speller must not assume UTF-8 bytes.

use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    #[default]
    Utf8,
    /// ISO-8859-15 (Latin-9): Latin-1 with 8 replaced code points.
    Iso885915,
    /// ISO-8859-1 (Latin-1).
    Iso88591,
    /// ISO-8859-2 (Latin-2, Central European).
    Iso88592,
    /// ISO-8859-7 (Greek).
    Iso88597,
}

/// The 8 code points where ISO-8859-15 differs from ISO-8859-1.
const LATIN9_OVERRIDES: [(u8, char); 8] = [
    (0xA4, '\u{20AC}'), // €
    (0xA6, '\u{0160}'), // Š
    (0xA8, '\u{0161}'), // š
    (0xB4, '\u{017D}'), // Ž
    (0xB8, '\u{017E}'), // ž
    (0xBC, '\u{0152}'), // Œ
    (0xBD, '\u{0153}'), // œ
    (0xBE, '\u{0178}'), // Ÿ
];

/// The high half (0xA0–0xFF) of ISO-8859-2 (Latin-2). Bytes below 0xA0 are
/// the identity mapping, like Latin-1.
const LATIN2_HIGH: [(u8, char); 96] = [
    (0xA0, '\u{00A0}'),
    (0xA1, '\u{0104}'),
    (0xA2, '\u{02D8}'),
    (0xA3, '\u{0141}'),
    (0xA4, '\u{00A4}'),
    (0xA5, '\u{013D}'),
    (0xA6, '\u{015A}'),
    (0xA7, '\u{00A7}'),
    (0xA8, '\u{00A8}'),
    (0xA9, '\u{0160}'),
    (0xAA, '\u{015E}'),
    (0xAB, '\u{0164}'),
    (0xAC, '\u{0179}'),
    (0xAD, '\u{00AD}'),
    (0xAE, '\u{017D}'),
    (0xAF, '\u{017B}'),
    (0xB0, '\u{00B0}'),
    (0xB1, '\u{0105}'),
    (0xB2, '\u{02DB}'),
    (0xB3, '\u{0142}'),
    (0xB4, '\u{00B4}'),
    (0xB5, '\u{013E}'),
    (0xB6, '\u{015B}'),
    (0xB7, '\u{02C7}'),
    (0xB8, '\u{00B8}'),
    (0xB9, '\u{0161}'),
    (0xBA, '\u{015F}'),
    (0xBB, '\u{0165}'),
    (0xBC, '\u{017A}'),
    (0xBD, '\u{02DD}'),
    (0xBE, '\u{017E}'),
    (0xBF, '\u{017C}'),
    (0xC0, '\u{0154}'),
    (0xC1, '\u{00C1}'),
    (0xC2, '\u{00C2}'),
    (0xC3, '\u{0102}'),
    (0xC4, '\u{00C4}'),
    (0xC5, '\u{0139}'),
    (0xC6, '\u{0106}'),
    (0xC7, '\u{00C7}'),
    (0xC8, '\u{010C}'),
    (0xC9, '\u{00C9}'),
    (0xCA, '\u{0118}'),
    (0xCB, '\u{00CB}'),
    (0xCC, '\u{011A}'),
    (0xCD, '\u{00CD}'),
    (0xCE, '\u{00CE}'),
    (0xCF, '\u{010E}'),
    (0xD0, '\u{0110}'),
    (0xD1, '\u{0143}'),
    (0xD2, '\u{0147}'),
    (0xD3, '\u{00D3}'),
    (0xD4, '\u{00D4}'),
    (0xD5, '\u{0150}'),
    (0xD6, '\u{00D6}'),
    (0xD7, '\u{00D7}'),
    (0xD8, '\u{0158}'),
    (0xD9, '\u{016E}'),
    (0xDA, '\u{00DA}'),
    (0xDB, '\u{0170}'),
    (0xDC, '\u{00DC}'),
    (0xDD, '\u{00DD}'),
    (0xDE, '\u{0162}'),
    (0xDF, '\u{00DF}'),
    (0xE0, '\u{0155}'),
    (0xE1, '\u{00E1}'),
    (0xE2, '\u{00E2}'),
    (0xE3, '\u{0103}'),
    (0xE4, '\u{00E4}'),
    (0xE5, '\u{013A}'),
    (0xE6, '\u{0107}'),
    (0xE7, '\u{00E7}'),
    (0xE8, '\u{010D}'),
    (0xE9, '\u{00E9}'),
    (0xEA, '\u{0119}'),
    (0xEB, '\u{00EB}'),
    (0xEC, '\u{011B}'),
    (0xED, '\u{00ED}'),
    (0xEE, '\u{00EE}'),
    (0xEF, '\u{010F}'),
    (0xF0, '\u{0111}'),
    (0xF1, '\u{0144}'),
    (0xF2, '\u{0148}'),
    (0xF3, '\u{00F3}'),
    (0xF4, '\u{00F4}'),
    (0xF5, '\u{0151}'),
    (0xF6, '\u{00F6}'),
    (0xF7, '\u{00F7}'),
    (0xF8, '\u{0159}'),
    (0xF9, '\u{016F}'),
    (0xFA, '\u{00FA}'),
    (0xFB, '\u{0171}'),
    (0xFC, '\u{00FC}'),
    (0xFD, '\u{00FD}'),
    (0xFE, '\u{0163}'),
    (0xFF, '\u{02D9}'),
];

/// The high half (0xA0-0xFF) of ISO-8859-7 (Greek); bytes below 0xA0 are
/// the identity mapping. Three code points (0xAE, 0xD2, 0xFF) are undefined
/// and decode to U+FFFD, like Java's `ISO-8859-7`.
const GREEK_88597_HIGH: [(u8, char); 96] = [
    (0xA0, '\u{00A0}'),
    (0xA1, '\u{2018}'),
    (0xA2, '\u{2019}'),
    (0xA3, '\u{00A3}'),
    (0xA4, '\u{20AC}'),
    (0xA5, '\u{20AF}'),
    (0xA6, '\u{00A6}'),
    (0xA7, '\u{00A7}'),
    (0xA8, '\u{00A8}'),
    (0xA9, '\u{00A9}'),
    (0xAA, '\u{037A}'),
    (0xAB, '\u{00AB}'),
    (0xAC, '\u{00AC}'),
    (0xAD, '\u{00AD}'),
    (0xAE, '\u{FFFD}'),
    (0xAF, '\u{2015}'),
    (0xB0, '\u{00B0}'),
    (0xB1, '\u{00B1}'),
    (0xB2, '\u{00B2}'),
    (0xB3, '\u{00B3}'),
    (0xB4, '\u{0384}'),
    (0xB5, '\u{0385}'),
    (0xB6, '\u{0386}'),
    (0xB7, '\u{00B7}'),
    (0xB8, '\u{0388}'),
    (0xB9, '\u{0389}'),
    (0xBA, '\u{038A}'),
    (0xBB, '\u{00BB}'),
    (0xBC, '\u{038C}'),
    (0xBD, '\u{00BD}'),
    (0xBE, '\u{038E}'),
    (0xBF, '\u{038F}'),
    (0xC0, '\u{0390}'),
    (0xC1, '\u{0391}'),
    (0xC2, '\u{0392}'),
    (0xC3, '\u{0393}'),
    (0xC4, '\u{0394}'),
    (0xC5, '\u{0395}'),
    (0xC6, '\u{0396}'),
    (0xC7, '\u{0397}'),
    (0xC8, '\u{0398}'),
    (0xC9, '\u{0399}'),
    (0xCA, '\u{039A}'),
    (0xCB, '\u{039B}'),
    (0xCC, '\u{039C}'),
    (0xCD, '\u{039D}'),
    (0xCE, '\u{039E}'),
    (0xCF, '\u{039F}'),
    (0xD0, '\u{03A0}'),
    (0xD1, '\u{03A1}'),
    (0xD2, '\u{FFFD}'),
    (0xD3, '\u{03A3}'),
    (0xD4, '\u{03A4}'),
    (0xD5, '\u{03A5}'),
    (0xD6, '\u{03A6}'),
    (0xD7, '\u{03A7}'),
    (0xD8, '\u{03A8}'),
    (0xD9, '\u{03A9}'),
    (0xDA, '\u{03AA}'),
    (0xDB, '\u{03AB}'),
    (0xDC, '\u{03AC}'),
    (0xDD, '\u{03AD}'),
    (0xDE, '\u{03AE}'),
    (0xDF, '\u{03AF}'),
    (0xE0, '\u{03B0}'),
    (0xE1, '\u{03B1}'),
    (0xE2, '\u{03B2}'),
    (0xE3, '\u{03B3}'),
    (0xE4, '\u{03B4}'),
    (0xE5, '\u{03B5}'),
    (0xE6, '\u{03B6}'),
    (0xE7, '\u{03B7}'),
    (0xE8, '\u{03B8}'),
    (0xE9, '\u{03B9}'),
    (0xEA, '\u{03BA}'),
    (0xEB, '\u{03BB}'),
    (0xEC, '\u{03BC}'),
    (0xED, '\u{03BD}'),
    (0xEE, '\u{03BE}'),
    (0xEF, '\u{03BF}'),
    (0xF0, '\u{03C0}'),
    (0xF1, '\u{03C1}'),
    (0xF2, '\u{03C2}'),
    (0xF3, '\u{03C3}'),
    (0xF4, '\u{03C4}'),
    (0xF5, '\u{03C5}'),
    (0xF6, '\u{03C6}'),
    (0xF7, '\u{03C7}'),
    (0xF8, '\u{03C8}'),
    (0xF9, '\u{03C9}'),
    (0xFA, '\u{03CA}'),
    (0xFB, '\u{03CB}'),
    (0xFC, '\u{03CC}'),
    (0xFD, '\u{03CD}'),
    (0xFE, '\u{03CE}'),
    (0xFF, '\u{FFFD}'),
];

impl Charset {
    pub fn from_info_name(name: &str) -> Option<Self> {
        let normalized = name.trim().to_ascii_lowercase().replace('_', "-");
        match normalized.as_str() {
            "utf-8" | "utf8" => Some(Charset::Utf8),
            "iso-8859-15" | "iso8859-15" | "latin9" | "l9" => Some(Charset::Iso885915),
            "iso-8859-1" | "iso8859-1" | "latin1" | "l1" => Some(Charset::Iso88591),
            "iso-8859-2" | "iso8859-2" | "latin2" | "l2" => Some(Charset::Iso88592),
            "iso-8859-7" | "iso8859-7" | "greek" | "greek8" => Some(Charset::Iso88597),
            _ => None,
        }
    }

    /// The high-range override table of a single-byte charset (`0x00–0x9F`
    /// are the Latin-1 identity mapping).
    fn overrides(self) -> &'static [(u8, char)] {
        match self {
            Charset::Iso885915 => &LATIN9_OVERRIDES[..],
            Charset::Iso88592 => &LATIN2_HIGH[..],
            Charset::Iso88597 => &GREEK_88597_HIGH[..],
            Charset::Utf8 | Charset::Iso88591 => &[][..],
        }
    }

    /// Decode FSA bytes into a string. Invalid UTF-8 falls back to a lossy
    /// decode like the previous UTF-8-only code.
    pub fn decode(self, bytes: &[u8]) -> Cow<'_, str> {
        match self {
            Charset::Utf8 => String::from_utf8_lossy(bytes),
            Charset::Iso885915 | Charset::Iso88591 | Charset::Iso88592 | Charset::Iso88597 => {
                let overrides = self.overrides();
                Cow::Owned(
                    bytes
                        .iter()
                        .map(|&b| match overrides.iter().find(|(byte, _)| *byte == b) {
                            Some((_, c)) => *c,
                            None => b as char,
                        })
                        .collect(),
                )
            }
        }
    }

    /// Decode the first character of a (possibly partial) byte sequence.
    /// UTF-8 returns `None` for an incomplete sequence so the caller can keep
    /// accumulating continuation bytes; single-byte charsets always decode the
    /// last byte.
    pub fn decode_one(self, bytes: &[u8]) -> Option<char> {
        match self {
            Charset::Utf8 => std::str::from_utf8(bytes)
                .ok()
                .and_then(|s| s.chars().next()),
            _ => bytes
                .last()
                .map(|&b| self.decode(&[b]).chars().next().unwrap_or('?')),
        }
    }

    /// Encode a string into FSA bytes. `None` when a character cannot be
    /// mapped to the charset: morfologik's `DictionaryLookup` treats that as
    /// "the dictionary cannot contain this word" and returns no hits, so the
    /// caller must do the same.
    pub fn encode(self, text: &str) -> Option<Vec<u8>> {
        match self {
            Charset::Utf8 => Some(text.as_bytes().to_vec()),
            Charset::Iso885915 | Charset::Iso88591 | Charset::Iso88592 | Charset::Iso88597 => {
                let overrides = self.overrides();
                let mut out = Vec::with_capacity(text.len());
                for c in text.chars() {
                    if (c as u32) < 0x80 || (0xA0..=0xFF).contains(&(c as u32)) {
                        // In the high range the identity mapping only holds
                        // for the bytes not in the override table; for
                        // ISO-8859-2 every high byte is overridden, so check
                        // the reverse table first.
                        if let Some((byte, _)) = overrides.iter().find(|(_, ch)| *ch == c) {
                            out.push(*byte);
                        } else {
                            out.push(c as u8);
                        }
                    } else if let Some((byte, _)) = overrides.iter().find(|(_, ch)| *ch == c) {
                        out.push(*byte);
                    } else {
                        return None;
                    }
                }
                Some(out)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin2_round_trips_slovenian() {
        let cs = Charset::Iso88592;
        let text = "čšžČŠŽ";
        let bytes = cs.encode(text).unwrap();
        assert_eq!(bytes, vec![0xE8, 0xB9, 0xBE, 0xC8, 0xA9, 0xAE]);
        assert_eq!(cs.decode(&bytes), text);
    }

    #[test]
    fn latin2_rejects_unmappable() {
        assert!(Charset::Iso88592.encode("€").is_none());
    }

    #[test]
    fn greek_88597_round_trips_greek() {
        let cs = Charset::Iso88597;
        let text = "αβγδεζηθικλμνξοπρστυφχψωΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ";
        let bytes = cs.encode(text).unwrap();
        assert_eq!(bytes[0], 0xE1); // α
        assert_eq!(*bytes.last().unwrap(), 0xD9); // Ω
        assert_eq!(cs.decode(&bytes), text);
    }

    #[test]
    fn greek_88597_rejects_unmappable() {
        // Cyrillic is not in ISO-8859-7
        assert!(Charset::Iso88597.encode("ж").is_none());
    }
}
