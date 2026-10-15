from pathlib import Path

from PIL import Image


def yellow_canvas_pixels(path: Path) -> int:
    image = Image.open(path).convert("RGB")
    canvas = image.crop((176, 149, 390, 844))
    return sum(
        red > 180 and green > 180 and blue < 100
        for red, green, blue in canvas.get_flattened_data()
    )


inspected = yellow_canvas_pixels(Path("tmp/mobile-touch-smoke/inspected.png"))
dismissed = yellow_canvas_pixels(Path("tmp/mobile-touch-smoke/dismissed.png"))
# The Inspector marks its selected cell with a bright yellow circle. The baseline
# scene and ordinary toolbar do not contain yellow marks in this canvas region.
if inspected < 10:
    raise SystemExit(f"touch inspection marker missing: found {inspected} yellow canvas pixels")
if dismissed >= 10:
    raise SystemExit(f"Inspector did not dismiss: found {dismissed} yellow canvas pixels")
narrow = Image.open(Path("tmp/mobile-touch-smoke/narrow.png")).convert("RGB")
canvas_edge_pixels = sum(
    red < 45 and green < 45 and blue < 70
    for red, green, blue in narrow.crop((128, 350, 168, 390)).get_flattened_data()
)
if canvas_edge_pixels < 100:
    raise SystemExit(
        f"narrow-screen canvas did not expand beside the compact palette: {canvas_edge_pixels} dark pixels"
    )
print(
    f"Mobile Inspector shown ({inspected}px) and dismissed ({dismissed}px); "
    f"narrow canvas edge verified ({canvas_edge_pixels}px)"
)
