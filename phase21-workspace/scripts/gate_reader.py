#!/usr/bin/env python3
"""One reader for the gate scripts. `DBT-P58-005`.

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
