"""Morfologik FSA builder.

Faithful Python port of ``morfologik.fsa.builders.FSABuilder`` and
``ConstantArcSizeFSA`` (BSD-3-Clause, https://github.com/morfologik/morfologik-stemming).

``FSABuilder`` incrementally builds a minimal deterministic acyclic FSA from a
sorted list of byte sequences (Daciuk's algorithm).  The result is a
:class:`morfologik.fsa.ConstantArcSizeFSA`, which
:class:`morfologik.serializer.CFSA2Serializer` turns into the on-disk format.
"""

from __future__ import annotations

from typing import Iterable, List

from .fsa import (
    ADDRESS_OFFSET,
    ARC_SIZE,
    BIT_ARC_FINAL,
    BIT_ARC_LAST,
    FLAGS_OFFSET,
    LABEL_OFFSET,
    ConstantArcSizeFSA,
)

BUFFER_GROWTH_SIZE = 5 * 1024 * 1024
MAX_LABELS = 256


def _signed_byte(value: int) -> int:
    return value - 256 if value >= 128 else value


class FSABuilder:
    """Build a minimal deterministic FSA from sorted byte sequences."""

    def __init__(self, buffer_growth_size: int = BUFFER_GROWTH_SIZE) -> None:
        self.buffer_growth_size = max(buffer_growth_size, ARC_SIZE * MAX_LABELS)
        self.serialized = bytearray()
        self.size = 0
        self.active_path: List[int] = []
        self.active_path_len = 0
        self.next_arc_offset: List[int] = []
        self.hash_set: List[int] = [0, 0]
        self.hash_size = 0

        # Allocate epsilon state.
        self.epsilon = self._allocate_state(1)
        self.serialized[self.epsilon + FLAGS_OFFSET] |= BIT_ARC_LAST

        # Allocate root, with an initial empty set of output arcs.
        self._expand_active_path(1)
        self.root = self.active_path[0]

    # -- construction -----------------------------------------------------

    def add(self, sequence: bytes, start: int = 0, length: int | None = None) -> None:
        if length is None:
            length = len(sequence) - start

        common_prefix = self._common_prefix(sequence, start, length)
        self._expand_active_path(length)

        # Freeze all states after the common prefix.
        for i in range(self.active_path_len - 1, common_prefix, -1):
            frozen_state = self._freeze_state(i)
            self._set_arc_target(self.next_arc_offset[i - 1] - ARC_SIZE, frozen_state)
            self.next_arc_offset[i] = self.active_path[i]

        # Create arcs to new suffix states.
        j = start + common_prefix
        for i in range(common_prefix + 1, length + 1):
            p = self.next_arc_offset[i - 1]
            self.serialized[p + FLAGS_OFFSET] = BIT_ARC_FINAL if i == length else 0
            self.serialized[p + LABEL_OFFSET] = sequence[j]
            j += 1
            self._set_arc_target(p, 0 if i == length else self.active_path[i])
            self.next_arc_offset[i - 1] = p + ARC_SIZE

        self.active_path_len = length

    def complete(self) -> ConstantArcSizeFSA:
        self.add(b"", 0, 0)

        if self.next_arc_offset[0] - self.active_path[0] == 0:
            self._set_arc_target(self.epsilon, 0)
        else:
            self.root = self._freeze_state(0)
            self._set_arc_target(self.epsilon, self.root)

        return ConstantArcSizeFSA(bytes(self.serialized[: self.size]), self.epsilon)

    @classmethod
    def build(cls, input_sequences: Iterable[bytes]) -> ConstantArcSizeFSA:
        builder = cls()
        for sequence in input_sequences:
            builder.add(sequence)
        return builder.complete()

    # -- internals --------------------------------------------------------

    def _common_prefix(self, sequence: bytes, start: int, length: int) -> int:
        max_len = min(length, self.active_path_len)
        i = 0
        while i < max_len:
            last_arc = self.next_arc_offset[i] - ARC_SIZE
            if sequence[start + i] != self.serialized[last_arc + LABEL_OFFSET]:
                break
            i += 1
        return i

    def _freeze_state(self, active_path_index: int) -> int:
        start = self.active_path[active_path_index]
        end = self.next_arc_offset[active_path_index]
        length = end - start

        self.serialized[end - ARC_SIZE + FLAGS_OFFSET] |= BIT_ARC_LAST

        bucket_mask = len(self.hash_set) - 1
        slot = self._hash(start, length) & bucket_mask
        i = 0
        while True:
            state = self.hash_set[slot]
            if state == 0:
                state = self._serialize(active_path_index)
                self.hash_set[slot] = state
                self.hash_size += 1
                if self.hash_size > len(self.hash_set) // 2:
                    self._expand_and_rehash()
                return state
            if self._equivalent(state, start, length):
                return state
            i += 1
            slot = (slot + i) & bucket_mask

    def _expand_and_rehash(self) -> None:
        new_hash_set = [0] * (len(self.hash_set) * 2)
        bucket_mask = len(new_hash_set) - 1
        for state in self.hash_set:
            if state > 0:
                slot = self._hash(state, self._state_length(state)) & bucket_mask
                i = 0
                while new_hash_set[slot] > 0:
                    i += 1
                    slot = (slot + i) & bucket_mask
                new_hash_set[slot] = state
        self.hash_set = new_hash_set

    def _state_length(self, state: int) -> int:
        arc = state
        while not self._is_arc_last(arc):
            arc += ARC_SIZE
        return arc - state + ARC_SIZE

    def _equivalent(self, start1: int, start2: int, length: int) -> bool:
        if start1 + length > self.size or start2 + length > self.size:
            return False
        return self.serialized[start1 : start1 + length] == self.serialized[start2 : start2 + length]

    def _serialize(self, active_path_index: int) -> int:
        self._expand_buffers()
        new_state = self.size
        start = self.active_path[active_path_index]
        length = self.next_arc_offset[active_path_index] - start
        self._ensure(new_state + length)
        self.serialized[new_state : new_state + length] = self.serialized[start : start + length]
        self.size += length
        return new_state

    def _hash(self, start: int, byte_count: int) -> int:
        h = 0
        arcs = byte_count // ARC_SIZE
        while arcs > 0:
            arcs -= 1
            h = 17 * h + _signed_byte(self.serialized[start + LABEL_OFFSET])
            h = 17 * h + self._get_arc_target(start)
            if self.serialized[start + FLAGS_OFFSET] & BIT_ARC_FINAL:
                h += 17
            start += ARC_SIZE
        return h

    def _expand_active_path(self, size: int) -> None:
        if len(self.active_path) < size:
            p = len(self.active_path)
            self.active_path.extend([0] * (size - p))
            self.next_arc_offset.extend([0] * (size - p))
            for i in range(p, size):
                self.next_arc_offset[i] = self.active_path[i] = self._allocate_state(MAX_LABELS)

    def _expand_buffers(self) -> None:
        if len(self.serialized) < self.size + ARC_SIZE * MAX_LABELS:
            self._ensure(len(self.serialized) + self.buffer_growth_size)

    def _allocate_state(self, labels: int) -> int:
        self._expand_buffers()
        state = self.size
        self.size += labels * ARC_SIZE
        self._ensure(self.size)
        return state

    def _ensure(self, capacity: int) -> None:
        if len(self.serialized) < capacity:
            self.serialized.extend(b"\x00" * (capacity - len(self.serialized)))

    def _is_arc_last(self, arc: int) -> bool:
        return bool(self.serialized[arc + FLAGS_OFFSET] & BIT_ARC_LAST)

    def _set_arc_target(self, arc: int, state: int) -> None:
        start = arc + ADDRESS_OFFSET
        self.serialized[start : start + 4] = state.to_bytes(4, "big")

    def _get_arc_target(self, arc: int) -> int:
        start = arc + ADDRESS_OFFSET
        return int.from_bytes(self.serialized[start : start + 4], "big")
