"""Morfologik sequence encoders.

Port of ``morfologik.stemming`` encoders (BSD-3-Clause,
https://github.com/morfologik/morfologik-stemming).  A sequence encoder stores
the inflected form relative to its base form so the automaton shares prefixes.
LanguageTool dictionaries use ``SUFFIX``.
"""

from __future__ import annotations

from typing import Dict, Type

REMOVE_EVERYTHING = 255


def _common_prefix(a: bytes, b: bytes) -> int:
    n = min(len(a), len(b))
    i = 0
    while i < n and a[i] == b[i]:
        i += 1
    return i


class ISequenceEncoder:
    prefix_bytes = 0

    def encode(self, source: bytes, target: bytes) -> bytes:  # pragma: no cover - interface
        raise NotImplementedError

    def decode(self, source: bytes, encoded: bytes) -> bytes:  # pragma: no cover - interface
        raise NotImplementedError


class NoEncoder(ISequenceEncoder):
    """No relative encoding at all (full target form is stored)."""

    prefix_bytes = 0

    def encode(self, source: bytes, target: bytes) -> bytes:
        return target

    def decode(self, source: bytes, encoded: bytes) -> bytes:
        return encoded


class TrimSuffixEncoder(ISequenceEncoder):
    """Trim the non-equal suffix of ``source`` and append the target suffix.

    ``{K}{suffix}`` where ``K - 'A'`` bytes are trimmed from the end of
    ``source`` before appending ``suffix``.
    """

    prefix_bytes = 1

    def encode(self, source: bytes, target: bytes) -> bytes:
        shared_prefix = _common_prefix(source, target)
        truncate_bytes = len(source) - shared_prefix
        if truncate_bytes >= REMOVE_EVERYTHING:
            truncate_bytes = REMOVE_EVERYTHING
            shared_prefix = 0
        code = (truncate_bytes + ord("A")) & 0xFF
        return bytes([code]) + target[shared_prefix:]

    def decode(self, source: bytes, encoded: bytes) -> bytes:
        truncate_bytes = (encoded[0] - ord("A")) & 0xFF
        if truncate_bytes == REMOVE_EVERYTHING:
            truncate_bytes = len(source)
        len1 = len(source) - truncate_bytes
        return source[:len1] + encoded[1:]


class TrimPrefixAndSuffixEncoder(ISequenceEncoder):
    """``{P}{K}{suffix}``: trim a prefix and suffix of ``source``, append suffix."""

    prefix_bytes = 2

    def encode(self, source: bytes, target: bytes) -> bytes:
        max_subsequence_length = 0
        max_subsequence_index = 0
        for i in range(len(source)):
            shared_prefix = _common_prefix(source[i:], target)
            if (
                shared_prefix > max_subsequence_length
                and i < REMOVE_EVERYTHING
                and (len(source) - (i + shared_prefix)) < REMOVE_EVERYTHING
            ):
                max_subsequence_length = shared_prefix
                max_subsequence_index = i

        truncate_prefix_bytes = max_subsequence_index
        truncate_suffix_bytes = len(source) - (max_subsequence_index + max_subsequence_length)
        if truncate_prefix_bytes >= REMOVE_EVERYTHING or truncate_suffix_bytes >= REMOVE_EVERYTHING:
            max_subsequence_length = 0
            truncate_prefix_bytes = truncate_suffix_bytes = REMOVE_EVERYTHING

        len1 = len(target) - max_subsequence_length
        return (
            bytes(
                [
                    (truncate_prefix_bytes + ord("A")) & 0xFF,
                    (truncate_suffix_bytes + ord("A")) & 0xFF,
                ]
            )
            + target[max_subsequence_length : max_subsequence_length + len1]
        )

    def decode(self, source: bytes, encoded: bytes) -> bytes:
        truncate_prefix_bytes = (encoded[0] - ord("A")) & 0xFF
        truncate_suffix_bytes = (encoded[1] - ord("A")) & 0xFF
        if truncate_prefix_bytes == REMOVE_EVERYTHING or truncate_suffix_bytes == REMOVE_EVERYTHING:
            truncate_prefix_bytes = len(source)
            truncate_suffix_bytes = 0
        len1 = len(source) - (truncate_suffix_bytes + truncate_prefix_bytes)
        return source[truncate_prefix_bytes : truncate_prefix_bytes + len1] + encoded[2:]


class TrimInfixAndSuffixEncoder(ISequenceEncoder):
    """``{X}{L}{K}{suffix}``: remove an infix and suffix of ``source``, append suffix."""

    prefix_bytes = 3

    def encode(self, source: bytes, target: bytes) -> bytes:
        max_infix_index = 0
        max_subsequence_length = _common_prefix(source, target)
        max_infix_length = 0
        for i in (0, max_subsequence_length):
            for j in range(1, len(source) - i + 1):
                scratch = source[:i] + source[i + j :]
                shared_prefix = _common_prefix(scratch, target)
                if (
                    shared_prefix > 0
                    and shared_prefix > max_subsequence_length
                    and i < REMOVE_EVERYTHING
                    and j < REMOVE_EVERYTHING
                ):
                    max_subsequence_length = shared_prefix
                    max_infix_index = i
                    max_infix_length = j

        truncate_suffix_bytes = len(source) - (max_infix_length + max_subsequence_length)
        if truncate_suffix_bytes == 0 and max_infix_index + max_infix_length == len(source):
            truncate_suffix_bytes = max_infix_length
            max_infix_index = max_infix_length = 0

        if (
            max_infix_index >= REMOVE_EVERYTHING
            or max_infix_length >= REMOVE_EVERYTHING
            or truncate_suffix_bytes >= REMOVE_EVERYTHING
        ):
            max_infix_index = max_subsequence_length = 0
            max_infix_length = truncate_suffix_bytes = REMOVE_EVERYTHING

        len1 = len(target) - max_subsequence_length
        return (
            bytes(
                [
                    (max_infix_index + ord("A")) & 0xFF,
                    (max_infix_length + ord("A")) & 0xFF,
                    (truncate_suffix_bytes + ord("A")) & 0xFF,
                ]
            )
            + target[max_subsequence_length : max_subsequence_length + len1]
        )

    def decode(self, source: bytes, encoded: bytes) -> bytes:
        infix_index = (encoded[0] - ord("A")) & 0xFF
        infix_length = (encoded[1] - ord("A")) & 0xFF
        truncate_suffix_bytes = (encoded[2] - ord("A")) & 0xFF
        if infix_length == REMOVE_EVERYTHING or truncate_suffix_bytes == REMOVE_EVERYTHING:
            infix_index = 0
            infix_length = len(source)
            truncate_suffix_bytes = 0
        len1 = len(source) - (infix_index + infix_length + truncate_suffix_bytes)
        start = infix_index + infix_length
        return source[:infix_index] + source[start : start + len1] + encoded[3:]


ENCODERS: Dict[str, Type[ISequenceEncoder]] = {
    "SUFFIX": TrimSuffixEncoder,
    "PREFIX": TrimPrefixAndSuffixEncoder,
    "INFIX": TrimInfixAndSuffixEncoder,
    "NONE": NoEncoder,
}


def get_encoder(name: str) -> ISequenceEncoder:
    try:
        return ENCODERS[name.strip().upper()]()
    except KeyError:
        raise ValueError(
            f"Invalid encoder name '{name}', only these coders are valid: "
            f"{sorted(ENCODERS)}"
        ) from None
