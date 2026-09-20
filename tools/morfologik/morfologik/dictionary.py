"""Dictionary iteration for decompilation.

Port of the parts of ``morfologik.stemming.Dictionary``/``DictionaryIterator``
needed by the ``dict_decompile`` tool (BSD-3-Clause,
https://github.com/morfologik/morfologik-stemming).
"""

from __future__ import annotations

from typing import Iterator, Tuple

from .fsa import FSA, read_automaton
from .metadata import DictionaryMetadata, get_expected_metadata_location


class Dictionary:
    def __init__(self, fsa: FSA, metadata: DictionaryMetadata) -> None:
        self.fsa = fsa
        self.metadata = metadata

    @classmethod
    def read(cls, dict_path: str) -> "Dictionary":
        with open(dict_path, "rb") as handle:
            fsa = read_automaton(handle.read())
        metadata = DictionaryMetadata.read_file(get_expected_metadata_location(dict_path))
        return cls(fsa, metadata)


class DictionaryIterator:
    """Yield ``(stem, inflected, tag)`` byte tuples for every dictionary entry."""

    def __init__(self, dictionary: Dictionary) -> None:
        self.fsa = dictionary.fsa
        self.separator = dictionary.metadata.separator
        self.encoder = dictionary.metadata.get_encoder()

    def __iter__(self) -> Iterator[Tuple[bytes, bytes, bytes]]:
        prefix_bytes = self.encoder.prefix_bytes
        for entry in self.fsa.iter_sequences():
            sep_pos = entry.find(bytes([self.separator]))
            if sep_pos == -1:
                raise ValueError("Invalid dictionary entry format (missing separator).")
            inflected = entry[:sep_pos]
            temp = entry[sep_pos + 1 :]

            pos = prefix_bytes
            while pos < len(temp) and temp[pos] != self.separator:
                pos += 1
            encoded = temp[:pos]
            stem = self.encoder.decode(inflected, encoded)
            if pos + 1 <= len(temp):
                pos += 1
            tag = temp[pos:]
            yield stem, inflected, tag
