#!/usr/bin/env python3

from __future__ import annotations

import argparse
import html
import re
import sys
from pathlib import Path
from urllib.parse import unquote, urlparse

LINK_RE = re.compile(r'(?<!!)(?:\[[^\]]*\])\(([^)]+)\)')
HEADING_RE = re.compile(r'^(#{1,6})\s+(.*?)\s*$')
FENCE_RE = re.compile(r'^(```|~~~)')
COMMENT_RE = re.compile(r'<!--.*?-->', re.DOTALL)


class LinkError(Exception):
    pass


def iter_markdown_files(paths: list[Path]) -> list[Path]:
    files: list[Path] = []
    for path in paths:
        if not path.exists():
            raise LinkError(f"input path does not exist: {path}")
        if path.is_dir():
            files.extend(sorted(p for p in path.rglob("*.md") if p.is_file()))
        elif path.is_file():
            files.append(path)
    return sorted(dict.fromkeys(files))


def strip_comments_and_fences(text: str) -> str:
    text = COMMENT_RE.sub("", text)
    lines: list[str] = []
    in_fence = False
    for line in text.splitlines():
        if FENCE_RE.match(line.strip()):
            in_fence = not in_fence
            continue
        if not in_fence:
            lines.append(line)
    return "\n".join(lines)


def extract_links(path: Path) -> list[tuple[int, str]]:
    text = strip_comments_and_fences(path.read_text(encoding="utf-8"))
    links: list[tuple[int, str]] = []
    for line_number, line in enumerate(text.splitlines(), start=1):
        for match in LINK_RE.finditer(line):
            dest = match.group(1).strip()
            if " " in dest and not dest.startswith("<"):
                dest = dest.split(" ", 1)[0]
            if dest.startswith("<") and dest.endswith(">"):
                dest = dest[1:-1]
            links.append((line_number, dest))
    return links


def is_external(dest: str) -> bool:
    if dest.startswith(("mailto:", "tel:", "data:", "javascript:")):
        return True
    parsed = urlparse(dest)
    return bool(parsed.scheme)


def slugify_heading(text: str) -> str:
    text = html.unescape(text)
    text = re.sub(r'<[^>]+>', '', text)
    text = re.sub(r'`([^`]*)`', r'\1', text)
    text = re.sub(r'\[([^\]]+)\]\([^)]*\)', r'\1', text)
    text = text.strip().strip('#').strip().lower()
    chars: list[str] = []
    for ch in text:
        if ch.isalnum() or ch in {' ', '-', '_'}:
            chars.append(ch)
    slug = ''.join(chars).strip()
    slug = re.sub(r'[\s-]+', '-', slug)
    return slug


def collect_anchors(path: Path) -> set[str]:
    text = strip_comments_and_fences(path.read_text(encoding="utf-8"))
    anchors: set[str] = set()
    counts: dict[str, int] = {}
    for line in text.splitlines():
        match = HEADING_RE.match(line)
        if not match:
            continue
        heading = re.sub(r'\s+#+\s*$', '', match.group(2)).strip()
        slug = slugify_heading(heading)
        if not slug:
            continue
        count = counts.get(slug, 0)
        anchor = slug if count == 0 else f"{slug}-{count}"
        counts[slug] = count + 1
        anchors.add(anchor)
    return anchors


def resolve_markdown_target(source: Path, dest: str) -> tuple[Path, str | None]:
    raw_target = unquote(dest)
    path_part, anchor = raw_target.split('#', 1) if '#' in raw_target else (raw_target, None)
    target = source if path_part == "" else (source.parent / path_part).resolve()
    return target, anchor


def anchor_target_for(path: Path) -> Path:
    if path.is_dir():
        for candidate in (path / "README.md", path / "readme.md", path / "index.md"):
            if candidate.exists():
                return candidate
        raise LinkError(f"directory has no README.md or index.md for anchor lookup: {path}")
    return path


def check_links(files: list[Path]) -> list[str]:
    failures: list[str] = []
    anchor_cache: dict[Path, set[str]] = {}

    for source in files:
        for line_number, dest in extract_links(source):
            if is_external(dest):
                continue
            try:
                target, anchor = resolve_markdown_target(source, dest)
                if not target.exists():
                    failures.append(
                        f"{source}:{line_number}: broken local link '{dest}' (missing path: {target})"
                    )
                    continue
                if anchor is None:
                    continue
                anchor_doc = anchor_target_for(target)
                if anchor_doc.suffix.lower() != ".md":
                    failures.append(
                        f"{source}:{line_number}: anchor link '{dest}' points to non-Markdown file: {anchor_doc}"
                    )
                    continue
                anchors = anchor_cache.setdefault(anchor_doc, collect_anchors(anchor_doc))
                if anchor not in anchors:
                    failures.append(
                        f"{source}:{line_number}: broken anchor '{anchor}' in link '{dest}' (checked {anchor_doc})"
                    )
            except LinkError as exc:
                failures.append(f"{source}:{line_number}: invalid local link '{dest}' ({exc})")

    return failures


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check local Markdown links and heading anchors without external dependencies."
    )
    parser.add_argument(
        "paths",
        nargs="*",
        default=["README.md", "docs", "examples"],
        help="Markdown files or directories to scan (default: README.md docs examples)",
    )
    args = parser.parse_args()

    roots = [Path(p).resolve() for p in args.paths]
    try:
        files = iter_markdown_files(roots)
    except LinkError as exc:
        print(f"markdown link check failed: {exc}", file=sys.stderr)
        return 1

    failures = check_links(files)
    if failures:
        print("markdown link check failed:\n", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print(f"markdown link check passed: {len(files)} files scanned")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
