import os
from pathlib import Path

from PIL import Image


output = Path(os.environ.get("TINDERBOX_TOUCH_OUTPUT_DIR", "tmp/mobile-touch-smoke"))


def yellow_canvas_pixels(path: Path) -> int:
    image = Image.open(path).convert("RGB")
    canvas = image.crop((176, 149, 390, 844))
    return sum(
        red > 180 and green > 180 and blue < 100
        for red, green, blue in canvas.get_flattened_data()
    )


inspected = yellow_canvas_pixels(output / "inspected.png")
dismissed = yellow_canvas_pixels(output / "dismissed.png")
# The Inspector marks its selected cell with a bright yellow circle. The baseline
# scene and ordinary toolbar do not contain yellow marks in this canvas region.
if inspected < 10:
    raise SystemExit(f"touch inspection marker missing: found {inspected} yellow canvas pixels")
if dismissed >= 10:
    raise SystemExit(f"Inspector did not dismiss: found {dismissed} yellow canvas pixels")
narrow = Image.open(output / "narrow.png").convert("RGB")
# Fixed worlds now retain their aspect ratio instead of expanding to fill a phone's
# height. Sample a known terrain strip in the narrow canvas below the 44 px toolbar.
canvas_world_pixels = sum(
    70 < red < 150 and 70 < green < 160 and 80 < blue < 180
    for red, green, blue in narrow.crop((128, 120, 168, 230)).get_flattened_data()
)
if canvas_world_pixels < 100:
    raise SystemExit(
        f"narrow-screen world canvas was not visible below the toolbar: {canvas_world_pixels} pixels"
    )
print(
    f"Mobile Inspector shown ({inspected}px) and dismissed ({dismissed}px); "
    f"narrow canvas visible below toolbar ({canvas_world_pixels}px)"
)
