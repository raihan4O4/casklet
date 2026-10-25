"""Regenerate the bundled catalog from Unicode 18.0 emoji-test.txt.
Download https://www.unicode.org/Public/18.0.0/emoji/emoji-test.txt,
then run: python scripts/generate-emoji-catalog.py <downloaded-file>
Unicode License V3: LICENSES/Unicode-LICENSE.txt.
"""
import json
import pathlib
import re
import sys

source = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
if "# Version: 18.0" not in source:
    raise ValueError("Expected Unicode Emoji 18.0 data")
group = ""
rows = []
for line in source.splitlines():
    if line.startswith("# group: "):
        group = line[9:]
    match = re.match(r"^([0-9A-F ]+)\s*;\s*fully-qualified\s*#.+?E[0-9.]+\s+(.+)$", line)
    if match:
        rows.append(["".join(chr(int(c, 16)) for c in match[1].split()), match[2], group])
output = pathlib.Path(__file__).resolve().parents[1] / "apps/desktop/src/app/emoji-data.json"
output.write_text(json.dumps(rows, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
print(f"Generated {len(rows)} fully-qualified emoji")
