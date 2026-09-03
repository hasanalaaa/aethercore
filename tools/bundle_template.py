#!/usr/bin/env python3
"""Extract/repack the JSON-escaped app source embedded in a Stitch
"bundler" single-file HTML export (e.g. AetherCore.html).

The bundler packs the real page markup+JS as a JSON string literal inside
  <script type="__bundler/template"> "...escaped html..." </script>
with one twist: any "/" that immediately follows "<" is additionally escaped
as \\u002F (so a literal "</script" can never appear inside the outer
<script> tag and truncate it early). This tool preserves that convention
on repack and round-trips byte-identical when given back unmodified input.

Usage:
  python3 bundle_template.py extract <bundled.html> <out.html>
  python3 bundle_template.py repack  <bundled.html> <edited.html> -o <output.html>
"""
import argparse
import json
import sys

MARKER = '<script type="__bundler/template">'


def find_string_span(data, marker):
    idx = data.index(marker)
    i = idx + len(marker)
    while data[i] in "\n\r\t ":
        i += 1
    if data[i] != '"':
        raise ValueError(f"expected opening quote at offset {i}, found {data[i]!r}")
    dec = json.JSONDecoder()
    value, end = dec.raw_decode(data, i)
    return i, end, value


def extract(bundled_path, out_path):
    with open(bundled_path, "r", encoding="utf-8") as f:
        data = f.read()
    _, _, value = find_string_span(data, MARKER)
    with open(out_path, "w", encoding="utf-8") as f:
        f.write(value)
    print(f"extracted {len(value)} chars -> {out_path}")


def repack(bundled_path, edited_path, out_path):
    with open(bundled_path, "r", encoding="utf-8") as f:
        data = f.read()
    start, end, _ = find_string_span(data, MARKER)

    with open(edited_path, "r", encoding="utf-8") as f:
        new_value = f.read()

    dumped = json.dumps(new_value, ensure_ascii=False)
    dumped = dumped.replace("</", "<\\u002F")  # match the bundler's escaping convention

    new_data = data[:start] + dumped + data[end:]

    with open(out_path, "w", encoding="utf-8") as f:
        f.write(new_data)
    print(f"repacked -> {out_path} ({len(new_data)} bytes, was {len(data)})")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p1 = sub.add_parser("extract", help="pull the decoded template out of a bundled HTML file")
    p1.add_argument("bundled")
    p1.add_argument("out")

    p2 = sub.add_parser("repack", help="write an edited template back into a bundled HTML file")
    p2.add_argument("bundled")
    p2.add_argument("edited")
    p2.add_argument("-o", "--output", required=True)

    args = ap.parse_args()
    if args.cmd == "extract":
        extract(args.bundled, args.out)
    elif args.cmd == "repack":
        repack(args.bundled, args.edited, args.output)


if __name__ == "__main__":
    main()
