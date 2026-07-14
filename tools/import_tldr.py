#!/usr/bin/env python3
"""Convert tldr-pages markdown into collective corpus YAML.

Usage:
  git clone --depth 1 https://github.com/tldr-pages/tldr /tmp/tldr
  python3 tools/import_tldr.py /tmp/tldr/pages --platforms osx common --out corpus/imported

Requires: pip install pyyaml
License: tldr-pages content is CC-BY-4.0; every generated entry carries
attribution in its `source` field. See NOTICE.
"""
import argparse
import pathlib
import re
import yaml


def sanitize(name: str) -> str:
    return re.sub(r"[^a-z0-9-]+", "-", name.lower()).strip("-")


def parse_page(text: str):
    """Return (page_description, [{'desc': ..., 'cmd': ...}])."""
    desc_line, examples, pending = "", [], None
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("> ") and not desc_line and "More information" not in line:
            desc_line = line[2:].rstrip(".")
        elif line.startswith("- "):
            pending = line[2:].rstrip(":")
        elif line.startswith("`") and pending:
            cmd = line.strip("`").replace("{{", "<").replace("}}", ">")
            examples.append({"desc": pending, "cmd": cmd})
            pending = None
    return desc_line, examples


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("pages_dir", help="path to tldr/pages")
    ap.add_argument("--platforms", nargs="+", default=["osx", "common"])
    ap.add_argument("--out", default="corpus/imported")
    args = ap.parse_args()

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    seen_pages, count = set(), 0

    for plat in args.platforms:  # osx first: platform page wins over common
        for page in sorted((pathlib.Path(args.pages_dir) / plat).glob("*.md")):
            name = page.stem
            if name in seen_pages:
                continue
            seen_pages.add(name)
            page_desc, examples = parse_page(page.read_text(encoding="utf-8"))
            slug = sanitize(name)
            if not slug:  # page name is all punctuation (e.g. ~, {) -> skip
                continue
            for i, ex in enumerate(examples, start=1):
                entry = {
                    "id": f"tldr-{slug}-{i}",
                    "title": f"{name}: {ex['desc']}",
                    "cmd": ex["cmd"],
                    "platform": ["macos"],
                    "domains": ["tldr-import"],
                    "danger": "low",
                    "explanation": page_desc or ex["desc"],
                    "source": (
                        f"https://github.com/tldr-pages/tldr/blob/main/pages/{plat}/{name}.md"
                        " (CC-BY-4.0)"
                    ),
                    "tags": [slug],
                }
                (out / f"{entry['id']}.yaml").write_text(
                    yaml.safe_dump(entry, sort_keys=False, allow_unicode=True),
                    encoding="utf-8",
                )
                count += 1
    print(f"wrote {count} entries from {len(seen_pages)} pages to {out}")


if __name__ == "__main__":
    main()
