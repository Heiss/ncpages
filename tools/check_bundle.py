#!/usr/bin/env python3
"""Check an OKF v0.2 knowledge bundle for conformance and link integrity.

Conformance rules checked (OKF v0.2, §Conformance):

  1. every non-reserved `.md` file has a parseable YAML frontmatter block
  2. every frontmatter block has a non-empty `type`
  3. reserved filenames (`index.md`, `log.md`) are not required to have any

Additionally checked, because broken links are the failure mode that actually
bites readers:

  * relative markdown links resolve to a file, or to a directory holding an
    `index.md`
  * `generated.by` and `verified[].by` follow the actor convention

Consumers must tolerate broken links per the spec; a bundle's own CI need not.

Usage: check_bundle.py <bundle-dir>
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

RESERVED = {"index.md", "log.md"}
FRONTMATTER = re.compile(r"\A---\n(.*?)\n---\n", re.DOTALL)
TYPE_KEY = re.compile(r"^type:\s*(\S.*)$", re.MULTILINE)
ACTOR_KEY = re.compile(r"^\s*(?:-\s*)?\{?\s*(?:by):\s*([^,}\n]+)", re.MULTILINE)
LINK = re.compile(r"\]\(([^)\s]+)\)")
ACTOR = re.compile(r"^(?:human:[\w.-]+|process:[\w.-]+|[\w.-]+/[\w.:-]+)$")


def check(bundle: Path) -> list[str]:
    problems: list[str] = []
    files = sorted(bundle.rglob("*.md"))
    if not files:
        return [f"{bundle}: no markdown files found"]

    for path in files:
        rel = path.relative_to(bundle)
        text = path.read_text(encoding="utf-8")
        match = FRONTMATTER.match(text)

        if path.name not in RESERVED:
            if not match:
                problems.append(f"{rel}: missing frontmatter block")
            elif not TYPE_KEY.search(match.group(1)):
                problems.append(f"{rel}: frontmatter has no non-empty `type`")

        if match:
            for actor in ACTOR_KEY.findall(match.group(1)):
                actor = actor.strip().strip("'\"")
                if not ACTOR.match(actor):
                    problems.append(
                        f"{rel}: `{actor}` does not follow the actor convention "
                        f"(agent/version, human:id or process:id)"
                    )

        for target in LINK.findall(text):
            if target.startswith(("http://", "https://", "mailto:", "#", "file:")):
                continue
            resolved = (path.parent / target.split("#")[0]).resolve()
            if resolved.is_dir():
                resolved = resolved / "index.md"
            if not resolved.exists():
                problems.append(f"{rel}: broken link `{target}`")

    return problems


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__.strip().splitlines()[-1], file=sys.stderr)
        return 2

    bundle = Path(sys.argv[1])
    if not bundle.is_dir():
        print(f"not a directory: {bundle}", file=sys.stderr)
        return 2

    problems = check(bundle)
    count = len(sorted(bundle.rglob("*.md")))

    for problem in problems:
        print(f"error: {problem}", file=sys.stderr)

    if problems:
        print(f"\n{len(problems)} problem(s) in {count} file(s)", file=sys.stderr)
        return 1

    print(f"OK: {count} file(s), OKF v0.2 conformant, all links resolve")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
