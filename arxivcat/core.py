import os
import requests
import tarfile
import re
import shutil
import tempfile
from pathlib import Path
from typing import Callable


LogFn = Callable[[str], None]


# ── Utilities ─────────────────────────────────────────────────

def _log(log: LogFn | None, msg: str) -> None:
    (log or print)(msg)


def extract_arxiv_id(input_str):
    match = re.search(r'(\d+\.\d+(?:v\d+)?)', input_str)
    return match.group(1) if match else None


def extract_arxiv_id_from_pdf(pdf_path, log: LogFn | None = None):
    """Extract arXiv ID (with version) from a local PDF file."""
    try:
        import fitz  # pymupdf
    except ImportError:
        _log(log, "[ERROR] pymupdf not installed (pip install pymupdf)")
        return None

    try:
        doc = fitz.open(pdf_path)
    except Exception as e:
        _log(log, f"[WARN] Cannot open PDF {Path(pdf_path).name}: {e}")
        return None

    arxiv_pat = r'arXiv[:\s]*(\d{4}\.\d{4,5}(?:v\d+)?)'
    bare_pat = r'(\d{4}\.\d{4,5}(?:v\d+)?)'

    # Strategy 1: PDF metadata
    meta = doc.metadata or {}
    for key in ("subject", "keywords", "title", "author"):
        val = meta.get(key, "") or ""
        m = re.search(bare_pat, val)
        if m:
            doc.close()
            return m.group(1)

    # Strategy 2: first page arXiv watermark
    if doc.page_count > 0:
        text = doc[0].get_text()
        m = re.search(arxiv_pat, text)
        if m:
            doc.close()
            return m.group(1)
        m = re.search(bare_pat, text)
        if m:
            doc.close()
            return m.group(1)

    # Strategy 3: first 3 pages
    for i in range(min(doc.page_count, 3)):
        text = doc[i].get_text()
        m = re.search(arxiv_pat, text)
        if m:
            doc.close()
            return m.group(1)

    doc.close()
    return None


def sanitize_filename(name):
    name = re.sub(r'[<>:"/\\|?*]', '_', name)
    name = re.sub(r'[\s_]+', '_', name)
    return name.strip('_')[:80]


def fetch_title_from_arxiv(arxiv_id, log: LogFn | None = None):
    try:
        resp = requests.get(f"https://arxiv.org/abs/{arxiv_id}", timeout=15)
        resp.raise_for_status()
        m = re.search(r'<meta property="og:title" content="(.+?)"', resp.text)
        if m:
            return m.group(1).strip()
    except Exception as e:
        _log(log, f"[WARN] Failed to fetch title: {e}")
    return None


def download_pdf(arxiv_id, output_dir, log: LogFn | None = None):
    """Download the PDF from arXiv into output_dir. Returns path or None."""
    pdf_path = output_dir / f"{arxiv_id}.pdf"
    if pdf_path.exists():
        _log(log, f"[INFO] PDF already exists: {pdf_path.name}")
        return pdf_path
    _log(log, "[INFO] Downloading PDF...")
    try:
        resp = requests.get(
            f"https://arxiv.org/pdf/{arxiv_id}",
            timeout=120,
            stream=True,
        )
        resp.raise_for_status()
        total = int(resp.headers.get("content-length", 0))
        downloaded = 0
        last_report = 0
        with open(pdf_path, "wb") as f:
            for chunk in resp.iter_content(8192):
                if chunk:
                    f.write(chunk)
                    downloaded += len(chunk)
                    if downloaded - last_report >= 256 * 1024:
                        last_report = downloaded
                        if total:
                            pct = downloaded * 100 // total
                            _log(log, f"[INFO] PDF: {downloaded // 1024}/{total // 1024} KB ({pct}%)")
                        else:
                            _log(log, f"[INFO] PDF: {downloaded // 1024} KB")
        _log(log, f"[OK] PDF saved ({pdf_path.stat().st_size // 1024} KB)")
        return pdf_path
    except Exception as e:
        _log(log, f"[WARN] PDF download failed: {e}")
        return None


# ── Download & Extract ────────────────────────────────────────

def _can_walk_dir(path: Path) -> bool:
    try:
        for _ in path.rglob("*"):
            pass
        return True
    except Exception:
        return False


def _can_read_tex_files(path: Path) -> bool:
    try:
        tex_files = list(path.rglob("*.tex"))
    except Exception:
        return False

    if not tex_files:
        return False

    for p in tex_files:
        try:
            p.read_text(encoding="utf-8", errors="ignore")
        except Exception:
            return False
    return True


def _all_inputs_readable(main_tex: Path, paper_dir: Path) -> bool:
    try:
        content = main_tex.read_text(encoding="utf-8", errors="ignore")
    except Exception:
        return False

    content = _strip_latex_comments(content)
    refs = re.findall(r'\\(?:input|include)\s*\{([^}]+)\}', content)
    for ref in refs:
        name = ref.strip()
        # Try original name first, then add .tex if it doesn't exist
        p = (paper_dir / name).resolve()
        if not p.exists() or not p.is_file():
            if not name.endswith('.tex'):
                name_with_tex = name + '.tex'
                p = (paper_dir / name_with_tex).resolve()
        if not p.exists() or not p.is_file():
            return False
        try:
            p.read_text(encoding="utf-8", errors="ignore")
        except Exception:
            return False

    return True


def _repair_permissions(path: Path) -> None:
    for p in path.rglob("*"):
        try:
            os.chmod(p, 0o777 if p.is_dir() else 0o666)
        except Exception:
            pass


def _strip_latex_comments(tex_content: str) -> str:
    return re.sub(r'(?<!\\)%.*', '', tex_content)


def _is_safe_tar_member(member: tarfile.TarInfo, target_dir: Path) -> bool:
    target_dir = target_dir.resolve()
    member_path = (target_dir / member.name).resolve()
    return member_path == target_dir or target_dir in member_path.parents


def download_source(arxiv_id, downloads_dir, log: LogFn | None = None):
    """Download and unpack arXiv source. Returns (paper_dir, folder_name)."""
    _log(log, f"[INFO] Fetching title for {arxiv_id}...")
    title = fetch_title_from_arxiv(arxiv_id, log=log) or "unknown"
    _log(log, f"[INFO] Title: {title}")

    folder_name = f"{arxiv_id.replace('.', '_')}_{sanitize_filename(title)}"
    paper_dir = downloads_dir / folder_name

    if paper_dir.exists():
        main_tex = find_main_tex(paper_dir)
        cache_ok = (
            main_tex is not None
            and _can_walk_dir(paper_dir)
            and _can_read_tex_files(paper_dir)
            and _all_inputs_readable(main_tex, paper_dir)
        )
        if cache_ok:
            _log(log, "[INFO] Already cached, skipping download.")
            return paper_dir, folder_name

        _log(log, "[WARN] Cache exists but is not readable. Repairing cache permissions...")
        _repair_permissions(paper_dir)

        main_tex = find_main_tex(paper_dir)
        cache_ok = (
            main_tex is not None
            and _can_walk_dir(paper_dir)
            and _can_read_tex_files(paper_dir)
            and _all_inputs_readable(main_tex, paper_dir)
        )
        if cache_ok:
            _log(log, "[INFO] Cache repaired, skipping download.")
            return paper_dir, folder_name

        _log(log, "[WARN] Cache still broken. Re-downloading source...")
        try:
            shutil.rmtree(paper_dir, ignore_errors=True)
        except Exception:
            pass

        if paper_dir.exists():
            suffix = 1
            while (downloads_dir / f"{folder_name}_fresh{suffix}").exists():
                suffix += 1
            folder_name = f"{folder_name}_fresh{suffix}"
            paper_dir = downloads_dir / folder_name
            _log(log, f"[WARN] Old cache locked; using new cache dir: {folder_name}")

    _log(log, "[INFO] Downloading source...")
    try:
        resp = requests.get(
            f"https://arxiv.org/src/{arxiv_id}",
            timeout=120,
            stream=True,
        )
        resp.raise_for_status()
    except Exception as e:
        _log(log, f"[ERROR] Download failed: {e}")
        return None, None

    total = int(resp.headers.get("content-length", 0))
    temp_tar = downloads_dir / f"{arxiv_id}.tar.gz"
    downloaded = 0
    chunk_size = 8192
    last_pct = -1

    with open(temp_tar, "wb") as f:
        for chunk in resp.iter_content(chunk_size=chunk_size):
            if chunk:
                f.write(chunk)
                downloaded += len(chunk)
                if total:
                    pct = int(downloaded / total * 100)
                    if pct != last_pct and pct % 10 == 0:
                        _log(log, f"[INFO] Downloading... {pct}% ({downloaded // 1024} KB / {total // 1024} KB)")
                        last_pct = pct
                else:
                    kb = downloaded // 1024
                    if kb % 200 == 0 and kb != 0:
                        _log(log, f"[INFO] Downloading... {kb} KB received")

    _log(log, f"[OK] Download complete ({downloaded // 1024} KB)")

    _log(log, "[INFO] Extracting archive...")
    temp_dir = Path(tempfile.mkdtemp(prefix=f"{arxiv_id}_", dir=str(downloads_dir)))
    try:
        with tarfile.open(temp_tar, 'r:gz') as tar:
            members = tar.getmembers()
            for i, member in enumerate(members):
                if not _is_safe_tar_member(member, temp_dir):
                    _log(log, f"[WARN] Skipping suspicious path: {member.name}")
                    continue
                tar.extract(member, temp_dir)
                if len(members) > 0 and (i + 1) % max(1, len(members) // 5) == 0:
                    pct = int((i + 1) / len(members) * 100)
                    _log(log, f"[INFO] Extracting... {pct}% ({i + 1}/{len(members)} files)")
    except Exception as e:
        _log(log, f"[ERROR] Extraction failed: {e}")
        return None, None

    paper_dir.mkdir(parents=True, exist_ok=True)
    for item in temp_dir.iterdir():
        dest = paper_dir / item.name
        if dest.exists():
            try:
                shutil.rmtree(dest) if dest.is_dir() else dest.unlink()
            except Exception:
                suffix = 1
                while (downloads_dir / f"{folder_name}_fresh{suffix}").exists():
                    suffix += 1
                folder_name = f"{folder_name}_fresh{suffix}"
                paper_dir = downloads_dir / folder_name
                paper_dir.mkdir(parents=True, exist_ok=True)
                _log(log, f"[WARN] Existing cache item locked; using new cache dir: {folder_name}")
                dest = paper_dir / item.name
        shutil.move(str(item), str(dest))

    shutil.rmtree(temp_dir, ignore_errors=True)
    temp_tar.unlink()
    _log(log, f"[OK] Source saved to: {paper_dir}")
    return paper_dir, folder_name


# ── TeX Extraction ────────────────────────────────────────────

def expand_inputs(tex_content, base_dir, _seen=None, root_dir=None):
    """Recursively expand all \\input{} and \\include{} commands."""
    tex_content = _strip_latex_comments(tex_content)
    if _seen is None:
        _seen = set()
    if root_dir is None:
        root_dir = base_dir

    def replace_input(match):
        filename = match.group(1).strip()
        # Try original name first, then add .tex if it doesn't exist
        candidates = [
            (base_dir / filename).resolve(),
            (root_dir / filename).resolve(),
        ]
        if not filename.endswith('.tex'):
            filename_with_tex = filename + '.tex'
            candidates.extend([
                (base_dir / filename_with_tex).resolve(),
                (root_dir / filename_with_tex).resolve(),
            ])

        filepath = None
        for candidate in candidates:
            if candidate.exists() and candidate.is_file():
                filepath = candidate
                break

        if filepath is None:
            return match.group(0)
        if filepath in _seen:
            return match.group(0)

        try:
            sub = filepath.read_text(encoding='utf-8', errors='ignore')
        except Exception:
            return match.group(0)

        _seen.add(filepath)
        return expand_inputs(sub, filepath.parent, _seen, root_dir)

    return re.sub(r'\\(?:input|include)\s*\{([^}]+)\}', replace_input, tex_content)


def find_main_tex(paper_dir):
    """Find the main .tex file (contains \\documentclass)."""
    if (paper_dir / "main.tex").exists():
        return paper_dir / "main.tex"
    for tex_file in paper_dir.glob("*.tex"):
        try:
            content = tex_file.read_text(encoding='utf-8', errors='ignore')
            if r'\documentclass' in content:
                return tex_file
        except Exception:
            continue
    return None


def extract_body_and_appendix(tex_content):
    """Extract body and appendix. Returns (body, appendix, error)."""
    abstract_match = re.search(r'\\begin\{abstract\}', tex_content)
    first_section_match = re.search(r'\\section\s*[\*]?\s*\{', tex_content)
    doc_start_match = re.search(r'\\begin\{document\}', tex_content)

    if abstract_match and first_section_match:
        start = min(abstract_match.start(), first_section_match.start())
    elif abstract_match:
        start = abstract_match.start()
    elif first_section_match:
        start = first_section_match.start()
    elif doc_start_match:
        start = doc_start_match.end()
    else:
        return None, None, "Could not find abstract, first section, or document start"

    conclusion_matches = list(re.finditer(
        r'\\section\*?\s*\{[^}]*(?:[Cc]onclusion|[Ss]ummary)[^}]*\}', tex_content
    ))

    appendix_sep = re.search(
        r'\\appendix(?:\s|$)|\\begin\{appendix\}',
        tex_content
    )
    bibliography_sep = re.search(
        r'\\bibliography(?:style)?\s*\{',
        tex_content
    )

    candidates = []
    if appendix_sep and appendix_sep.start() > start:
        candidates.append(appendix_sep.start())
    if bibliography_sep and bibliography_sep.start() > start:
        candidates.append(bibliography_sep.start())

    if candidates:
        body_end = min(candidates)
    elif conclusion_matches:
        conclusion_start = conclusion_matches[-1].start()
        after = tex_content[conclusion_start + 1:]
        next_sec = re.search(r'\\(?:section|chapter)\s*[\*]?\s*\{', after)
        if next_sec:
            body_end = conclusion_start + 1 + next_sec.start()
        else:
            end_doc = re.search(r'\\end\{document\}', tex_content[conclusion_start:])
            body_end = conclusion_start + (end_doc.start() if end_doc else len(tex_content[conclusion_start:]))
    else:
        end_doc = re.search(r'\\end\{document\}', tex_content)
        body_end = end_doc.start() if end_doc else len(tex_content)

    body = tex_content[start:body_end].strip()

    after_body = tex_content[body_end:]
    end_doc = re.search(r'\\end\{document\}', after_body)
    app_end = end_doc.start() if end_doc else len(after_body)
    appendix_raw = after_body[:app_end].strip()

    appendix_cleaned = re.sub(r'\\bibliography(?:style)?\s*\{[^}]*\}', '', appendix_raw).strip()
    appendix_cleaned = re.sub(r'\\clearpage', '', appendix_cleaned).strip()
    appendix = appendix_cleaned if len(appendix_cleaned) > 50 else None

    return body, appendix, None


def extract_body_from_dir(paper_dir, output_dir, folder_name, log: LogFn | None = None):
    """Extract body and appendix from paper directory."""
    main_tex = find_main_tex(paper_dir)
    if not main_tex:
        _log(log, "[ERROR] Could not find main .tex file")
        return None
    _log(log, f"[INFO] Main file: {main_tex.name}")

    if not _all_inputs_readable(main_tex, paper_dir):
        _log(log, "[ERROR] Required \\input/\\include files are missing or unreadable")
        return None

    content = main_tex.read_text(encoding='utf-8', errors='ignore')
    _log(log, "[INFO] Expanding \\input references...")
    expanded = expand_inputs(content, paper_dir)

    if re.search(r'\\(?:input|include)\s*\{', expanded):
        _log(log, "[ERROR] Input expansion incomplete; unresolved \\input/\\include remains")
        return None

    _log(log, "[INFO] Parsing body (abstract → conclusion)...")
    body, appendix, err = extract_body_and_appendix(expanded)
    if err:
        _log(log, f"[ERROR] {err}")
        return None

    out_dir = output_dir / folder_name
    out_dir.mkdir(parents=True, exist_ok=True)

    out_path = out_dir / "body.tex"
    out_path.write_text(body, encoding='utf-8')
    _log(log, f"[OK] body.tex saved ({len(body)} chars, {body.count(chr(10))} lines)")

    if appendix:
        app_path = out_dir / "appendix.tex"
        app_path.write_text(appendix, encoding='utf-8')
        _log(log, f"[OK] appendix.tex saved ({len(appendix)} chars, {appendix.count(chr(10))} lines)")
    else:
        _log(log, "[INFO] No appendix detected")

    return out_path
