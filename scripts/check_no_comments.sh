#!/usr/bin/env bash
set -euo pipefail

DEFAULT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${1:-$DEFAULT_ROOT}"

exec python3 - "$ROOT_DIR" <<'PY'
import ast
import io
import os
import sys
import tokenize

root = os.path.abspath(sys.argv[1])

EXCLUDED_DIRS = {".git", "target", "vendor", "vendored", "node_modules", "dist"}
EXCLUDED_FILES = {"Cargo.lock", "THIRD_PARTY_NOTICES.md"}
SCAN_EXTS = {
    ".rs",
    ".py",
    ".sh",
    ".bash",
    ".zsh",
    ".toml",
    ".json",
    ".yaml",
    ".yml",
    ".ts",
    ".tsx",
    ".js",
    ".jsx",
    ".css",
    ".html",
}
CONFIG_EXACT_NAMES = {
    ".gitignore",
    ".gitattributes",
    ".editorconfig",
    ".env",
    ".env.example",
    "Cargo.toml",
    "deny.toml",
    "package.json",
    "tsconfig.json",
}


def is_excluded_rel(rel_path):
    parts = rel_path.split(os.sep)
    if any(part in EXCLUDED_DIRS for part in parts):
        return True
    if parts[-1] in EXCLUDED_FILES:
        return True
    if rel_path.endswith(".md"):
        return True
    return False


def should_scan(rel_path):
    if is_excluded_rel(rel_path):
        return False
    parts = rel_path.split(os.sep)
    base = parts[-1]
    _, ext = os.path.splitext(base)
    if parts[0] in {"src", "tests", "scripts", "src-tauri", "ui"}:
        return ext in SCAN_EXTS or base in CONFIG_EXACT_NAMES
    if ext in {".toml", ".json", ".yaml", ".yml", ".sh", ".py", ".rs", ".ts", ".tsx", ".js", ".jsx", ".css", ".html"}:
        return True
    if base in CONFIG_EXACT_NAMES or base.startswith(".env"):
        return True
    return False


def collect_files():
    matches = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in EXCLUDED_DIRS)
        for fname in sorted(filenames):
            full = os.path.join(dirpath, fname)
            rel = os.path.relpath(full, root)
            if should_scan(rel):
                matches.append((rel, full))
    return matches


def scan_rust(rel, text):
    findings = []
    n = len(text)
    i = 0
    line = 1
    col = 1

    def advance(k=1):
        nonlocal i, line, col
        for _ in range(k):
            if i < n:
                if text[i] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 1
                i += 1

    while i < n:
        ch = text[i]

        raw_prefix = None
        for prefix in ("br", "cr", "r"):
            if text.startswith(prefix, i) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
                j = i + len(prefix)
                hashes = 0
                while j < n and text[j] == chr(35):
                    hashes += 1
                    j += 1
                if j < n and text[j] == '"':
                    raw_prefix = (j + 1, hashes)
                    break
        if raw_prefix is not None:
            start_quote_end, hashes = raw_prefix
            advance(start_quote_end - i)
            closing = '"' + (chr(35) * hashes)
            while i < n:
                if text.startswith(closing, i):
                    advance(len(closing))
                    break
                advance(1)
            continue

        if (ch == "b" or ch == "c") and i + 1 < n and text[i + 1] == '"' and (
            i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")
        ):
            advance(2)
            while i < n:
                if text[i] == "\\":
                    advance(2)
                elif text[i] == '"':
                    advance(1)
                    break
                else:
                    advance(1)
            continue

        if ch == '"':
            advance(1)
            while i < n:
                if text[i] == "\\":
                    advance(2)
                elif text[i] == '"':
                    advance(1)
                    break
                else:
                    advance(1)
            continue

        if ch == "'":
            if i + 1 < n and text[i + 1] == "\\":
                j = i + 2
                while j < n and text[j] != "\n" and j - i <= 12:
                    if text[j] == "'":
                        advance(j - i + 1)
                        j = -1
                        break
                    j += 1
                if j == -1:
                    continue
            elif i + 2 < n and text[i + 2] == "'" and text[i + 1] != "\n":
                advance(3)
                continue
            advance(1)
            continue

        if text.startswith("//", i):
            findings.append((rel, line, col, "Rust line comment"))
            while i < n and text[i] != "\n":
                advance(1)
            continue

        if text.startswith("/*", i):
            findings.append((rel, line, col, "Rust block comment"))
            advance(2)
            depth = 1
            while i < n and depth > 0:
                if text.startswith("/*", i):
                    depth += 1
                    advance(2)
                elif text.startswith("*/", i):
                    depth -= 1
                    advance(2)
                else:
                    advance(1)
            continue

        advance(1)

    return findings


def scan_python(rel, text):
    findings = []
    try:
        tokens = tokenize.generate_tokens(io.StringIO(text).readline)
        for tok in tokens:
            if tok.type == tokenize.COMMENT:
                if tok.start[0] == 1 and tok.string.startswith("#!"):
                    continue
                findings.append((rel, tok.start[0], tok.start[1] + 1, "Python comment"))
    except tokenize.TokenError as exc:
        findings.append((rel, 1, 1, f"Python tokenize error: {exc}"))

    try:
        tree = ast.parse(text)
        for node in ast.walk(tree):
            if isinstance(node, (ast.Module, ast.ClassDef, ast.FunctionDef, ast.AsyncFunctionDef)):
                for stmt in node.body:
                    if (
                        isinstance(stmt, ast.Expr)
                        and isinstance(stmt.value, ast.Constant)
                        and isinstance(stmt.value.value, str)
                    ):
                        findings.append((rel, stmt.lineno, stmt.col_offset + 1, "Python docstring/string statement"))
    except SyntaxError as exc:
        findings.append((rel, exc.lineno or 1, 1, f"Python syntax error: {exc}"))

    return findings


def scan_shell(rel, text):
    findings = []
    lines = text.splitlines(keepends=True)
    idx = 0
    total = len(text)
    line = 1
    col = 1

    if lines and lines[0].startswith("#!"):
        idx = len(lines[0])
        line = 2
        col = 1

    in_single = False
    in_double = False
    word_start = True

    while idx < total:
        ch = text[idx]
        if ch == "\n":
            line += 1
            col = 1
            idx += 1
            word_start = True
            continue

        if in_single:
            if ch == "'":
                in_single = False
            idx += 1
            col += 1
            continue

        if in_double:
            if ch == "\\" and idx + 1 < total:
                if text[idx + 1] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 2
                idx += 2
                continue
            if ch == '"':
                in_double = False
            idx += 1
            col += 1
            continue

        if ch == "\\" and idx + 1 < total:
            if text[idx + 1] == "\n":
                line += 1
                col = 1
            else:
                col += 2
            idx += 2
            word_start = False
            continue

        if ch == "<" and text.startswith("<<", idx):
            k = idx + 2
            while k < total and text[k] == "-":
                k += 1
            while k < total and text[k] in " \t":
                k += 1
            quote_char = None
            if k < total and text[k] in "'\"":
                quote_char = text[k]
                k += 1
            tag_start = k
            while k < total and (text[k].isalnum() or text[k] == "_"):
                k += 1
            tag = text[tag_start:k]
            if quote_char and k < total and text[k] == quote_char:
                k += 1
            if tag:
                eol = text.find("\n", idx)
                if eol == -1:
                    break
                rest_pos = eol + 1
                line += 1
                col = 1
                body_start = rest_pos
                body_line_start = line
                while rest_pos < total:
                    next_nl = text.find("\n", rest_pos)
                    raw_line = text[rest_pos:] if next_nl == -1 else text[rest_pos:next_nl]
                    if raw_line == tag:
                        body_text = text[body_start:rest_pos]
                        if tag.upper() == "PY" or "python" in text[idx:eol]:
                            for _, sub_l, sub_c, sub_msg in scan_python(rel, body_text):
                                findings.append((rel, body_line_start + sub_l - 1, sub_c, sub_msg))
                        idx = len(text) if next_nl == -1 else next_nl + 1
                        line += 1
                        col = 1
                        break
                    line += 1
                    if next_nl == -1:
                        idx = total
                        break
                    rest_pos = next_nl + 1
                word_start = True
                continue

        if ch == "'":
            in_single = True
            word_start = False
        elif ch == '"':
            in_double = True
            word_start = False
        elif ch == chr(35) and word_start:
            findings.append((rel, line, col, "Shell comment"))
            while idx < total and text[idx] != "\n":
                idx += 1
            continue
        elif ch in " \t;|&()":
            word_start = True
        else:
            word_start = False

        idx += 1
        col += 1

    return findings


def scan_toml_or_yaml(rel, text):
    findings = []
    n = len(text)
    i = 0
    line = 1
    col = 1

    def adv(k=1):
        nonlocal i, line, col
        for _ in range(k):
            if i < n:
                if text[i] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 1
                i += 1

    while i < n:
        if text.startswith('"""', i):
            adv(3)
            while i < n and not text.startswith('"""', i):
                if text[i] == "\\":
                    adv(2)
                else:
                    adv(1)
            adv(3)
        elif text.startswith("'''", i):
            adv(3)
            while i < n and not text.startswith("'''", i):
                adv(1)
            adv(3)
        elif text[i] == '"':
            adv(1)
            while i < n and text[i] != '"':
                if text[i] == "\\":
                    adv(2)
                else:
                    adv(1)
            adv(1)
        elif text[i] == "'":
            adv(1)
            while i < n and text[i] != "'":
                adv(1)
            adv(1)
        elif text[i] == chr(35):
            findings.append((rel, line, col, "Config comment"))
            while i < n and text[i] != "\n":
                adv(1)
        else:
            adv(1)

    return findings


def scan_json(rel, text):
    findings = []
    n = len(text)
    i = 0
    line = 1
    col = 1

    def adv(k=1):
        nonlocal i, line, col
        for _ in range(k):
            if i < n:
                if text[i] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 1
                i += 1

    while i < n:
        if text[i] == '"':
            adv(1)
            while i < n and text[i] != '"':
                if text[i] == "\\":
                    adv(2)
                else:
                    adv(1)
            adv(1)
        elif text.startswith("//", i):
            findings.append((rel, line, col, "JSON line comment"))
            while i < n and text[i] != "\n":
                adv(1)
        elif text.startswith("/*", i):
            findings.append((rel, line, col, "JSON block comment"))
            adv(2)
            while i < n and not text.startswith("*/", i):
                adv(1)
            adv(2)
        elif text[i] == chr(35):
            findings.append((rel, line, col, "JSON hash comment"))
            while i < n and text[i] != "\n":
                adv(1)
        else:
            adv(1)

    return findings


def scan_line_hash_config(rel, text):
    findings = []
    for lineno, raw_line in enumerate(text.splitlines(), start=1):
        stripped = raw_line.lstrip(" \t")
        if stripped.startswith(chr(35)):
            col = len(raw_line) - len(stripped) + 1
            findings.append((rel, lineno, col, "Config file comment"))
    return findings


def scan_js_ts(rel, text):
    findings = []
    n = len(text)
    i = 0
    line = 1
    col = 1
    prev_token = ""
    expr_stack = []

    def adv(k=1):
        nonlocal i, line, col
        for _ in range(k):
            if i < n:
                if text[i] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 1
                i += 1

    def can_start_regex(tok):
        return tok in {
            "",
            "(",
            "[",
            "{",
            ",",
            ";",
            ":",
            "=",
            "!",
            "&",
            "|",
            "?",
            "+",
            "-",
            "*",
            "%",
            "^",
            "~",
            "<",
            ">",
            "=>",
            "return",
            "typeof",
            "void",
            "delete",
            "await",
            "yield",
            "case",
            "in",
            "of",
            "else",
            "throw",
            "new",
            "default",
        }

    while i < n:
        ch = text[i]

        if ch in " \t\r\n":
            adv(1)
            continue

        if expr_stack and expr_stack[-1][0] == "template" and expr_stack[-1][1] == 0:
            if text.startswith("${", i):
                expr_stack[-1] = ("template", 1)
                adv(2)
                prev_token = "{"
                continue
            elif ch == "`":
                expr_stack.pop()
                adv(1)
                prev_token = "str"
                continue
            elif ch == "\\":
                adv(2)
                continue
            else:
                adv(1)
                continue

        if ch == "`":
            expr_stack.append(("template", 0))
            adv(1)
            continue

        if ch == "'" or ch == '"':
            q = ch
            adv(1)
            while i < n:
                if text[i] == "\\":
                    adv(2)
                elif text[i] == q:
                    adv(1)
                    break
                else:
                    adv(1)
            prev_token = "str"
            continue

        if text.startswith("//", i):
            findings.append((rel, line, col, "JS/TS line comment"))
            while i < n and text[i] != "\n":
                adv(1)
            continue

        if text.startswith("/*", i):
            findings.append((rel, line, col, "JS/TS block comment"))
            adv(2)
            while i < n and not text.startswith("*/", i):
                adv(1)
            adv(2)
            continue

        if ch == "/" and can_start_regex(prev_token):
            adv(1)
            in_class = False
            while i < n:
                if text[i] == "\\":
                    adv(2)
                elif text[i] == "[":
                    in_class = True
                    adv(1)
                elif text[i] == "]" and in_class:
                    in_class = False
                    adv(1)
                elif text[i] == "/" and not in_class:
                    adv(1)
                    while i < n and text[i].isalpha():
                        adv(1)
                    break
                elif text[i] == "\n":
                    break
                else:
                    adv(1)
            prev_token = "regex"
            continue

        if ch == "{" and expr_stack and expr_stack[-1][0] == "template":
            expr_stack[-1] = ("template", expr_stack[-1][1] + 1)
            prev_token = "{"
            adv(1)
            continue

        if ch == "}" and expr_stack and expr_stack[-1][0] == "template":
            depth = expr_stack[-1][1] - 1
            expr_stack[-1] = ("template", depth)
            prev_token = "}"
            adv(1)
            continue

        if ch.isalpha() or ch == "_" or ch == "$":
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] in "_$"):
                j += 1
            prev_token = text[i:j]
            adv(j - i)
            continue

        if ch.isdigit():
            j = i + 1
            while j < n and (text[j].isalnum() or text[j] in "._"):
                j += 1
            prev_token = "num"
            adv(j - i)
            continue

        if text.startswith("=>", i):
            prev_token = "=>"
            adv(2)
            continue

        prev_token = ch
        adv(1)

    return findings


def scan_css(rel, text):
    findings = []
    n = len(text)
    i = 0
    line = 1
    col = 1

    def adv(k=1):
        nonlocal i, line, col
        for _ in range(k):
            if i < n:
                if text[i] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 1
                i += 1

    while i < n:
        ch = text[i]
        if ch in "'\"":
            q = ch
            adv(1)
            while i < n:
                if text[i] == "\\":
                    adv(2)
                elif text[i] == q:
                    adv(1)
                    break
                else:
                    adv(1)
        elif text.startswith("/*", i):
            findings.append((rel, line, col, "CSS block comment"))
            adv(2)
            while i < n and not text.startswith("*/", i):
                adv(1)
            adv(2)
        else:
            adv(1)

    return findings


def scan_html(rel, text):
    findings = []
    n = len(text)
    i = 0
    line = 1
    col = 1

    def adv(k=1):
        nonlocal i, line, col
        for _ in range(k):
            if i < n:
                if text[i] == "\n":
                    line += 1
                    col = 1
                else:
                    col += 1
                i += 1

    while i < n:
        if text.startswith("<!--", i):
            findings.append((rel, line, col, "HTML comment"))
            adv(4)
            while i < n and not text.startswith("-->", i):
                adv(1)
            adv(3)
        else:
            adv(1)

    return findings


all_findings = []
scanned_count = 0

for rel, full in collect_files():
    scanned_count += 1
    content = open(full, "r", encoding="utf-8", errors="replace").read()
    base = os.path.basename(rel)
    _, ext = os.path.splitext(base)

    if ext == ".rs":
        all_findings.extend(scan_rust(rel, content))
    elif ext == ".py":
        all_findings.extend(scan_python(rel, content))
    elif ext in {".ts", ".tsx", ".js", ".jsx"}:
        all_findings.extend(scan_js_ts(rel, content))
    elif ext == ".css":
        all_findings.extend(scan_css(rel, content))
    elif ext == ".html":
        all_findings.extend(scan_html(rel, content))
    elif ext in {".sh", ".bash", ".zsh"} or content.startswith("#!/bin/") or content.startswith("#!/usr/bin/env bash") or content.startswith("#!/usr/bin/env sh"):
        all_findings.extend(scan_shell(rel, content))
    elif ext in {".toml", ".yaml", ".yml"}:
        all_findings.extend(scan_toml_or_yaml(rel, content))
    elif ext == ".json":
        all_findings.extend(scan_json(rel, content))
    else:
        all_findings.extend(scan_line_hash_config(rel, content))

for rel, line, col, kind in all_findings:
    print(f"{rel}:{line}:{col}: {kind}")

if all_findings:
    print(f"Found {len(all_findings)} comment(s) across {scanned_count} checked file(s).", file=sys.stderr)
    sys.exit(1)

print(f"OK: 0 comments found across {scanned_count} checked file(s).")
PY
