"""Morfologik dictionary metadata (``*.info``) parsing.

Port of ``morfologik.stemming.DictionaryMetadata`` and
``DictionaryAttribute`` (BSD-3-Clause,
https://github.com/morfologik/morfologik-stemming).
"""

from __future__ import annotations

import os
from typing import Dict, List, Optional

from .encoders import ISequenceEncoder, get_encoder

METADATA_FILE_EXTENSION = "info"

SEPARATOR = "fsa.dict.separator"
ENCODING = "fsa.dict.encoding"
ENCODER = "fsa.dict.encoder"
FREQUENCY_INCLUDED = "fsa.dict.frequency-included"
IGNORE_NUMBERS = "fsa.dict.speller.ignore-numbers"
IGNORE_PUNCTUATION = "fsa.dict.speller.ignore-punctuation"
IGNORE_CAMEL_CASE = "fsa.dict.speller.ignore-camel-case"
IGNORE_ALL_UPPERCASE = "fsa.dict.speller.ignore-all-uppercase"
IGNORE_DIACRITICS = "fsa.dict.speller.ignore-diacritics"
CONVERT_CASE = "fsa.dict.speller.convert-case"
RUN_ON_WORDS = "fsa.dict.speller.runon-words"
LOCALE = "fsa.dict.speller.locale"
INPUT_CONVERSION = "fsa.dict.input-conversion"
OUTPUT_CONVERSION = "fsa.dict.output-conversion"
REPLACEMENT_PAIRS = "fsa.dict.speller.replacement-pairs"
EQUIVALENT_CHARS = "fsa.dict.speller.equivalent-chars"
LICENSE = "fsa.dict.license"
AUTHOR = "fsa.dict.author"
CREATION_DATE = "fsa.dict.created"

KNOWN_ATTRIBUTES = {
    SEPARATOR,
    ENCODING,
    ENCODER,
    FREQUENCY_INCLUDED,
    IGNORE_NUMBERS,
    IGNORE_PUNCTUATION,
    IGNORE_CAMEL_CASE,
    IGNORE_ALL_UPPERCASE,
    IGNORE_DIACRITICS,
    CONVERT_CASE,
    RUN_ON_WORDS,
    LOCALE,
    INPUT_CONVERSION,
    OUTPUT_CONVERSION,
    REPLACEMENT_PAIRS,
    EQUIVALENT_CHARS,
    LICENSE,
    AUTHOR,
    CREATION_DATE,
}

DEFAULT_BOOL_ATTRIBUTES = {
    FREQUENCY_INCLUDED: False,
    IGNORE_PUNCTUATION: True,
    IGNORE_NUMBERS: True,
    IGNORE_CAMEL_CASE: True,
    IGNORE_ALL_UPPERCASE: True,
    IGNORE_DIACRITICS: True,
    CONVERT_CASE: True,
    RUN_ON_WORDS: True,
}

REQUIRED_ATTRIBUTES = (SEPARATOR, ENCODER, ENCODING)


def parse_java_properties(text: str) -> Dict[str, str]:
    """Parse a Java ``.properties`` file (comments, escapes, continuations)."""
    props: Dict[str, str] = {}
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        i += 1
        stripped = line.lstrip()
        if not stripped or stripped[0] in "#!":
            continue
        # Handle line continuations (trailing odd number of backslashes).
        while _ends_with_continuation(line) and i < len(lines):
            line = line[:-1] + lines[i].lstrip()
            i += 1
        key, _, value = _split_property(line)
        props[key] = value
    return props


def _ends_with_continuation(line: str) -> bool:
    backslashes = 0
    for ch in reversed(line):
        if ch == "\\":
            backslashes += 1
        else:
            break
    return backslashes % 2 == 1


def _split_property(line: str) -> tuple[str, str, str]:
    length = len(line)
    key_end = 0
    value_start = length
    while key_end < length:
        ch = line[key_end]
        if ch in "=:" or ch.isspace():
            break
        key_end += 1
    key = _unescape(line[:key_end])
    pos = key_end
    while pos < length and line[pos].isspace():
        pos += 1
    if pos < length and line[pos] in "=:":
        pos += 1
        while pos < length and line[pos].isspace():
            pos += 1
        value_start = pos
    else:
        value_start = pos
    value = _unescape(line[value_start:])
    return key, "=", value


def _unescape(value: str) -> str:
    if "\\" not in value:
        return value
    out: List[str] = []
    i = 0
    while i < len(value):
        ch = value[i]
        if ch != "\\" or i + 1 >= len(value):
            out.append(ch)
            i += 1
            continue
        nxt = value[i + 1]
        if nxt == "u" and i + 5 < len(value):
            out.append(chr(int(value[i + 2 : i + 6], 16)))
            i += 6
        elif nxt == "t":
            out.append("\t")
            i += 2
        elif nxt == "n":
            out.append("\n")
            i += 2
        elif nxt == "r":
            out.append("\r")
            i += 2
        else:
            out.append(nxt)
            i += 2
    return "".join(out)


def boolean_value(value: str) -> bool:
    lowered = value.strip().lower()
    if lowered in ("true", "yes", "on"):
        return True
    if lowered in ("false", "no", "off"):
        return False
    raise ValueError(f"Not a boolean value: {value}")


class DictionaryMetadata:
    def __init__(self, attributes: Dict[str, str]) -> None:
        merged = dict(DEFAULT_BOOL_ATTRIBUTES)
        merged.update({k: v for k, v in attributes.items() if k in DEFAULT_BOOL_ATTRIBUTES})
        self.attributes = dict(attributes)

        missing = [key for key in REQUIRED_ATTRIBUTES if key not in attributes]
        if missing:
            raise ValueError(f"At least one of the required attributes was not provided: {missing}")

        self.encoding = attributes[ENCODING]
        self.separator_char = attributes[SEPARATOR]
        if len(self.separator_char) != 1:
            raise ValueError(f"Attribute {SEPARATOR} must be a single character.")
        try:
            separator_bytes = self.separator_char.encode(self.encoding)
        except LookupError as exc:
            raise ValueError(f"Encoding not supported: {self.encoding}") from exc
        if len(separator_bytes) != 1:
            raise ValueError(
                f"Separator character is not a single byte in encoding {self.encoding}: "
                f"{self.separator_char}"
            )
        self.separator = separator_bytes[0]
        self.encoder_type = get_encoder(attributes[ENCODER])
        self.bool_attributes = merged
        self.input_conversion = _parse_conversion(attributes.get(INPUT_CONVERSION))
        self.output_conversion = _parse_conversion(attributes.get(OUTPUT_CONVERSION))

    def get_encoder(self) -> ISequenceEncoder:
        return self.encoder_type

    def is_frequency_included(self) -> bool:
        return self.bool_attributes[FREQUENCY_INCLUDED]

    def is_ignoring_punctuation(self) -> bool:
        return self.bool_attributes[IGNORE_PUNCTUATION]

    def is_ignoring_numbers(self) -> bool:
        return self.bool_attributes[IGNORE_NUMBERS]

    def is_ignoring_camel_case(self) -> bool:
        return self.bool_attributes[IGNORE_CAMEL_CASE]

    def is_ignoring_all_uppercase(self) -> bool:
        return self.bool_attributes[IGNORE_ALL_UPPERCASE]

    def is_ignoring_diacritics(self) -> bool:
        return self.bool_attributes[IGNORE_DIACRITICS]

    def is_converting_case(self) -> bool:
        return self.bool_attributes[CONVERT_CASE]

    def is_supporting_run_on_words(self) -> bool:
        return self.bool_attributes[RUN_ON_WORDS]

    @classmethod
    def read_file(cls, path: str) -> "DictionaryMetadata":
        with open(path, encoding="utf-8") as handle:
            return cls(parse_java_properties(handle.read()))

    @classmethod
    def read_text(cls, text: str) -> "DictionaryMetadata":
        return cls(parse_java_properties(text))


def _parse_conversion(value: Optional[str]) -> Dict[str, str]:
    result: Dict[str, str] = {}
    if not value:
        return result
    for pair in value.split(","):
        parts = pair.strip().split(" ")
        if len(parts) == 2:
            result.setdefault(parts[0], parts[1])
    return result


def get_expected_metadata_file_name(dictionary_file: str) -> str:
    base, ext = os.path.splitext(dictionary_file)
    if ext:
        return base + "." + METADATA_FILE_EXTENSION
    return dictionary_file + "." + METADATA_FILE_EXTENSION


def get_expected_metadata_location(dictionary: str) -> str:
    directory, name = os.path.split(dictionary)
    return os.path.join(directory, get_expected_metadata_file_name(name))
