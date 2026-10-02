#!/bin/bash
# SVG -> 1024x1024 PNG (headless Chrome) -> Tauri 全套位图图标
set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICONS="$ROOT/icons"
CHROME="/c/Program Files/Google/Chrome/Application/chrome.exe"

echo "[1/3] SVG -> PNG (headless chrome)"
"$CHROME" --headless=new --disable-gpu \
  --screenshot="$(cygpath -w "$ICONS/icon.png")" \
  --window-size=1024,1024 --default-background-color=00000000 \
  "file:///$(cygpath -m "$ICONS/icon.svg")"

echo "[2/3] 生成多尺寸 PNG + ICO"
uv run --with pillow python - "$ICONS" <<'PYEOF'
import sys
from PIL import Image
from pathlib import Path

icons = Path(sys.argv[1])
img = Image.open(icons / "icon.png").convert("RGBA")
assert img.size == (1024, 1024), f"unexpected size {img.size}"

for size, name in [(32, "32x32.png"), (128, "128x128.png"), (256, "128x128@2x.png")]:
    img.resize((size, size), Image.LANCZOS).save(icons / name)

img.resize((256, 256), Image.LANCZOS).save(
    icons / "icon.ico", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)]
)
print("icons written:", [p.name for p in sorted(icons.glob('*'))])
PYEOF

echo "[3/3] done"
