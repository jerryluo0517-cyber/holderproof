#!/usr/bin/env python3
"""Build PDF counterparts from the Markdown tokens emitted by build_docs.mjs.

Run the Node builder first. Dependencies: scripts/requirements-docs.txt.
Set HOLDERPROOF_DOC_FONT to a TrueType font covering Chinese when the macOS
Arial Unicode font is unavailable; otherwise the standard STSong CID font is used.
"""

import html
import json
import os
from pathlib import Path
from typing import Any

from reportlab.lib import colors
from reportlab.lib.enums import TA_LEFT
from reportlab.lib.styles import ParagraphStyle
from reportlab.lib.utils import ImageReader
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.cidfonts import UnicodeCIDFont
from reportlab.pdfbase.ttfonts import TTFont
from reportlab.platypus import (
    Image, KeepTogether, Paragraph, SimpleDocTemplate, Spacer, Table, TableStyle,
)

ROOT = Path(__file__).resolve().parents[1]
WIDTH = 511
NAVY = colors.HexColor("#183153")
TEAL = colors.HexColor("#0f766e")
MUTED = colors.HexColor("#52657b")
FONT_PATH = Path(os.environ.get(
    "HOLDERPROOF_DOC_FONT", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"
))
if FONT_PATH.exists():
    pdfmetrics.registerFont(TTFont("DocSans", str(FONT_PATH)))
else:
    pdfmetrics.registerFont(UnicodeCIDFont("STSong-Light"))
    pdfmetrics.registerFontFamily("DocSans", normal="STSong-Light", bold="STSong-Light")
    # Paragraph styles refer directly to the registered CID name below.
FONT = "DocSans" if FONT_PATH.exists() else "STSong-Light"
pdfmetrics.registerFontFamily(FONT, normal=FONT, bold=FONT, italic=FONT, boldItalic=FONT)

STYLES = {
    "body": ParagraphStyle("Body", fontName=FONT, fontSize=9.2, leading=14.5,
                           textColor=colors.HexColor("#243449"), spaceAfter=5,
                           wordWrap="CJK", allowWidows=0, allowOrphans=0),
    "title": ParagraphStyle("Title", fontName=FONT, fontSize=23, leading=32,
                            textColor=NAVY, spaceBefore=4, spaceAfter=15, keepWithNext=True),
    "heading": ParagraphStyle("Heading", fontName=FONT, fontSize=14, leading=21,
                              textColor=NAVY, spaceBefore=12, spaceAfter=7, keepWithNext=True),
    "cell": ParagraphStyle("Cell", fontName=FONT, fontSize=8.3, leading=13,
                           textColor=colors.HexColor("#243449"), wordWrap="CJK"),
    "code": ParagraphStyle("Code", fontName="Courier", fontSize=7.5, leading=11.5,
                           textColor=NAVY, backColor=colors.HexColor("#f1f5f9"),
                           borderPadding=9, spaceBefore=6, spaceAfter=12, wordWrap="CJK"),
    "note": ParagraphStyle("Note", fontName=FONT, fontSize=8, leading=12,
                           textColor=MUTED, spaceAfter=8, wordWrap="CJK"),
}


def inline(tokens: list[dict[str, Any]]) -> str:
    """Convert only the safe inline subset used by these local source documents."""
    fragments = []
    for token in tokens:
        kind = token["type"]
        text = html.escape(token.get("text", ""))
        if kind == "strong":
            fragments.append(f"<b>{inline(token['tokens'])}</b>")
        elif kind == "em":
            fragments.append(f"<i>{inline(token['tokens'])}</i>")
        elif kind == "codespan":
            fragments.append(f'<font face="Courier" size="8">{text}</font>')
        elif kind == "link":
            target = html.escape(token["href"], quote=True)
            fragments.append(f'<link href="{target}" color="#0f766e">{inline(token["tokens"])}</link>')
        elif kind == "br":
            fragments.append("<br/>")
        elif "tokens" in token:
            fragments.append(inline(token["tokens"]))
        else:
            fragments.append(text)
    return "".join(fragments)


def diagram() -> list[Any]:
    path = ROOT / "docs/architecture/assets/holderproof-flow.png"
    width, height = ImageReader(str(path)).getSize()
    return [Image(str(path), width=WIDTH, height=WIDTH * height / width), Spacer(1, 9)]


def paragraphs(tokens: list[dict[str, Any]]) -> list[Any]:
    story = []
    for token in tokens:
        kind = token["type"]
        if kind == "heading":
            style = STYLES["title"] if token["depth"] == 1 else STYLES["heading"]
            story.append(Paragraph(inline(token["tokens"]), style))
        elif kind in ("paragraph", "text"):
            if any(item["type"] == "image" for item in token.get("tokens", [])):
                story.extend(diagram())
            else:
                story.append(Paragraph(inline(token.get("tokens", [])), STYLES["body"]))
        elif kind == "code":
            if token.get("lang") == "mermaid":
                story.append(Paragraph("可编辑时序图见 Markdown 中的 Mermaid 源码。", STYLES["note"]))
            else:
                lines = []
                for line in token["text"].splitlines():
                    escaped = html.escape(line).replace(" ", "&#160;")
                    if any("\u4e00" <= character <= "\u9fff" for character in line):
                        escaped = f'<font face="{FONT}">{escaped}</font>'
                    lines.append(escaped)
                code = "<br/>".join(lines)
                story.append(KeepTogether([Paragraph(code, STYLES["code"])]))
        elif kind == "table":
            rows = [token["header"], *token["rows"]]
            cells = [[Paragraph(inline(cell["tokens"]), STYLES["cell"]) for cell in row] for row in rows]
            columns = len(token["header"])
            widths = [WIDTH * .31] + [WIDTH * .69 / (columns - 1)] * (columns - 1)
            table = Table(cells, colWidths=widths, repeatRows=1, hAlign=TA_LEFT)
            table.setStyle(TableStyle([
                ("BACKGROUND", (0, 0), (-1, 0), colors.HexColor("#eff6ff")),
                ("ROWBACKGROUNDS", (0, 1), (-1, -1), [colors.white, colors.HexColor("#f8fafc")]),
                ("GRID", (0, 0), (-1, -1), .4, colors.HexColor("#cbd5e1")),
                ("VALIGN", (0, 0), (-1, -1), "TOP"),
                ("LEFTPADDING", (0, 0), (-1, -1), 8),
                ("RIGHTPADDING", (0, 0), (-1, -1), 8),
                ("TOPPADDING", (0, 0), (-1, -1), 6),
                ("BOTTOMPADDING", (0, 0), (-1, -1), 6),
            ]))
            story.extend([table, Spacer(1, 10)])
        elif kind == "list":
            for number, item in enumerate(token["items"], 1):
                prefix = f"{number}. " if token["ordered"] else "• "
                content = " ".join(inline(child.get("tokens", [])) for child in item["tokens"])
                story.append(Paragraph(prefix + content, STYLES["body"]))
    return story


def page_frame(canvas: Any, document: Any) -> None:
    canvas.saveState()
    canvas.setStrokeColor(TEAL)
    canvas.setLineWidth(2)
    canvas.line(42, 810, 553, 810)
    canvas.setFont("Helvetica", 8)
    canvas.setFillColor(MUTED)
    canvas.drawString(42, 823, "HOLDERPROOF  /  LOCAL FIXTURE P0  /  2026-11-15")
    canvas.drawString(42, 24, "2-of-3 committee  |  Airdrop + Governance  |  Current implementation")
    canvas.drawRightString(553, 24, str(document.page))
    canvas.restoreState()


def main() -> None:
    exports = json.loads((ROOT / ".run/docs/tokens.json").read_text())
    for item in exports:
        output = ROOT / item["pdf"]
        document = SimpleDocTemplate(
            str(output), pagesize=(595.28, 841.89),
            leftMargin=36, rightMargin=36, topMargin=49, bottomMargin=44,
            title=item["title"], author="HolderProof",
        )
        document.build(paragraphs(item["tokens"]), onFirstPage=page_frame, onLaterPages=page_frame)
        print(f"Generated {output.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
