"""LanguageTool dictionary builders and exporter.

Port of the Java ``languagetool-tools`` module
(https://github.com/languagetool-org/languagetool, LGPL-2.1-or-later):

* ``POSDictionaryBuilder``   -- tagger dictionary (wordform/lemma/postag).
* ``SpellDictionaryBuilder`` -- speller FSA (word list, optional frequencies).
* ``SynthDictionaryBuilder`` -- synthesizer dictionary (wordform/lemma/postag).
* ``DictionaryExporter``     -- binary dictionary back to tab-separated text.

These wrap the Morfologik compile/decompile tools in :mod:`morfologik.tools`,
just like the Java classes wrap ``morfologik.tools.*``.
"""

from __future__ import annotations

import math
import os
import re
import tempfile
from typing import Dict, Optional

from .metadata import DictionaryMetadata, get_expected_metadata_location
from .tools import (
    MorfologikError,
    dict_compile,
    dict_decompile,
    fsa_compile,
    fsa_decompile,
)

FREQ_RANGES_IN = 256
FREQ_RANGES_OUT = 26
FIRST_RANGE_CODE = ord("A")

_FREQ_ENTRY = re.compile(r'.*<w f="(\d+)"(?: flags="(.*?)")?>(.+)</w>.*')
_TAGGER_ENTRY = re.compile(r"^([^\t]+).*$")
_POLISH_IGNORE_REGEX = re.compile(":neg|qub|depr")


def read_freq_list(freq_list_file: str) -> Dict[str, int]:
    freq_list: Dict[str, int] = {}
    with open(freq_list_file, encoding="utf-8") as handle:
        for line in handle:
            match = _FREQ_ENTRY.match(line.rstrip("\n"))
            if match:
                freq_list[match.group(3)] = int(match.group(1))
    return freq_list


def add_freq_data(
    dict_file: str, freq_list: Dict[str, int], metadata: DictionaryMetadata, use_separator: bool
) -> str:
    if not metadata.is_frequency_included():
        raise MorfologikError(
            "In order to use frequency data add the line "
            "'fsa.dict.frequency-included=true' to the dictionary info file."
        )
    separator = metadata.separator_char
    encoding = metadata.encoding
    max_freq = max(freq_list.values()) if freq_list else 0
    max_freq_log = math.log(max_freq) if max_freq > 0 else 0.0
    freq_values_applied = 0

    with tempfile.NamedTemporaryFile(
        "w", suffix="WithFrequencies.txt", delete=False, encoding=encoding, newline="\n"
    ) as out:
        temp_name = out.name
        with open(dict_file, encoding=encoding) as handle:
            for line in handle:
                line = line.rstrip("\n")
                match = _TAGGER_ENTRY.match(line)
                if not match:
                    continue
                key = match.group(1)
                freq = freq_list.get(key, 0)
                if key in freq_list:
                    freq_values_applied += 1
                normalized = freq
                if freq > 0 and max_freq > 255:
                    normalized = int(math.log(freq) / max_freq_log * (FREQ_RANGES_IN - 1))
                if normalized < 0 or normalized > 255:
                    raise MorfologikError(
                        f"Frequency out of range (0-255): {normalized} in word {key}"
                    )
                freq_char = chr(FIRST_RANGE_CODE + normalized * FREQ_RANGES_OUT // FREQ_RANGES_IN)
                if use_separator:
                    out.write(line + separator + freq_char + "\n")
                else:
                    out.write(line + freq_char + "\n")
    return temp_name


def convert_tab_to_separator(input_file: str, metadata: DictionaryMetadata) -> str:
    separator = metadata.separator_char
    encoding = metadata.encoding
    with tempfile.NamedTemporaryFile(
        "w", suffix="_separator.txt", delete=False, encoding=encoding, newline="\n"
    ) as out:
        temp_name = out.name
        with open(input_file, encoding=encoding) as handle:
            for line in handle:
                line = line.rstrip("\n")
                parts = line.split("\t")
                if len(parts) == 3:
                    out.write(parts[1] + separator + parts[0] + separator + parts[2] + "\n")
    return temp_name


def _write_info_for(input_path: str, info_path: str) -> None:
    target = get_expected_metadata_location(input_path)
    with open(info_path, "rb") as src, open(target, "wb") as dst:
        dst.write(src.read())


def _build_dict(input_file: str, info_path: str, output_path: str, validate: bool = True) -> None:
    _write_info_for(input_file, info_path)
    dict_compile(input_file, output_path=output_path, overwrite=True, validate=validate)


# -- POS tagger dictionary -------------------------------------------------


def pos_dictionary_builder(
    input_file: str,
    info_path: str,
    output_path: str,
    freq_file: Optional[str] = None,
) -> None:
    """Build a tagger dictionary from tab-separated ``wordform/lemma/postag``."""
    metadata = DictionaryMetadata.read_file(info_path)
    work_file = input_file
    if freq_file:
        freq_list = read_freq_list(freq_file)
        work_file = add_freq_data(work_file, freq_list, metadata, use_separator=False)
    converted = convert_tab_to_separator(work_file, metadata)
    _build_dict(converted, info_path, output_path)


# -- speller dictionary ----------------------------------------------------


def _tokenize_input(plain_text_dict_file: str, metadata: DictionaryMetadata) -> str:
    encoding = metadata.encoding
    separator_char = metadata.separator_char
    with tempfile.NamedTemporaryFile(
        "w", suffix=".txt", delete=False, encoding=encoding, newline="\n"
    ) as out:
        temp_name = out.name
        with open(plain_text_dict_file, encoding=encoding) as handle:
            for line in handle:
                line = line.rstrip("\n")
                sep_pos = line.find(separator_char)
                occurrences = line[sep_pos + len(separator_char) :] if sep_pos != -1 else ""
                line_without_occ = line[:sep_pos] if sep_pos != -1 else line
                if len(line_without_occ) > 0:
                    out.write(line_without_occ)
                    if sep_pos != -1:
                        out.write(separator_char)
                        out.write(occurrences)
                    out.write("\n")
    return temp_name


def spell_dictionary_builder(
    input_file: str,
    info_path: str,
    output_path: str,
    freq_file: Optional[str] = None,
) -> None:
    """Build a speller FSA from a plain word list (e.g. from Hunspell ``unmunch``)."""
    metadata = DictionaryMetadata.read_file(info_path)
    work_file = input_file
    if freq_file:
        freq_list = read_freq_list(freq_file)
        work_file = add_freq_data(work_file, freq_list, metadata, use_separator=True)
    tokenized = _tokenize_input(work_file, metadata)
    fsa_compile(tokenized, output_path, ignore_empty=True)


# -- synthesizer dictionary ------------------------------------------------


def _get_ignore_items(info_path: str, metadata: DictionaryMetadata) -> set:
    path = os.path.join(os.path.dirname(info_path), "filter-archaic.txt")
    if not os.path.exists(path):
        return set()
    result = set()
    with open(path, encoding=metadata.encoding) as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line.startswith("#"):
                result.add(line)
    return result


def _get_pos_tag_ignore_regex(info_path: str):
    file_name = os.path.basename(info_path)
    underscore_pos = file_name.find("_")
    if underscore_pos == -1:
        raise MorfologikError(
            "Please specify an .info file for a synthesizer as the second parameter, named "
            "'<xyz>_synth.info', with <xyz> being a language"
        )
    base_name = file_name[:underscore_pos]
    if base_name == "polish":
        return _POLISH_IGNORE_REGEX
    return None


def _reverse_line_content(
    plain_text_dict_file: str,
    items_to_be_ignored: set,
    ignore_pos_regex,
    metadata: DictionaryMetadata,
) -> str:
    separator = metadata.separator_char
    encoding = metadata.encoding
    with tempfile.NamedTemporaryFile(
        "w", suffix="_reversed.txt", delete=False, encoding=encoding, newline="\n"
    ) as out:
        temp_name = out.name
        with open(plain_text_dict_file, encoding=encoding) as handle:
            for line in handle:
                line = line.rstrip("\n")
                if line in items_to_be_ignored:
                    continue
                parts = line.split("\t")
                if len(parts) == 3:
                    pos_tag = parts[2]
                    if ignore_pos_regex is not None and ignore_pos_regex.search(pos_tag):
                        continue
                    out.write(parts[0] + separator + parts[1] + "|" + pos_tag + "\n")
    return temp_name


def _write_pos_tags_to_file(plain_text_dict_file: str, tag_file: str, metadata: DictionaryMetadata) -> None:
    tags = set()
    with open(plain_text_dict_file, encoding=metadata.encoding) as handle:
        for line in handle:
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 3:
                tags.add(parts[2])
    with open(tag_file, "w", encoding="utf-8", newline="\n") as out:
        for tag in sorted(tags):
            out.write(tag + "\n")


def synth_dictionary_builder(input_file: str, info_path: str, output_path: str) -> None:
    """Build a synthesizer dictionary from tab-separated ``wordform/lemma/postag``."""
    metadata = DictionaryMetadata.read_file(info_path)
    items_to_be_ignored = _get_ignore_items(info_path, metadata)
    ignore_pos_regex = _get_pos_tag_ignore_regex(info_path)
    reversed_file = _reverse_line_content(
        input_file, items_to_be_ignored, ignore_pos_regex, metadata
    )
    _write_pos_tags_to_file(input_file, output_path + "_tags.txt", metadata)
    _build_dict(reversed_file, info_path, output_path)


# -- exporter --------------------------------------------------------------


def _output_separator_to_tab(input_file: str, output_file: str, metadata: DictionaryMetadata) -> None:
    separator = metadata.separator_char
    has_frequency = metadata.is_frequency_included()
    encoding = metadata.encoding
    with open(input_file, encoding=encoding) as handle, open(
        output_file, "w", encoding=encoding, newline="\n"
    ) as out:
        for line in handle:
            line = line.rstrip("\n")
            parts = line.split(separator)
            if len(parts) == 3:
                if has_frequency:
                    parts[2] = parts[2][:-1]
                out.write(parts[1] + "\t" + parts[0] + "\t" + parts[2] + "\n")
            elif len(parts) == 2:
                out.write(parts[1] + "\t" + parts[0] + "\n")
            elif len(parts) == 1:
                out.write(parts[0] + "\n")


def dictionary_exporter(input_file: str, info_path: str, output_path: str) -> None:
    """Print the contents of a binary Morfologik dictionary to tab-separated text."""
    metadata = DictionaryMetadata.read_file(info_path)
    with tempfile.NamedTemporaryFile("w", suffix="_separator.txt", delete=False) as tmp:
        tmp_path = tmp.name
    if "hunspell" in input_file or "spelling" in input_file:
        fsa_decompile(input_file, tmp_path)
    else:
        dict_decompile(input_file, tmp_path, overwrite=True, validate=False)
    _output_separator_to_tab(tmp_path, output_path, metadata)
    os.unlink(tmp_path)
