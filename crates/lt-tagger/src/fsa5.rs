//! Morfologik FSA version 5 reader (`morfologik.fsa.FSA5`).
//!
//! The older LT dictionaries (`it/italian.dict` among them) were compiled
//! with the version-5 format (byte-packed addresses, `FLEXIBLE | STOPBIT |
//! NEXTBIT`, `ADDRESS_OFFSET == 1`); the current toolchain emits CFSA2. The
//! automaton is exposed with the same primitives as `Cfsa2` so the
//! `Dictionary`/`SynthDictionary` lookups work on both formats.
//!
//! See internal development notes D-100.

use lt_core::Result;

pub const VERSION_FSA5: u8 = 5;

const BIT_FINAL_ARC: u8 = 1 << 0;
const BIT_LAST_ARC: u8 = 1 << 1;
const BIT_TARGET_NEXT: u8 = 1 << 2;
/// `FSA5.ADDRESS_OFFSET`: the flags/address field starts after the label.
const ADDRESS_OFFSET: usize = 1;

#[derive(Debug, Clone)]
pub struct Fsa5 {
    arcs: Vec<u8>,
    /// `nodeDataLength` (NUMBERS header), 0 for LT dictionaries.
    node_data_length: usize,
    /// Bytes per full-form address (`gotoLength`).
    gtl: usize,
    root: usize,
}

impl Fsa5 {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                "file too short".into(),
            ));
        }
        let magic = &bytes[..4];
        if magic != b"\\fsa" {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("bad magic {magic:02x?}, expected \\\\fsa (FSA5)"),
            ));
        }
        if bytes[4] != VERSION_FSA5 {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("unsupported FSA version {:#04x} (FSA5 parser)", bytes[4]),
            ));
        }
        // header: filler, annotation, hgtl
        let hgtl = bytes[7];
        let node_data_length = ((hgtl >> 4) & 0x0f) as usize;
        let gtl = (hgtl & 0x0f) as usize;
        let arcs = bytes[8..].to_vec();
        let mut fsa = Self {
            arcs,
            node_data_length,
            gtl,
            root: 0,
        };
        // `FSA5.getRootNode`: skip the dummy terminating node's arc and follow
        // the epsilon node's only arc.
        let epsilon = fsa.skip_arc(fsa.first_arc(0));
        fsa.root = fsa.destination_node_offset(fsa.first_arc(epsilon));
        Ok(fsa)
    }

    pub fn root_node(&self) -> usize {
        self.root
    }

    pub fn first_arc(&self, node: usize) -> usize {
        self.node_data_length + node
    }

    pub fn next_arc(&self, arc: usize) -> usize {
        if self.is_arc_last(arc) {
            0
        } else {
            self.skip_arc(arc)
        }
    }

    pub fn arc_label(&self, arc: usize) -> u8 {
        self.arcs[arc]
    }

    pub fn is_arc_final(&self, arc: usize) -> bool {
        self.arcs[arc + ADDRESS_OFFSET] & BIT_FINAL_ARC != 0
    }

    pub fn is_arc_terminal(&self, arc: usize) -> bool {
        self.destination_node_offset(arc) == 0
    }

    pub fn end_node(&self, arc: usize) -> usize {
        self.destination_node_offset(arc)
    }

    pub fn arc_by_label(&self, node: usize, label: u8) -> usize {
        let mut arc = self.first_arc(node);
        while arc != 0 {
            if self.arc_label(arc) == label {
                return arc;
            }
            arc = self.next_arc(arc);
        }
        0
    }

    fn is_arc_last(&self, arc: usize) -> bool {
        self.arcs[arc + ADDRESS_OFFSET] & BIT_LAST_ARC != 0
    }

    fn is_next_set(&self, arc: usize) -> bool {
        self.arcs[arc + ADDRESS_OFFSET] & BIT_TARGET_NEXT != 0
    }

    /// `FSA5.getDestinationNodeOffset`.
    fn destination_node_offset(&self, arc: usize) -> usize {
        if self.is_next_set(arc) {
            self.skip_arc(arc)
        } else {
            decode_from_bytes(&self.arcs, arc + ADDRESS_OFFSET, self.gtl) >> 3
        }
    }

    fn skip_arc(&self, offset: usize) -> usize {
        offset
            + if self.is_next_set(offset) {
                2 // label + flags
            } else {
                1 + self.gtl // label + flags/address
            }
    }
}

/// `FSA5.decodeFromBytes`: little-endian packed integer.
fn decode_from_bytes(arcs: &[u8], start: usize, n: usize) -> usize {
    let mut r = 0usize;
    for i in (0..n).rev() {
        r = (r << 8) | (arcs[start + i] as usize);
    }
    r
}
