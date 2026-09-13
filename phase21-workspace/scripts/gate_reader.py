#!/usr/bin/env python3
"""One reader, and one token comparison, for the gate scripts.

`DBT-P58-005` (the reader) and `DBT-P61-001` (the comparison, see `contains`).

Ten gates read their sources through a helper shaped like

    p.read_text(encoding="utf-8") if p.exists() else ""

which cannot tell "this source is absent" from "this source is empty". Every
check that asserts something is **absent** from a source then passes against a
source the gate never opened. Measured in `docs/phase59/P59-READER-CENSUS.md`:
`phase16-ga-audit.py` reports `ok: true, 42 checks, 0 failed` with one of the
five files it audits deleted from the tree.

P58 repaired *where* eight of those readers looked. It did not change what they
do when the look fails, and a guard at each of the several hundred call sites is
several hundred guards plus the next one nobody writes. The fix belongs at the
read: a source a gate cannot read is a gate failure, so this reader raises. The
exception is deliberately not caught anywhere - the gate exits non-zero and the
message names the file and the absolute path it was expected at.
"""
from __future__ import annotations

from functools import lru_cache
from pathlib import Path


class UnreadableSource(Exception):
    """A gate asked for a source it could not read. Never swallowed, never ""."""


class SourceReader:
    """Resolves and reads gate sources.

    `.github/` is repository-root relative and everything else is workspace
    relative: P58 / `DBT-P55-001` - GitHub Actions reads workflows only from
    `.github/workflows` at the repository root, one directory above this
    workspace, which is why the eight readers were returning "" for files that
    existed.
    """

    def __init__(self, workspace: Path) -> None:
        self.workspace = workspace
        self.repo = workspace.parent

    def base(self, rel: str) -> Path:
        return self.repo if rel.startswith(".github/") else self.workspace

    def read(self, rel: str) -> str:
        path = self.base(rel) / rel
        try:
            return path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as exc:
            raise UnreadableSource(
                f"gate source {rel!r} could not be read at {path}: {exc}"
            ) from None


@lru_cache(maxsize=None)
def _squash(text: str) -> str:
    """`text` with every whitespace run removed.

    Cached because the same source string is squashed once per token and the
    gates ask hundreds of questions of a handful of files.
    """
    return "".join(text.split())


def contains(text: str, token: str) -> bool:
    """Is `token` present in `text`, ignoring whitespace? `DBT-P61-001`.

    The six token gates assert by substring, and their tokens were written
    against a tree that predates the formatter: `hardware_gate:IsolationGate`,
    `record.claimed=true`, `if args.len()!=5`. `cargo fmt` now owns the spacing
    of every Rust source in the workspace (`cargo fmt --all -- --check`, exit 0),
    so the tree says `hardware_gate: IsolationGate` and 81 of the 108 failures in
    `DBT-P61-001` are the gate spelling a construct that is present. Classified
    before it was touched, in `docs/phase62/P62-GATE-CLASSIFICATION.md`.

    Whitespace is not load-bearing in any of these tokens, so the comparison
    drops it on both sides. It lives here, once, rather than in each gate's own
    `has`/`marker`: six copies of a comparison is six things to keep in step.

    This is deliberately a **widening** of the old `token in text`: squashing
    preserves the order of every non-whitespace character, so anything that
    matched exactly still matches. What it does not do is invent a match - a
    token naming an identifier, a field or a call that is not in the tree is
    still absent once the spaces are gone, and the check still fails. The two
    cases in `test_gate_contains.py` are precisely that pair.
    """
    return token in text or _squash(token) in _squash(text)


def count(text: str, token: str) -> int:
    """How many times `token` occurs in `text`, ignoring whitespace.

    The `>= N` checks are the same `DBT-P61-001` defect as `contains`: a gate
    counting `verify_file_hash_size(&path` against a `rustfmt`-clean tree counts
    zero. Squashing preserves the order of non-whitespace characters, so this is
    never smaller than `text.count(token)`.
    """
    return _squash(text).count(_squash(token))


def position(text: str, token: str, start: int = 0) -> int:
    """Index of `token` in `text` ignoring whitespace, or -1. `DBT-P61-001`.

    The index is in SQUASHED coordinates, which is why `start` must come from
    another `position` call rather than from `str.find`. Squashing is monotone -
    it deletes characters and never reorders them - so `position(a) <
    position(b)` holds exactly when `a` precedes `b` in the real source. That is
    the only property the ordering checks need, and it is the reason these
    indices are never reported as line numbers.
    """
    return _squash(text).find(_squash(token), start)
