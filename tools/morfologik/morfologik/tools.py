"""Morfologik command-line tools: compile/decompile FSA and dictionaries.

Port of ``morfologik.tools.FSACompile``/``DictCompile``/``FSADecompile``/
``DictDecompile`` plus the shared ``BinaryInput`` reader (BSD-3-Clause,
https://github.com/morfologik/morfologik-stemming).
"""

from __future__ import annotations

import os
from typing import List, Optional

from .builder import FSABuilder
from .dictionary import Dictionary, DictionaryIterator
from .fsa import read_automaton
from .metadata import (
    DictionaryMetadata,
    get_expected_metadata_location,
)
from .serializer import CFSA2Serializer


class MorfologikError(Exception):
    """Raised when input data is invalid."""


def read_binary_sequences(
    path: str,
    separator: bytes = b"\n",
    accept_bom: bool = False,
    accept_cr: bool = False,
    ignore_empty: bool = False,
) -> List[bytes]:
    """Read a byte-separated sequence file (``BinaryInput.readBinarySequences``)."""
    with open(path, "rb") as handle:
        data = handle.read()

    if not accept_bom and data.startswith(b"\xef\xbb\xbf"):
        raise MorfologikError(
            "The input starts with UTF-8 BOM bytes which is most likely not what you want. "
            "Use a header-less UTF-8 file or pass --accept-bom."
        )

    sequences: List[bytes] = []
    # Split on the separator byte; a trailing separator does not create a final empty line.
    if not data:
        return sequences
    parts = data.split(separator)
    if data.endswith(separator):
        parts = parts[:-1]
    for part in parts:
        if not accept_cr and b"\r" in part:
            raise MorfologikError(
                "The input contains a \\r byte (CR) which would be encoded as part of the "
                "automaton. Pass --accept-cr if this is desired."
            )
        if len(part) == 0:
            if not ignore_empty:
                raise MorfologikError(
                    "The input contains empty sequences. Pass --ignore-empty to ignore them."
                )
            continue
        sequences.append(part)
    return sequences


def _serialize(sequences: List[bytes]) -> bytes:
    sequences = sorted(sequences)
    fsa = FSABuilder.build(sequences)
    out = bytearray()
    CFSA2Serializer().serialize(fsa, out)
    return bytes(out)


def fsa_compile(
    input_path: str,
    output_path: str,
    accept_bom: bool = False,
    accept_cr: bool = False,
    ignore_empty: bool = False,
) -> None:
    sequences = read_binary_sequences(
        input_path,
        accept_bom=accept_bom,
        accept_cr=accept_cr,
        ignore_empty=ignore_empty,
    )
    data = _serialize(sequences)
    with open(output_path, "wb") as handle:
        handle.write(data)


def dict_compile(
    input_path: str,
    output_path: Optional[str] = None,
    overwrite: bool = False,
    validate: bool = True,
    accept_bom: bool = False,
    accept_cr: bool = False,
    ignore_empty: bool = False,
) -> str:
    metadata_path = get_expected_metadata_location(input_path)
    if not os.path.isfile(metadata_path):
        raise MorfologikError(
            f"Dictionary metadata file for the input does not exist: {metadata_path}"
        )
    metadata = DictionaryMetadata.read_file(metadata_path)

    if output_path is None:
        base, _ext = os.path.splitext(metadata_path)
        output_path = base + ".dict"

    if not overwrite and os.path.exists(output_path):
        raise MorfologikError(
            f"Output dictionary file already exists: {output_path}, pass --overwrite to override."
        )

    sequences = read_binary_sequences(
        input_path,
        accept_bom=accept_bom,
        accept_cr=accept_cr,
        ignore_empty=ignore_empty,
    )

    separator = metadata.separator
    encoder = metadata.get_encoder()

    if sequences:
        separator_count = sequences[0].count(separator)
        if separator_count < 1 or separator_count > 2:
            raise MorfologikError(
                "Invalid input. Each row must consist of [base,inflected,tag?] columns, "
                f"separated by the declared separator. This row contains {separator_count} "
                f"separator characters."
            )
        for row in sequences[1:]:
            if row.count(separator) != separator_count:
                raise MorfologikError(
                    "The number of separators is inconsistent with previous lines."
                )

    assembled_sequences: List[bytes] = []
    sep_byte = bytes([separator])
    for row in sequences:
        sep1 = row.find(separator)
        sep2 = row.find(separator, sep1 + 1)
        if sep2 < 0:
            sep2 = len(row)

        base = row[:sep1]
        inflected = row[sep1 + 1 : sep2]
        tag = row[sep2 + 1 :] if sep2 < len(row) else b""

        # Morfologik encodes the inflected form relative to its base form:
        # `source` is the inflected word, `target` is the base/lemma.
        encoded = encoder.encode(inflected, base)
        assembled = inflected + sep_byte + encoded
        if tag:
            assembled += sep_byte + tag
        assembled_sequences.append(assembled)

    data = _serialize(assembled_sequences)
    with open(output_path, "wb") as handle:
        handle.write(data)

    if validate:
        dictionary = Dictionary(read_automaton(data), metadata)
        for _entry in DictionaryIterator(dictionary):
            pass

    return output_path


def fsa_decompile(input_path: str, output_path: str) -> None:
    with open(input_path, "rb") as handle:
        fsa = read_automaton(handle.read())
    with open(output_path, "wb") as handle:
        for sequence in fsa.iter_sequences():
            handle.write(sequence)
            handle.write(b"\n")


def dict_decompile(
    input_path: str,
    output_path: Optional[str] = None,
    overwrite: bool = False,
    validate: bool = True,
) -> str:
    dictionary = Dictionary.read(input_path)
    separator = dictionary.metadata.separator
    sep_byte = bytes([separator])

    if output_path is None:
        base, ext = os.path.splitext(input_path)
        output_path = base + ".input" if ext else input_path + ".input"
        if os.path.exists(output_path) and not overwrite:
            raise MorfologikError(
                f"The default output file location already exists: {output_path}. "
                "Pass --overwrite or remove the file."
            )

    entries = list(DictionaryIterator(dictionary))
    has_tags = any(tag for _stem, _word, tag in entries)

    with open(output_path, "wb") as handle:
        for stem, word, tag in entries:
            handle.write(stem)
            handle.write(sep_byte)
            handle.write(word)
            if has_tags:
                handle.write(sep_byte)
                handle.write(tag)
            handle.write(b"\n")

            if validate and (separator in stem or separator in word):
                raise MorfologikError(
                    "The stem or word of a dictionary entry contains the separator byte, "
                    "which prevents proper re-encoding."
                )

    return output_path
