"""Test: extract arXiv ID from a local PDF."""
import re
import fitz  # pymupdf


def extract_arxiv_id_from_pdf(pdf_path: str) -> str | None:
    doc = fitz.open(pdf_path)

    # Strategy 1: Check PDF metadata
    meta = doc.metadata or {}
    for key in ("subject", "keywords", "title", "author"):
        val = meta.get(key, "") or ""
        m = re.search(r'(\d{4}\.\d{4,5})', val)
        if m:
            print(f"  [meta:{key}] found: {m.group(1)}")
            return m.group(1)

    # Strategy 2: Scan first page text for arXiv watermark
    if doc.page_count > 0:
        page = doc[0]
        text = page.get_text()
        # arXiv puts "arXiv:YYMM.NNNNN" watermark on first page
        m = re.search(r'arXiv[:\s]*(\d{4}\.\d{4,5})', text)
        if m:
            print(f"  [page0 watermark] found: {m.group(1)}")
            return m.group(1)
        # Sometimes it's just the ID somewhere on the page
        m = re.search(r'(\d{4}\.\d{4,5})', text)
        if m:
            print(f"  [page0 text] found: {m.group(1)}")
            return m.group(1)

    # Strategy 3: Scan all pages (slower, fallback)
    for i in range(min(doc.page_count, 3)):
        text = doc[i].get_text()
        m = re.search(r'arXiv[:\s]*(\d{4}\.\d{4,5})', text)
        if m:
            print(f"  [page{i}] found: {m.group(1)}")
            return m.group(1)

    doc.close()
    return None


if __name__ == "__main__":
    pdf = r"D:\Research\want_to_read\geoalign.pdf"
    print(f"Testing: {pdf}")
    result = extract_arxiv_id_from_pdf(pdf)
    print(f"Result: {result}")
    print(f"Expected: 2604.12630")
    print(f"Match: {result == '2604.12630' if result else False}")
