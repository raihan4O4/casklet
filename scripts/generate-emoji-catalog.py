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
picker_values = []
for line in source.splitlines():
    if line.startswith("# group: "):
        group = line[9:]
    match = re.match(r"^([0-9A-F ]+)\s*;\s*fully-qualified\s*#.+?E([0-9.]+)\s+(.+)$", line)
    if match:
        value = "".join(chr(int(c, 16)) for c in match[1].split())
        rows.append([value, match[3], group])
        # Default yellow variants only. Keep a conservative Windows baseline;
        # region/tag flags and standalone modifiers do not render as icons there.
        if (
            float(match[2]) <= 13.0
            and group != "Component"
            and not any(0x1F3FB <= ord(c) <= 0x1F3FF for c in value)
            and not any(0x1F1E6 <= ord(c) <= 0x1F1FF for c in value)
            and not any(0xE0020 <= ord(c) <= 0xE007F for c in value)
        ):
            picker_values.append(value)
output = pathlib.Path(__file__).resolve().parents[1] / "apps/desktop/src/app/emoji-data.json"
if not output.exists() or json.loads(output.read_text(encoding="utf-8")) != rows:
    output.write_text(json.dumps(rows, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
picker_output = output.with_name("emoji-picker-data.json")
if not picker_output.exists() or json.loads(picker_output.read_text(encoding="utf-8")) != picker_values:
    picker_output.write_text(json.dumps(picker_values, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(f"Generated {len(picker_values)} default-color picker emoji")
print(f"Generated {len(rows)} fully-qualified emoji")
