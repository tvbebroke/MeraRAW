#!/usr/bin/env python3
"""Regenerate scoped UI shell stylesheets from unscoped sources.

Sources:
  src/styles/_faithful.source.css  (Lightroom Classic look from v0.1.4)
  src/styles/_modern.source.css    (Figma bento chrome from v0.1.5)

Outputs:
  src/styles/faithful.css
  src/styles/modern.css

Usage:
  python3 scripts/scope-ui-shell-css.py
"""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCES = {
    "faithful": ROOT / "src/styles/_faithful.source.css",
    "modern": ROOT / "src/styles/_modern.source.css",
}
OUT = {
    "faithful": ROOT / "src/styles/faithful.css",
    "modern": ROOT / "src/styles/modern.css",
}


def scope_css(src: str, shell: str) -> str:
    prefix = f'html[data-ui-shell="{shell}"]'
    out: list[str] = [
        f"/* Auto-scoped UI shell: {shell} — regenerate with scripts/scope-ui-shell-css.py */\n"
    ]
    i = 0
    n = len(src)
    if src.startswith("\ufeff"):
        src = src[1:]
        n = len(src)

    def skip_ws_comment(pos: int) -> tuple[int, str]:
        while pos < n:
            if src[pos] in " \t\r\n":
                pos += 1
                continue
            if src.startswith("/*", pos):
                end = src.find("*/", pos + 2)
                if end < 0:
                    return n, src[pos:]
                return end + 2, src[pos : end + 2]
            break
        return pos, ""

    while i < n:
        j, cmt = skip_ws_comment(i)
        if cmt:
            out.append(cmt)
            i = j
            continue
        if j > i:
            out.append(src[i:j])
            i = j
        if i >= n:
            break

        if src.startswith("@", i):
            m = re.match(
                r"@(keyframes|font-face|media|supports|layer|-webkit-keyframes)\b",
                src[i:],
                re.I,
            )
            brace = src.find("{", i)
            semi = src.find(";", i)
            if brace < 0 or (semi != -1 and semi < brace and not m):
                if semi < 0:
                    out.append(src[i:])
                    break
                out.append(src[i : semi + 1])
                i = semi + 1
                continue
            header = src[i:brace].strip()
            depth = 0
            k = brace
            while k < n:
                if src[k] == "{":
                    depth += 1
                elif src[k] == "}":
                    depth -= 1
                    if depth == 0:
                        break
                k += 1
            body = src[brace + 1 : k]
            name = m.group(1).lower() if m else ""
            if name in ("keyframes", "-webkit-keyframes", "font-face"):
                out.append(src[i : k + 1])
            elif name in ("media", "supports", "layer"):
                scoped_body = scope_css(body, shell)
                scoped_body = re.sub(r"^/\* Auto-scoped.*?\*/\n", "", scoped_body)
                out.append(f"{header} {{\n{scoped_body}\n}}")
            else:
                out.append(src[i : k + 1])
            i = k + 1
            continue

        brace = src.find("{", i)
        if brace < 0:
            out.append(src[i:])
            break
        selectors = src[i:brace].strip()
        depth = 0
        k = brace
        while k < n:
            if src[k] == "{":
                depth += 1
            elif src[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        body = src[brace + 1 : k]

        parts: list[str] = []
        buf: list[str] = []
        depth_p = 0
        for ch in selectors:
            if ch == "(":
                depth_p += 1
                buf.append(ch)
            elif ch == ")":
                depth_p -= 1
                buf.append(ch)
            elif ch == "," and depth_p == 0:
                parts.append("".join(buf).strip())
                buf = []
            else:
                buf.append(ch)
        if buf:
            parts.append("".join(buf).strip())

        scoped_parts: list[str] = []
        for sel in parts:
            if not sel:
                continue
            if sel == ":root":
                scoped_parts.append(prefix)
            elif sel.startswith(":root"):
                scoped_parts.append(prefix + sel[5:])
            elif sel.startswith("html"):
                scoped_parts.append(prefix + sel[4:])
            else:
                scoped_parts.append(f"{prefix} {sel}")
        out.append(",\n".join(scoped_parts) + " {" + body + "}")
        i = k + 1
    return "".join(out)


def main() -> None:
    for shell, src in SOURCES.items():
        if not src.exists():
            raise SystemExit(f"missing source: {src}")
        OUT[shell].write_text(scope_css(src.read_text(), shell))
        print(f"wrote {OUT[shell].relative_to(ROOT)}")


if __name__ == "__main__":
    main()
