#!/usr/bin/env python3
"""Evaluate the source-marker assertions in the PowerShell gates, from any host.

Six steps of the windows job are PowerShell and none of them can run on the
development Mac, so every defect in them has cost one ~50-minute CI round trip to
find - and P63 spent four of those finding one defect each. A scanner that
replays what `Require-Marker` actually does found five more in seconds.

This is that scanner, kept this time. It learns each script's own assertion
helpers from their bodies rather than assuming one shape, because the shapes
differ and the difference is load-bearing:

* `Require-Marker $File $Pattern` is `$text -notmatch $Pattern` -> throw. `-match`
  is a .NET REGEX and is case-INSENSITIVE by default, which is why
  `'self.writer.shutdown();'` can never match the literal it was written to find:
  it parses as `shutdown` + an empty group + `;`. `DBT-P63-014`.
* `Require-Text $File @('a','b')` is `$text.Contains($needle)` - a LITERAL,
  case-SENSITIVE test over an ARRAY. P63's scanner could not read the array form,
  so `phase11-design-audit.ps1` was reported as unmeasured rather than passing.
  It is read here.

What it cannot evaluate it says so about, per assertion, and the exit code counts
those separately from failures. An assertion this tool skips is UNMEASURED, never
passing - that distinction is the whole reason the tool exists.

Not a replacement for running the gate on Windows: it evaluates source markers
only, not the `cargo`/`pnpm`/packaging steps those scripts also drive.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

WORKSPACE = Path(__file__).resolve().parents[1]


@dataclass
class Helper:
    """One script's own assertion function, as its signature and body define it."""

    name: str
    kind: str  # "require" | "reject" | "file"
    literal: bool  # .Contains (literal, case-sensitive) vs -match (regex, ci)
    over_tree: bool = False  # sweeps a directory set instead of taking a path
    roots: tuple[str, ...] = ()
    suffixes: tuple[str, ...] = ()


@dataclass
class Result:
    script: str
    helper: str
    path: str
    needle: str
    verdict: str  # PASS | FAIL | UNMEASURED
    detail: str = ""


@dataclass
class Scan:
    results: list[Result] = field(default_factory=list)
    skipped: list[str] = field(default_factory=list)


FUNC = re.compile(
    r"^function\s+([A-Za-z]+-[A-Za-z]+)\s*\((?P<params>[^)]*)\)\s*\{",
    re.M,
)


def _body(text: str, open_brace: int) -> str:
    depth, i = 0, open_brace
    while i < len(text):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[open_brace + 1 : i]
        i += 1
    return text[open_brace:]


PATH_PARAM = re.compile(r"\$(File|Relative|Path|Source)\b", re.I)
TREE = re.compile(r"Get-ChildItem\s+([\w,./-]+)\s+-Recurse")
EXTS = re.compile(r"-in\s+((?:'[^']*'\s*,?\s*)+)")


def helpers_of(text: str) -> dict[str, Helper]:
    """The assertion helpers this script defines, read from their bodies."""
    found: dict[str, Helper] = {}
    for m in FUNC.finditer(text):
        name = m.group(1)
        body = _body(text, m.end() - 1)
        if "throw" not in body:
            continue
        if ".Contains(" in body:
            helper = Helper(name, "require", literal=True)
        elif "-notmatch" in body:
            helper = Helper(name, "require", literal=False)
        elif "-match" in body:
            helper = Helper(name, "reject", literal=False)
        elif "Test-Path" in body:
            helper = Helper(name, "file", literal=False)
        else:
            continue
        # A helper whose first parameter is not a path sweeps a directory set:
        # `Reject-Tree $Pattern $Label` over `apps,services,crates`. Reading the
        # signature rather than assuming one is what lets both shapes be measured.
        params = [p.strip() for p in m.group("params").split(",") if p.strip()]
        first_is_path = bool(params) and PATH_PARAM.search(params[0])
        if not first_is_path:
            roots = TREE.search(body)
            exts = EXTS.search(body)
            if roots:
                helper.over_tree = True
                helper.roots = tuple(r for r in roots.group(1).split(",") if r)
                helper.suffixes = tuple(
                    e for e, _ in STRINGS.findall(exts.group(1))
                ) if exts else ()
            elif helper.kind != "file":
                continue
        found[name] = helper
    return found


# `'a'` or `"a"`, an `@('a','b')` array, or a bare `$var`.
ARG = re.compile(
    r"""\s*(?:
        @\(\s*(?P<array>(?:'[^']*'|"[^"]*"|\s|,|\r|\n)*?)\s*\)
      | '(?P<sq>(?:[^']|'')*)'
      | "(?P<dq>[^"]*)"
      | \((?P<paren>[^()]*(?:\([^()]*\)[^()]*)*)\)
      | (?P<bare>\$?[\w:.\\/-]+)
    )""",
    re.X,
)
STRINGS = re.compile(r"'([^']*)'|\"([^\"]*)\"")


def parse_args(rest: str) -> list[object] | None:
    """The call's arguments, as strings or lists of strings. None if unreadable."""
    out: list[object] = []
    pos = 0
    while pos < len(rest):
        if rest[pos] in "\r\n" or rest[pos : pos + 1] == "#":
            break
        m = ARG.match(rest, pos)
        if not m:
            break
        if m.group("array") is not None:
            out.append([(a or b).replace("''", "'")
                        for a, b in STRINGS.findall(m.group("array"))])
        elif m.group("sq") is not None:
            out.append(m.group("sq").replace("''", "'"))
        elif m.group("dq") is not None:
            out.append(m.group("dq"))
        elif m.group("paren") is not None:
            out.append(_join_path(m.group("paren")))
        else:
            out.append({"expr": m.group("bare")})
        pos = m.end()
        if pos < len(rest) and rest[pos] == ",":
            pos += 1
    return out or None


JOINPATH = re.compile(r"^\s*Join-Path\s+(?P<a>'[^']*'|\"[^\"]*\"|[\w./\\-]+)\s+(?P<b>'[^']*'|\"[^\"]*\"|[\w./\\-]+)\s*$")


def _join_path(expr: str) -> object:
    """`Join-Path 'a' 'b'` once its loop variable has been substituted."""
    m = JOINPATH.match(expr)
    if not m:
        return {"expr": expr}
    parts = [m.group("a").strip("'\""), m.group("b").strip("'\"")]
    return "/".join(p.strip("/") for p in parts)


FOREACH = re.compile(
    r"foreach\s*\(\s*\$(?P<var>\w+)\s+in\s+@\(\s*(?P<items>(?:'[^']*'|\s|,|\r|\n)*?)\s*\)\s*\)\s*\{",
    re.M,
)
PIPE_EACH = re.compile(
    r"@\(\s*(?P<items>(?:'[^']*'|\s|,|\r|\n)*?)\s*\)\s*\|\s*ForEach-Object\s*\{",
    re.M,
)


def expand_loops(text: str) -> tuple[str, list[str]]:
    """Unroll `foreach ($x in @(..)) {..}` and `@(..) | ForEach-Object {..}`.

    `DBT-P63-014` named these unmeasured. Unrolling them is what measures them:
    the loop body is emitted once per item with the loop variable substituted, so
    every call inside is an ordinary call by the time it is parsed.
    """
    skipped: list[str] = []
    for _ in range(8):  # nested loops; bounded so a pathological file terminates
        m = FOREACH.search(text) or PIPE_EACH.search(text)
        if not m:
            break
        var = m.groupdict().get("var") or "_"
        items = [a or b for a, b in STRINGS.findall(m.group("items"))]
        body = _body(text, m.end() - 1)
        if not items:
            skipped.append(f"loop with no literal items: {m.group(0)[:70]!r}")
            text = text[: m.start()] + text[m.start() + len(m.group(0)) + len(body) + 1 :]
            continue
        unrolled = "\n".join(
            body.replace("$" + var, item).replace("${" + var + "}", item)
            for item in items
        )
        end = m.end() - 1 + len(body) + 2
        text = text[: m.start()] + unrolled + text[end:]
    return text, skipped


CALL = re.compile(r"^[ \t]*([A-Za-z]+-[A-Za-z]+)[ \t]+(.*)$", re.M)


def scan_script(path: Path, workspace: Path) -> Scan:
    text = path.read_text(encoding="utf-8", errors="replace")
    helpers = helpers_of(text)
    scan = Scan()
    if not helpers:
        return scan
    # Drop the function definitions themselves so their bodies are not scanned as
    # calls, then unroll the loops that hide calls from a line-oriented reader.
    stripped = text
    for m in reversed(list(FUNC.finditer(text))):
        body = _body(text, m.end() - 1)
        end = m.end() - 1 + len(body) + 2
        stripped = stripped[: m.start()] + "\n" * text[m.start():end].count("\n") + stripped[end:]
    stripped, skipped = expand_loops(stripped)
    scan.skipped.extend(f"{path.name}: {s}" for s in skipped)

    for m in CALL.finditer(stripped):
        name, rest = m.group(1), m.group(2)
        helper = helpers.get(name)
        if helper is None:
            continue
        args = parse_args(rest)
        if helper.over_tree:
            if not args or isinstance(args[0], dict):
                scan.results.append(Result(path.name, name, ",".join(helper.roots), "",
                                           "UNMEASURED", "pattern is an expression"))
                continue
            pattern = args[0]
            hits = []
            for root in helper.roots:
                base = workspace / root
                if not base.is_dir():
                    continue
                for f in sorted(base.rglob("*")):
                    if not f.is_file():
                        continue
                    if helper.suffixes and f.suffix not in helper.suffixes:
                        continue
                    body = f.read_text(encoding="utf-8", errors="replace")
                    found = (pattern in body) if helper.literal else bool(
                        re.search(pattern, body, re.IGNORECASE | re.MULTILINE))
                    if found:
                        hits.append(str(f.relative_to(workspace)))
            want = helper.kind == "require"
            ok = bool(hits) if want else not hits
            scan.results.append(Result(path.name, name, ",".join(helper.roots), pattern,
                                       "PASS" if ok else "FAIL",
                                       "" if ok else f"{len(hits)} hit(s): {hits[:4]}"))
            continue
        if not args or isinstance(args[0], dict):
            scan.results.append(
                Result(path.name, name, str(args[0] if args else "?"), "",
                       "UNMEASURED", "argument is an expression this tool does not evaluate")
            )
            continue
        rel = args[0]
        target = workspace / rel
        if helper.kind == "file":
            scan.results.append(
                Result(path.name, name, rel, "", "PASS" if target.is_file() else "FAIL",
                       "" if target.is_file() else "missing file")
            )
            continue
        if len(args) < 2 or isinstance(args[1], dict):
            scan.results.append(
                Result(path.name, name, rel, "", "UNMEASURED",
                       "pattern is an expression this tool does not evaluate")
            )
            continue
        if not target.is_file():
            scan.results.append(Result(path.name, name, rel, "", "FAIL", "missing file"))
            continue
        body = target.read_text(encoding="utf-8", errors="replace")
        needles = args[1] if isinstance(args[1], list) else [args[1]]
        for needle in needles:
            if helper.literal:
                hit = needle in body
            else:
                try:
                    hit = re.search(needle, body, re.IGNORECASE | re.MULTILINE) is not None
                except re.error as exc:
                    scan.results.append(
                        Result(path.name, name, rel, needle, "UNMEASURED",
                               f"pattern is not a Python-compatible regex: {exc}")
                    )
                    continue
            want = helper.kind == "require"
            scan.results.append(
                Result(path.name, name, rel, needle,
                       "PASS" if hit == want else "FAIL",
                       "" if hit == want else ("marker absent" if want else "forbidden marker present"))
            )
    return scan


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("scripts", nargs="*", help="*.ps1 to scan; default: every gate in scripts/")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--quiet", action="store_true", help="print failures and unmeasured only")
    args = parser.parse_args()

    paths = [Path(s) for s in args.scripts] or sorted((WORKSPACE / "scripts").glob("*.ps1"))
    results: list[Result] = []
    skipped: list[str] = []
    silent: list[str] = []
    for path in paths:
        scan = scan_script(path if path.is_absolute() else WORKSPACE / path, WORKSPACE)
        if not scan.results:
            silent.append(path.name)
        results.extend(scan.results)
        skipped.extend(scan.skipped)

    failed = [r for r in results if r.verdict == "FAIL"]
    unmeasured = [r for r in results if r.verdict == "UNMEASURED"]
    if args.json:
        print(json.dumps({
            "assertions": len(results),
            "failed": [r.__dict__ for r in failed],
            "unmeasured": [r.__dict__ for r in unmeasured],
            "skipped_constructs": skipped,
            "scripts_scanned": len(paths),
            "scripts_without_assertions": silent,
        }, indent=1))
    else:
        by_script: dict[str, list[Result]] = {}
        for r in results:
            by_script.setdefault(r.script, []).append(r)
        for script, rows in sorted(by_script.items()):
            bad = [r for r in rows if r.verdict != "PASS"]
            if args.quiet and not bad:
                continue
            print(f"{script:34s} assertions={len(rows):3d} "
                  f"failed={len([r for r in rows if r.verdict == 'FAIL']):2d} "
                  f"unmeasured={len([r for r in rows if r.verdict == 'UNMEASURED']):2d}")
            for r in bad:
                print(f"    {r.verdict:10s} {r.path} :: {r.needle!r} {r.detail}")
        for s in skipped:
            print(f"    SKIPPED    {s}")
        print(f"\n{len(paths)} scripts scanned, {len(paths) - len(silent)} define assertion helpers; "
              f"the rest orchestrate and assert nothing of the source themselves.")
        print(f"total assertions={len(results)} failed={len(failed)} unmeasured={len(unmeasured)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
