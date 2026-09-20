"""Morfologik CFSA2 serializer.

Faithful Python port of ``morfologik.fsa.builders.CFSA2Serializer`` (BSD-3-Clause,
https://github.com/morfologik/morfologik-stemming).  The output is byte-for-byte
identical to the Java implementation for the same in-memory automaton.
"""

from __future__ import annotations

from typing import Dict, List, Optional

from .fsa import (
    CFSA2_BIT_FINAL_ARC,
    CFSA2_BIT_LAST_ARC,
    CFSA2_BIT_TARGET_NEXT,
    CFSA2_VERSION,
    FLAG_FLEXIBLE,
    FLAG_NEXTBIT,
    FLAG_NUMBERS,
    FLAG_STOPBIT,
    FSA_MAGIC,
    LABEL_INDEX_SIZE,
    FSA,
    write_vint,
)

NO_STATE = -1
MAX_OFFSET = 0x7FFFFFFF


class CFSA2Serializer:
    """Serialize an in-memory :class:`~morfologik.fsa.FSA` to CFSA2."""

    def __init__(self, with_numbers: bool = False) -> None:
        self.with_numbers = with_numbers
        self.offsets: Dict[int, int] = {}
        self.numbers: Dict[int, int] = {}
        self.labels_index = bytearray()
        self.labels_inv_index = [0] * 256
        # Cache of per-state arcs: (label, is_final, is_terminal, end_node, is_last).
        # The automaton is immutable during serialization, so this only removes
        # repeated attribute/method access; the emitted bytes are unchanged.
        self._arc_cache: Dict[int, List[tuple]] = {}

    def serialize(self, fsa: FSA, out: bytearray) -> bytearray:
        self._arc_cache = {}
        self._compute_labels_index(fsa)
        if self.with_numbers:
            self.numbers = self._right_language_for_all_states(fsa)

        linearized = self._linearize(fsa)

        out += FSA_MAGIC
        out.append(CFSA2_VERSION)
        flags = FLAG_FLEXIBLE | FLAG_STOPBIT | FLAG_NEXTBIT
        if self.with_numbers:
            flags |= FLAG_NUMBERS
        out += flags.to_bytes(2, "big")
        out.append(len(self.labels_index))
        out += self.labels_index

        size = self._emit_nodes(fsa, out, linearized)
        assert size == 0, "Size changed in the final pass?"
        return out

    def _state_arcs(self, fsa: FSA, state: int) -> List[tuple]:
        cached = self._arc_cache.get(state)
        if cached is not None:
            return cached
        arcs: List[tuple] = []
        arc = fsa.get_first_arc(state)
        while arc != 0:
            is_terminal = fsa.is_arc_terminal(arc)
            end_node = 0 if is_terminal else fsa.get_end_node(arc)
            arcs.append(
                (
                    fsa.get_arc_label(arc),
                    fsa.is_arc_final(arc),
                    is_terminal,
                    end_node,
                    fsa.get_next_arc(arc) == 0,
                )
            )
            arc = fsa.get_next_arc(arc)
        self._arc_cache[state] = arcs
        return arcs

    # -- label index ------------------------------------------------------

    def _compute_labels_index(self, fsa: FSA) -> None:
        count_by_value = [0] * 256

        def count(state: int) -> bool:
            for label, _final, _terminal, _end, _last in self._state_arcs(fsa, state):
                count_by_value[label & 0xFF] += 1
            return True

        fsa.visit_in_post_order(count)

        label_and_count = sorted(
            ((label, cnt) for label, cnt in enumerate(count_by_value) if cnt > 0),
            key=lambda item: (-item[1], item[0]),
        )

        self.labels_index = bytearray(1 + min(len(label_and_count), LABEL_INDEX_SIZE))
        self.labels_inv_index = [0] * 256
        idx = 0
        for i in range(len(self.labels_index) - 1, 0, -1):
            if idx >= len(label_and_count):
                break
            label, _count = label_and_count[idx]
            idx += 1
            self.labels_inv_index[label] = i
            self.labels_index[i] = label

    # -- linearization ----------------------------------------------------

    def _linearize(self, fsa: FSA) -> List[int]:
        inlink_count = self._compute_inlink_count(fsa)
        linearized: List[int] = []

        states = self._compute_first_states(inlink_count)

        serialized_size = self._linearize_and_calculate_offsets(fsa, [], linearized)

        cut_at = 0
        upper = min(150, len(states))
        cut = min(25, len(states))
        while cut <= upper:
            new_size = self._linearize_and_calculate_offsets(fsa, states[:cut], linearized)
            if new_size >= serialized_size:
                break
            cut_at = cut
            cut += 25

        self._linearize_and_calculate_offsets(fsa, states[:cut_at], linearized)
        return linearized

    def _linearize_and_calculate_offsets(self, fsa: FSA, states: List[int], linearized: List[int]) -> int:
        visited = set()
        nodes: List[int] = []
        linearized.clear()

        for state in states:
            self._linearize_state(fsa, nodes, linearized, visited, state)

        nodes.append(fsa.get_root_node())
        while nodes:
            node = nodes.pop()
            if node in visited:
                continue
            self._linearize_state(fsa, nodes, linearized, visited, node)

        for state in linearized:
            self.offsets[state] = MAX_OFFSET

        j = 0
        while True:
            i = self._emit_nodes(fsa, None, linearized)
            if i > 0:
                j = i
            else:
                break
        return j

    def _linearize_state(self, fsa: FSA, nodes: List[int], linearized: List[int], visited: set, node: int) -> None:
        linearized.append(node)
        visited.add(node)
        for _label, _final, is_terminal, end_node, _last in self._state_arcs(fsa, node):
            if not is_terminal and end_node not in visited:
                nodes.append(end_node)

    @staticmethod
    def _compute_first_states(inlink_count: Dict[int, int], max_states: int = 0x7FFFFFFF, min_inlink_count: int = 2) -> List[int]:
        candidates = sorted(
            (count, state) for state, count in inlink_count.items() if count > min_inlink_count
        )
        if len(candidates) > max_states:
            candidates = candidates[-max_states:]
        return [state for _count, state in reversed(candidates)]

    def _compute_inlink_count(self, fsa: FSA) -> Dict[int, int]:
        inlink_count: Dict[int, int] = {}
        visited = set()
        nodes = [fsa.get_root_node()]
        while nodes:
            node = nodes.pop()
            if node in visited:
                continue
            visited.add(node)
            for _label, _final, is_terminal, end_node, _last in self._state_arcs(fsa, node):
                if not is_terminal:
                    inlink_count[end_node] = inlink_count.get(end_node, 0) + 1
                    if end_node not in visited:
                        nodes.append(end_node)
        return inlink_count

    # -- emission ---------------------------------------------------------

    def _emit_nodes(self, fsa: FSA, out: Optional[bytearray], linearized: List[int]) -> int:
        offset = 0

        offset += self._emit_node_data(out, 0)
        root = fsa.get_root_node()
        if root != 0:
            offset += self._emit_arc(out, CFSA2_BIT_LAST_ARC, ord("^"), self.offsets.get(root, 0))
        else:
            offset += self._emit_arc(out, CFSA2_BIT_LAST_ARC, ord("^"), 0)

        offsets_changed = False
        max_len = len(linearized)
        for index, state in enumerate(linearized):
            next_state = linearized[index + 1] if index + 1 < max_len else NO_STATE

            if out is None:
                if self.offsets.get(state, 0) != offset:
                    offsets_changed = True
                self.offsets[state] = offset
            else:
                assert self.offsets.get(state, 0) == offset, (
                    state,
                    self.offsets.get(state, 0),
                    offset,
                )

            number = self.numbers.get(state, 0) if self.with_numbers else 0
            offset += self._emit_node_data(out, number)
            offset += self._emit_node_arcs(fsa, out, state, next_state)

        return offset if offsets_changed else 0

    def _emit_node_arcs(self, fsa: FSA, out: Optional[bytearray], state: int, next_state: int) -> int:
        offset = 0
        for label, is_final, is_terminal, target, is_last in self._state_arcs(fsa, state):
            if is_terminal:
                target_offset = 0
            else:
                target_offset = self.offsets.get(target, 0)

            flags = 0
            if is_final:
                flags |= CFSA2_BIT_FINAL_ARC
            if is_last:
                flags |= CFSA2_BIT_LAST_ARC
            if target_offset != 0 and target == next_state:
                flags |= CFSA2_BIT_TARGET_NEXT
                target_offset = 0

            offset += self._emit_arc(out, flags, label, target_offset)
        return offset

    def _emit_arc(self, out: Optional[bytearray], flags: int, label: int, target_offset: int) -> int:
        length = 0
        label_index = self.labels_inv_index[label & 0xFF]
        if label_index > 0:
            if out is not None:
                out.append(flags | label_index)
            length += 1
        else:
            if out is not None:
                out.append(flags)
                out.append(label)
            length += 2

        if (flags & CFSA2_BIT_TARGET_NEXT) == 0:
            encoded = write_vint(target_offset)
            if out is not None:
                out += encoded
            length += len(encoded)

        return length

    def _emit_node_data(self, out: Optional[bytearray], number: int) -> int:
        if not self.with_numbers:
            return 0
        encoded = write_vint(number)
        if out is not None:
            out += encoded
        return len(encoded)

    def _right_language_for_all_states(self, fsa: FSA) -> Dict[int, int]:
        numbers: Dict[int, int] = {}

        def visit(state: int) -> bool:
            this_node_number = 0
            for _label, is_final, is_terminal, end_node, _last in self._state_arcs(fsa, state):
                this_node_number += 1 if is_final else 0
                if not is_terminal:
                    this_node_number += numbers.get(end_node, 0)
            numbers[state] = this_node_number
            return True

        fsa.visit_in_post_order(visit)
        return numbers


def compile_fsa(sequences) -> bytes:
    """Build and serialize a CFSA2 automaton from an iterable of byte sequences."""
    from .builder import FSABuilder

    fsa = FSABuilder.build(sequences)
    out = bytearray()
    CFSA2Serializer().serialize(fsa, out)
    return bytes(out)
