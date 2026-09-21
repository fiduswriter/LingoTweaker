//! CFSA (Compact Finite State Automaton, version byte `0xc5`) reader — the
//! older Morfologik format used by the Asturian tagger dictionary
//! (`ast/dictionaries/asturian.dict`). Port of `morfologik.fsa.CFSA`
//! (`getFirstArc`/`getNextArc`/`getArcLabel`/`getDestinationNodeOffset`).

use lt_core::Result;

pub const VERSION_CFSA: u8 = 0xc5;

const BIT_FINAL_ARC: u8 = 1 << 0;
const BIT_LAST_ARC: u8 = 1 << 1;
const BIT_TARGET_NEXT: u8 = 1 << 2;

#[derive(Debug, Clone)]
pub struct Cfsa {
    arcs: Vec<u8>,
    node_data_length: usize,
    gtl: usize,
    label_mapping: [u8; 32],
    root: usize,
}

impl Cfsa {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 8 {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                "file too short".into(),
            ));
        }
        if &bytes[..4] != b"\\fsa" {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("bad magic {:02x?}, expected \\\\fsa (CFSA)", &bytes[..4]),
            ));
        }
        if bytes[4] != VERSION_CFSA {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                format!("unsupported FSA version {:#04x} (CFSA parser)", bytes[4]),
            ));
        }
        // header: filler, annotation, hgtl
        let hgtl = bytes[7];
        let (node_data_length, gtl) = if (hgtl & 0xf0) != 0 {
            (((hgtl >> 4) & 0x0f) as usize, (hgtl & 0x0f) as usize)
        } else {
            (0usize, (hgtl & 0x0f) as usize)
        };
        if bytes.len() < 40 {
            return Err(lt_core::CoreError::Parse(
                "dict".into(),
                "file too short".into(),
            ));
        }
        let mut label_mapping = [0u8; 32];
        label_mapping.copy_from_slice(&bytes[8..40]);
        let mut fsa = Self {
            arcs: bytes[40..].to_vec(),
            node_data_length,
            gtl,
            label_mapping,
            root: 0,
        };
        // `CFSA.getRootNode`: skip the dummy terminating node's arc and follow
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
        if self.is_next_set(arc) && self.is_label_compressed(arc) {
            self.label_mapping[((self.arcs[arc] as usize) >> 3) & 0x1f]
        } else {
            self.arcs[arc + 1]
        }
    }

    pub fn is_arc_final(&self, arc: usize) -> bool {
        self.arcs[arc] & BIT_FINAL_ARC != 0
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
        self.arcs[arc] & BIT_LAST_ARC != 0
    }

    fn is_next_set(&self, arc: usize) -> bool {
        self.arcs[arc] & BIT_TARGET_NEXT != 0
    }

    fn is_label_compressed(&self, arc: usize) -> bool {
        self.arcs[arc] & 0xf8 != 0
    }

    /// `CFSA.getDestinationNodeOffset`.
    fn destination_node_offset(&self, arc: usize) -> usize {
        if self.is_next_set(arc) {
            self.skip_arc(arc)
        } else {
            // byte 0 holds the flags plus the address low bits, byte 1 the
            // label, bytes 2..gtl the address continuation.
            let mut r = self.arcs[arc] as usize;
            for i in 2..=self.gtl {
                r |= (self.arcs[arc + i] as usize) << (8 * (i - 1));
            }
            r >> 3
        }
    }

    /// `CFSA.skipArc`.
    fn skip_arc(&self, offset: usize) -> usize {
        if self.is_next_set(offset) {
            if self.is_label_compressed(offset) {
                offset + 1
            } else {
                offset + 2
            }
        } else {
            offset + 1 + self.gtl
        }
    }
}
