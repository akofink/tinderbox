from pathlib import Path
from PIL import Image

image = Image.open(Path("web-smoke-canvas.png")).convert("RGB")
sample = image.resize((min(160, image.width), min(120, image.height)))
colors = sample.getcolors(maxcolors=sample.width * sample.height)
color_count = len(colors) if colors else sample.width * sample.height + 1
if image.width < 100 or image.height < 100 or color_count < 12:
    raise SystemExit(
        f"canvas did not draw a varied frame: {image.width}x{image.height}, {color_count} sampled colors"
    )
print(f"Canvas drew {image.width}x{image.height} with {color_count} sampled colors")
