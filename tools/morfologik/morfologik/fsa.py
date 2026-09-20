"""Morfologik finite-state automaton readers.

This is a faithful Python port of the read side of ``morfologik-fsa``
(https://github.com/morfologik/morfologik-stemming, BSD-3-Clause).  Only the
formats the LanguageTool dictionaries actually use are implemented:

* :class:`ConstantArcSizeFSA` -- the in-memory automaton produced by
  :class:`morfologik.builder.FSABuilder` (fixed 6-byte arcs).
* :class:`CFSA2` -- the compressed, serialized format written by
  :class:`morfologik.serializer.CFSA2Serializer` (version byte ``0xc6``).

The older ``FSA5``/``CFSA`` formats are intentionally not ported; LanguageTool
always compiles dictionaries with ``SerializationFormat.CFSA2``.
"""

from __future__ import annotations

from typing import Callable, Iterator, List, Optional

# --- ConstantArcSizeFSA constants (morfologik.fsa.builders.ConstantArcSizeFSA) ---
TARGET_ADDRESS_SIZE = 4
FLAGS_SIZE = 1
LABEL_SIZE = 1
ARC_SIZE = FLAGS_SIZE + LABEL_SIZE + TARGET_ADDRESS_SIZE
FLAGS_OFFSET = 0
LABEL_OFFSET = FLAGS_SIZE
ADDRESS_OFFSET = LABEL_OFFSET + LABEL_SIZE
TERMINAL_STATE = 0
BIT_ARC_FINAL = 1 << 1
BIT_ARC_LAST = 1 << 0

# --- CFSA2 constants (morfologik.fsa.CFSA2) ---
CFSA2_VERSION = 0xC6
CFSA2_BIT_TARGET_NEXT = 1 << 7
CFSA2_BIT_LAST_ARC = 1 << 6
CFSA2_BIT_FINAL_ARC = 1 << 5
LABEL_INDEX_BITS = 5
LABEL_INDEX_MASK = (1 << LABEL_INDEX_BITS) - 1
LABEL_INDEX_SIZE = (1 << LABEL_INDEX_BITS) - 1

# --- FSA5 constants (morfologik.fsa.FSA5) ---
FSA5_VERSION = 5
FSA5_BIT_FINAL_ARC = 1 << 0
FSA5_BIT_LAST_ARC = 1 << 1
FSA5_BIT_TARGET_NEXT = 1 << 2
FSA5_ADDRESS_OFFSET = 1

# --- FSAFlags bits ---
FLAG_FLEXIBLE = 1 << 0
FLAG_STOPBIT = 1 << 1
FLAG_NEXTBIT = 1 << 2
FLAG_NUMBERS = 1 << 8

FSA_MAGIC = b"\\fsa"


class FSAError(Exception):
    """Raised for malformed automaton data."""


def read_vint(array: bytes, offset: int) -> int:
    """Read a little-endian 7-bit-encoded unsigned integer."""
    b = array[offset]
    value = b & 0x7F
    shift = 7
    while b >= 0x80:
        offset += 1
        b = array[offset]
        value |= (b & 0x7F) << shift
        shift += 7
    return value


def skip_vint(array: bytes, offset: int) -> int:
    while array[offset] >= 0x80:
        offset += 1
    return offset + 1


def write_vint(value: int) -> bytes:
    """Write a little-endian 7-bit-encoded unsigned integer."""
    if value < 0:
        raise ValueError("Can't v-code negative ints.")
    out = bytearray()
    while value > 0x7F:
        out.append(0x80 | (value & 0x7F))
        value >>= 7
    out.append(value)
    return bytes(out)


class FSA:
    """Abstract, arc-based finite-state automaton.

    Node and arc identifiers are integer byte offsets into the automaton's
    backing data.  Arc identifier ``0`` means "no arc".
    """

    def get_root_node(self) -> int:  # pragma: no cover - interface
        raise NotImplementedError

    def get_first_arc(self, node: int) -> int:  # pragma: no cover - interface
        raise NotImplementedError

    def get_next_arc(self, arc: int) -> int:  # pragma: no cover - interface
        raise NotImplementedError

    def get_arc_label(self, arc: int) -> int:  # pragma: no cover - interface
        raise NotImplementedError

    def is_arc_final(self, arc: int) -> bool:  # pragma: no cover - interface
        raise NotImplementedError

    def is_arc_terminal(self, arc: int) -> bool:  # pragma: no cover - interface
        raise NotImplementedError

    def get_end_node(self, arc: int) -> int:  # pragma: no cover - interface
        raise NotImplementedError

    def get_arc(self, node: int, label: int) -> int:
        arc = self.get_first_arc(node)
        while arc != 0:
            if self.get_arc_label(arc) == label:
                return arc
            arc = self.get_next_arc(arc)
        return 0

    def get_arc_count(self, node: int) -> int:
        count = 0
        arc = self.get_first_arc(node)
        while arc != 0:
            count += 1
            arc = self.get_next_arc(arc)
        return count

    def iter_sequences(self, node: Optional[int] = None) -> Iterator[bytes]:
        """Iterate all sequences reachable from ``node`` (default: root).

        Port of ``morfologik.fsa.ByteSequenceIterator``.  The traversal order
        matches the Java implementation, which matters for reproducible
        decompilation.
        """
        if node is None:
            node = self.get_root_node()
        if self.get_first_arc(node) == 0:
            return
        stack: List[int] = [self.get_first_arc(node)]
        buf = bytearray()
        while stack:
            last = len(stack) - 1
            arc = stack[last]
            if arc == 0:
                stack.pop()
                continue
            stack[last] = self.get_next_arc(arc)
            if last >= len(buf):
                buf.extend(b"\x00" * (last + 1 - len(buf)))
            buf[last] = self.get_arc_label(arc)
            if not self.is_arc_terminal(arc):
                stack.append(self.get_first_arc(self.get_end_node(arc)))
            if self.is_arc_final(arc):
                yield bytes(buf[: last + 1])

    def visit_in_post_order(self, visitor: Callable[[int], bool], node: Optional[int] = None) -> None:
        if node is None:
            node = self.get_root_node()
        visited = set()

        def rec(n: int) -> bool:
            if n in visited:
                return True
            visited.add(n)
            arc = self.get_first_arc(n)
            while arc != 0:
                if not self.is_arc_terminal(arc):
                    if not rec(self.get_end_node(arc)):
                        return False
                arc = self.get_next_arc(arc)
            return visitor(n)

        rec(node)

    def visit_in_pre_order(self, visitor: Callable[[int], bool], node: Optional[int] = None) -> None:
        if node is None:
            node = self.get_root_node()
        visited = set()

        def rec(n: int) -> None:
            if n in visited:
                return
            visited.add(n)
            if visitor(n):
                arc = self.get_first_arc(n)
                while arc != 0:
                    if not self.is_arc_terminal(arc):
                        rec(self.get_end_node(arc))
                    arc = self.get_next_arc(arc)

        rec(node)


class ConstantArcSizeFSA(FSA):
    """An FSA with constant-size arc representation (6 bytes per arc)."""

    def __init__(self, data: bytes, epsilon: int = 0) -> None:
        self.data = data
        self.epsilon = epsilon

    def get_root_node(self) -> int:
        return self.get_end_node(self.get_first_arc(self.epsilon))

    def get_first_arc(self, node: int) -> int:
        return node

    def get_next_arc(self, arc: int) -> int:
        if self._is_arc_last(arc):
            return 0
        return arc + ARC_SIZE

    def get_arc_label(self, arc: int) -> int:
        return self.data[arc + LABEL_OFFSET]

    def is_arc_final(self, arc: int) -> bool:
        return bool(self.data[arc + FLAGS_OFFSET] & BIT_ARC_FINAL)

    def is_arc_terminal(self, arc: int) -> bool:
        return self._get_arc_target(arc) == TERMINAL_STATE

    def get_end_node(self, arc: int) -> int:
        return self._get_arc_target(arc)

    def _is_arc_last(self, arc: int) -> bool:
        return bool(self.data[arc + FLAGS_OFFSET] & BIT_ARC_LAST)

    def _get_arc_target(self, arc: int) -> int:
        start = arc + ADDRESS_OFFSET
        return int.from_bytes(self.data[start : start + TARGET_ADDRESS_SIZE], "big")


class CFSA2(FSA):
    """A compact finite-state automaton, version 2."""

    def __init__(self, arcs: bytes, label_mapping: bytes, has_numbers: bool = False) -> None:
        self.arcs = arcs
        self.label_mapping = label_mapping
        self.has_numbers = has_numbers
        self.epsilon = 0

    @classmethod
    def parse(cls, data: bytes) -> "CFSA2":
        if len(data) < 8:
            raise FSAError("file too short")
        if data[:4] != FSA_MAGIC:
            raise FSAError(f"bad magic {data[:4]!r}, expected {FSA_MAGIC!r}")
        version = data[4]
        if version != CFSA2_VERSION:
            raise FSAError(f"unsupported FSA version {version:#04x} (only CFSA2 supported)")
        flags = int.from_bytes(data[5:7], "big")
        has_numbers = bool(flags & FLAG_NUMBERS)
        pos = 7
        label_mapping_size = data[pos]
        pos += 1
        label_mapping = data[pos : pos + label_mapping_size]
        pos += label_mapping_size
        return cls(data[pos:], label_mapping, has_numbers)

    def get_root_node(self) -> int:
        return self._destination_node_offset(self.get_first_arc(self.epsilon))

    def get_first_arc(self, node: int) -> int:
        if self.has_numbers:
            return skip_vint(self.arcs, node)
        return node

    def get_next_arc(self, arc: int) -> int:
        if self._is_arc_last(arc):
            return 0
        return self._skip_arc(arc)

    def get_arc_label(self, arc: int) -> int:
        index = self.arcs[arc] & LABEL_INDEX_MASK
        if index > 0:
            return self.label_mapping[index]
        return self.arcs[arc + 1]

    def is_arc_final(self, arc: int) -> bool:
        return bool(self.arcs[arc] & CFSA2_BIT_FINAL_ARC)

    def is_arc_terminal(self, arc: int) -> bool:
        return self._destination_node_offset(arc) == 0

    def get_end_node(self, arc: int) -> int:
        return self._destination_node_offset(arc)

    def get_right_language_count(self, node: int) -> int:
        if not self.has_numbers:
            raise ValueError("Automaton not compiled with NUMBERS")
        return read_vint(self.arcs, node)

    def _is_arc_last(self, arc: int) -> bool:
        return bool(self.arcs[arc] & CFSA2_BIT_LAST_ARC)

    def _is_next_set(self, arc: int) -> bool:
        return bool(self.arcs[arc] & CFSA2_BIT_TARGET_NEXT)

    def _destination_node_offset(self, arc: int) -> int:
        if self._is_next_set(arc):
            while not self._is_arc_last(arc):
                arc = self.get_next_arc(arc)
            return self._skip_arc(arc)
        start = arc + (2 if (self.arcs[arc] & LABEL_INDEX_MASK) == 0 else 1)
        return read_vint(self.arcs, start)

    def _skip_arc(self, offset: int) -> int:
        flag = self.arcs[offset]
        offset += 1
        if (flag & LABEL_INDEX_MASK) == 0:
            offset += 1
        if (flag & CFSA2_BIT_TARGET_NEXT) == 0:
            offset = skip_vint(self.arcs, offset)
        return offset


class FSA5(FSA):
    """The legacy FSA version-5 format (used by some older LT dictionaries)."""

    def __init__(self, arcs: bytes, node_data_length: int, gtl: int) -> None:
        self.arcs = arcs
        self.node_data_length = node_data_length
        self.gtl = gtl

    @classmethod
    def parse(cls, data: bytes) -> "FSA5":
        if len(data) < 8:
            raise FSAError("file too short")
        if data[:4] != FSA_MAGIC:
            raise FSAError(f"bad magic {data[:4]!r}, expected {FSA_MAGIC!r}")
        if data[4] != FSA5_VERSION:
            raise FSAError(f"unsupported FSA version {data[4]:#04x} (FSA5 parser)")
        hgtl = data[7]
        node_data_length = (hgtl >> 4) & 0x0F
        gtl = hgtl & 0x0F
        return cls(data[8:], node_data_length, gtl)

    def get_root_node(self) -> int:
        epsilon_node = self._skip_arc(self.get_first_arc(0))
        return self._destination_node_offset(self.get_first_arc(epsilon_node))

    def get_first_arc(self, node: int) -> int:
        return self.node_data_length + node

    def get_next_arc(self, arc: int) -> int:
        if self._is_arc_last(arc):
            return 0
        return self._skip_arc(arc)

    def get_arc_label(self, arc: int) -> int:
        return self.arcs[arc]

    def is_arc_final(self, arc: int) -> bool:
        return bool(self.arcs[arc + FSA5_ADDRESS_OFFSET] & FSA5_BIT_FINAL_ARC)

    def is_arc_terminal(self, arc: int) -> bool:
        return self._destination_node_offset(arc) == 0

    def get_end_node(self, arc: int) -> int:
        return self._destination_node_offset(arc)

    def _is_arc_last(self, arc: int) -> bool:
        return bool(self.arcs[arc + FSA5_ADDRESS_OFFSET] & FSA5_BIT_LAST_ARC)

    def _is_next_set(self, arc: int) -> bool:
        return bool(self.arcs[arc + FSA5_ADDRESS_OFFSET] & FSA5_BIT_TARGET_NEXT)

    def _destination_node_offset(self, arc: int) -> int:
        if self._is_next_set(arc):
            return self._skip_arc(arc)
        value = int.from_bytes(
            self.arcs[arc + FSA5_ADDRESS_OFFSET : arc + FSA5_ADDRESS_OFFSET + self.gtl], "little"
        )
        return value >> 3

    def _skip_arc(self, offset: int) -> int:
        if self._is_next_set(offset):
            return offset + 2
        return offset + 1 + self.gtl


def read_automaton(data: bytes) -> FSA:
    """Read a serialized automaton, dispatching on the version byte."""
    if len(data) < 5:
        raise FSAError("file too short")
    if data[:4] != FSA_MAGIC:
        raise FSAError(f"bad magic {data[:4]!r}, expected {FSA_MAGIC!r}")
    version = data[4]
    if version == CFSA2_VERSION:
        return CFSA2.parse(data)
    if version == FSA5_VERSION:
        return FSA5.parse(data)
    raise FSAError(f"unsupported automaton version: {version:#04x}")
