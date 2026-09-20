"""Self-contained tests for the pure-Python morfologik/LanguageTool tools.

Run with::

    python3 -m unittest discover -s tools/morfologik/tests

The golden blobs were produced by the Java tooling
(``morfologik-tools``/``languagetool-tools`` 2.2.0/6.6) and pin byte-for-byte
compatibility without requiring a JVM at test time.
"""

import base64
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from morfologik import lt, tools  # noqa: E402
from morfologik.builder import FSABuilder  # noqa: E402
from morfologik.encoders import (  # noqa: E402
    NoEncoder,
    TrimInfixAndSuffixEncoder,
    TrimPrefixAndSuffixEncoder,
    TrimSuffixEncoder,
)
from morfologik.fsa import read_automaton  # noqa: E402
from morfologik.metadata import DictionaryMetadata  # noqa: E402
from morfologik.serializer import CFSA2Serializer  # noqa: E402

SMALL_WORDS = [b"apple", b"apply", b"banana", b"band", b"bandana", b"cat"]

GOLDEN_CFSA2 = "XGZzYcYABwsAeXRsZWRjYnBuYUBeAwoUBwvGymIAyskKEeXKyWoAyMjDJABhAA=="
GOLDEN_FSA5 = "XGZzYQVfKwEAAF4GYeBicGMGYQZ0A2EGbgZhwGQHYQZuBmEDcAZwBmwGZQF5Aw=="
GOLDEN_POS = "XGZzYcYABxMAdXRyb2dkY1ZTRENBc25hQk4rQF4DBx0GE8PPztLLwc7SyNBqAMTFzdLQ0tHRaQDPwpJNFszS0XEA"
GOLDEN_SYNTH = "XGZzYcYABxQAdXRzcm9nZGNWR0VCbmFTRHwrTkBeAwgdBxPEwc3RyczQ0srObQDFxtHT08/Sy2MAzsLR09OSTxlwAA=="
GOLDEN_SYNTH_TAGS = "Tk4KTk5TClZCRAo="
GOLDEN_SPELL = "XGZzYcYABwkAdGxlY2JwbmFAXgMIEQULxMhhAMjHyMdoAMbGwmMA"
GOLDEN_POS_FREQ = "XGZzYcYABxQAdXRyb2dkY1ZTSURDc25hQkFOK0BeAwceBhTDz87TzMHO08jQy3EAxMXN09DT0tJJEs/Ck00X0dPS0moA"

POS_INFO = "fsa.dict.separator=+\nfsa.dict.encoding=utf-8\nfsa.dict.encoder=SUFFIX\n"
FREQ_INFO = POS_INFO + "fsa.dict.frequency-included=true\n"
POS_INPUT = "cats\tcat\tNNS\ndogs\tdog\tNNS\ncat\tcat\tNN\nran\trun\tVBD\n"
FREQ_XML = '<w f="1000">apple</w>\n<w f="500">banana</w>\n<w f="10">cat</w>\n'


def _b64(data: str) -> bytes:
    return base64.b64decode(data)


class GoldenTest(unittest.TestCase):
    def test_fsa_compile_golden(self):
        fsa = FSABuilder.build(sorted(SMALL_WORDS))
        out = bytearray()
        CFSA2Serializer().serialize(fsa, out)
        self.assertEqual(bytes(out), _b64(GOLDEN_CFSA2))

    def test_fsa5_read_golden(self):
        fsa = read_automaton(_b64(GOLDEN_FSA5))
        self.assertEqual(set(fsa.iter_sequences()), set(SMALL_WORDS))

    def test_cfsa2_roundtrip(self):
        fsa = FSABuilder.build(sorted(SMALL_WORDS))
        out = bytearray()
        CFSA2Serializer().serialize(fsa, out)
        self.assertEqual(set(read_automaton(bytes(out)).iter_sequences()), set(SMALL_WORDS))


class EncoderTest(unittest.TestCase):
    def test_encoder_roundtrip(self):
        pairs = [
            (b"foo", b"foobar"),
            (b"foo", b"bar"),
            (b"abc", b"abcd"),
            (b"abc", b"xyz"),
            (b"ayz", b"abc"),
            (b"aillent", b"aller"),
            (b"cats", b"cat"),
            (b"", b"word"),
            (b"word", b""),
            (b"x" * 300, b"y" * 10),
        ]
        for encoder in (
            NoEncoder(),
            TrimSuffixEncoder(),
            TrimPrefixAndSuffixEncoder(),
            TrimInfixAndSuffixEncoder(),
        ):
            for source, target in pairs:
                encoded = encoder.encode(source, target)
                self.assertEqual(encoder.decode(source, encoded), target, (encoder, source, target))


class MetadataTest(unittest.TestCase):
    def test_parse(self):
        metadata = DictionaryMetadata.read_text(POS_INFO)
        self.assertEqual(metadata.separator_char, "+")
        self.assertEqual(metadata.separator, ord("+"))
        self.assertEqual(metadata.encoding, "utf-8")
        self.assertFalse(metadata.is_frequency_included())
        self.assertTrue(metadata.is_supporting_run_on_words())

    def test_java_properties_escapes(self):
        text = "# comment\nfsa.dict.separator=+\nfsa.dict.encoding=utf\\-8\nfsa.dict.encoder=SUFFIX\n"
        metadata = DictionaryMetadata.read_text(text)
        self.assertEqual(metadata.encoding, "utf-8")


class BuilderTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = self.tmp.name
        self.pos = self._write("pos.txt", POS_INPUT)
        self.info = self._write("pos.info", POS_INFO)
        self.synth_info = self._write("pos_synth.info", POS_INFO)
        self.freq_info = self._write("freq.info", FREQ_INFO)
        self.freq_xml = self._write("freq.xml", FREQ_XML)

    def tearDown(self):
        self.tmp.cleanup()

    def _write(self, name, text, encoding="utf-8"):
        path = os.path.join(self.dir, name)
        with open(path, "w", encoding=encoding, newline="\n") as handle:
            handle.write(text)
        return path

    def _read_bytes(self, path):
        with open(path, "rb") as handle:
            return handle.read()

    def test_pos_builder_golden(self):
        out = os.path.join(self.dir, "pos.dict")
        lt.pos_dictionary_builder(self.pos, self.info, out)
        self.assertEqual(self._read_bytes(out), _b64(GOLDEN_POS))

    def test_pos_builder_freq_golden(self):
        out = os.path.join(self.dir, "posfreq.dict")
        lt.pos_dictionary_builder(self.pos, self.freq_info, out, freq_file=self.freq_xml)
        self.assertEqual(self._read_bytes(out), _b64(GOLDEN_POS_FREQ))

    def test_synth_builder_golden(self):
        out = os.path.join(self.dir, "synth.dict")
        lt.synth_dictionary_builder(self.pos, self.synth_info, out)
        self.assertEqual(self._read_bytes(out), _b64(GOLDEN_SYNTH))
        self.assertEqual(self._read_bytes(out + "_tags.txt"), _b64(GOLDEN_SYNTH_TAGS))

    def test_spell_builder_golden(self):
        spell = self._write("spell.txt", "apple\nbanana\ncat\n")
        out = os.path.join(self.dir, "spell.dict")
        lt.spell_dictionary_builder(spell, self.info, out)
        self.assertEqual(self._read_bytes(out), _b64(GOLDEN_SPELL))

    def test_dict_decompile_roundtrip(self):
        out = os.path.join(self.dir, "pos.dict")
        lt.pos_dictionary_builder(self.pos, self.info, out)
        decompiled = tools.dict_decompile(out, output_path=os.path.join(self.dir, "pos.out"))
        with open(decompiled, encoding="utf-8") as handle:
            lines = sorted(handle.read().splitlines())
        expected = ["cat+cat+NN", "cat+cats+NNS", "dog+dogs+NNS", "run+ran+VBD"]
        self.assertEqual(lines, sorted(expected))

    def test_fsa_decompile(self):
        fsa_path = os.path.join(self.dir, "small.dict")
        small = self._write("small.txt", "\n".join(w.decode() for w in SMALL_WORDS) + "\n")
        tools.fsa_compile(small, fsa_path, ignore_empty=True)
        out = os.path.join(self.dir, "small.out")
        tools.fsa_decompile(fsa_path, out)
        self.assertEqual(sorted(self._read_bytes(out).splitlines()), sorted(SMALL_WORDS))

    def test_exporter(self):
        out = os.path.join(self.dir, "pos.dict")
        lt.pos_dictionary_builder(self.pos, self.info, out)
        exported = os.path.join(self.dir, "pos.export")
        lt.dictionary_exporter(out, self.info, exported)
        with open(exported, encoding="utf-8") as handle:
            lines = sorted(handle.read().splitlines())
        expected = ["cat\tcat\tNN", "cats\tcat\tNNS", "dogs\tdog\tNNS", "ran\trun\tVBD"]
        self.assertEqual(lines, sorted(expected))


if __name__ == "__main__":
    unittest.main()
