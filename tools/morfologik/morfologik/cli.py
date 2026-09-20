"""Command-line interface mirroring the LanguageTool/Morfologik dictionary tools."""

from __future__ import annotations

import argparse
import sys

from . import lt, tools


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="lt-morfologik",
        description=(
            "Pure-Python port of the LanguageTool (languagetool-tools) and Morfologik "
            "dictionary builders. Output is byte-for-byte identical to the Java tooling."
        ),
    )
    sub = parser.add_subparsers(dest="command", required=True)

    def add_common(p, input_required=True):
        p.add_argument("-i", "--input", required=input_required, help="input file")

    p = sub.add_parser("fsa_compile", help="compile a CFSA2 automaton from newline-delimited input")
    add_common(p)
    p.add_argument("-o", "--output", required=True, help="output automaton (.dict)")
    p.add_argument("--accept-bom", action="store_true")
    p.add_argument("--accept-cr", action="store_true")
    p.add_argument("--ignore-empty", action="store_true")
    p.set_defaults(func=_fsa_compile)

    p = sub.add_parser("dict_compile", help="compile a morphological dictionary")
    add_common(p)
    p.add_argument("-o", "--output", help="output dictionary (default: input with .info->.dict)")
    p.add_argument("--overwrite", action="store_true")
    p.add_argument("--no-validate", action="store_true")
    p.add_argument("--accept-bom", action="store_true")
    p.add_argument("--accept-cr", action="store_true")
    p.add_argument("--ignore-empty", action="store_true")
    p.set_defaults(func=_dict_compile)

    p = sub.add_parser("fsa_decompile", help="dump all sequences of an automaton")
    add_common(p)
    p.add_argument("-o", "--output", required=True)
    p.set_defaults(func=_fsa_decompile)

    p = sub.add_parser("dict_decompile", help="decompile a dictionary back to text")
    add_common(p)
    p.add_argument("-o", "--output")
    p.add_argument("--overwrite", action="store_true")
    p.add_argument("--no-validate", action="store_true")
    p.set_defaults(func=_dict_decompile)

    p = sub.add_parser("pos", help="LanguageTool POSDictionaryBuilder")
    add_common(p)
    p.add_argument("--info", required=True, help="*.info metadata file")
    p.add_argument("-o", "--output", required=True, help="output dictionary (.dict)")
    p.add_argument("--freq", help="optional frequency wordlist XML")
    p.set_defaults(func=_pos)

    p = sub.add_parser("spell", help="LanguageTool SpellDictionaryBuilder")
    add_common(p)
    p.add_argument("--info", required=True, help="*.info metadata file")
    p.add_argument("-o", "--output", required=True, help="output speller automaton")
    p.add_argument("--freq", help="optional frequency wordlist XML")
    p.set_defaults(func=_spell)

    p = sub.add_parser("synth", help="LanguageTool SynthDictionaryBuilder")
    add_common(p)
    p.add_argument("--info", required=True, help="*_synth.info metadata file")
    p.add_argument("-o", "--output", required=True, help="output synthesizer dictionary")
    p.set_defaults(func=_synth)

    p = sub.add_parser("export", help="LanguageTool DictionaryExporter")
    add_common(p)
    p.add_argument("--info", required=True, help="*.info metadata file")
    p.add_argument("-o", "--output", required=True, help="output tab-separated text")
    p.set_defaults(func=_export)

    return parser


def _fsa_compile(args):
    tools.fsa_compile(
        args.input,
        args.output,
        accept_bom=args.accept_bom,
        accept_cr=args.accept_cr,
        ignore_empty=args.ignore_empty,
    )


def _dict_compile(args):
    tools.dict_compile(
        args.input,
        output_path=args.output,
        overwrite=args.overwrite,
        validate=not args.no_validate,
        accept_bom=args.accept_bom,
        accept_cr=args.accept_cr,
        ignore_empty=args.ignore_empty,
    )


def _fsa_decompile(args):
    tools.fsa_decompile(args.input, args.output)


def _dict_decompile(args):
    tools.dict_decompile(
        args.input, output_path=args.output, overwrite=args.overwrite, validate=not args.no_validate
    )


def _pos(args):
    lt.pos_dictionary_builder(args.input, args.info, args.output, freq_file=args.freq)


def _spell(args):
    lt.spell_dictionary_builder(args.input, args.info, args.output, freq_file=args.freq)


def _synth(args):
    lt.synth_dictionary_builder(args.input, args.info, args.output)


def _export(args):
    lt.dictionary_exporter(args.input, args.info, args.output)


def main(argv=None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        args.func(args)
    except (tools.MorfologikError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
