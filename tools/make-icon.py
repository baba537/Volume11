"""Generate the Volume11 icon.

A level meter: four ascending rounded bars on a dark rounded tile, the tallest
picked out in the application's accent grey. Chosen because bars stay legible at
16 px in the notification area, where a speaker cone or a slider turns to mush.

Drawn at 16x the target size and downsampled, which is cheaper than writing an
anti-aliased rasteriser and gives cleaner edges than drawing small directly.
"""
import struct
from PIL import Image, ImageDraw

SS = 16          # supersampling factor
BASE = 64        # design grid

TILE = (0x15, 0x17, 0x1B, 255)      # near-black tile
EDGE = (0x2E, 0x33, 0x3A, 255)      # subtle rim so it reads on black backgrounds
BAR = (0xE8, 0xEA, 0xEE, 255)       # near-white bars
ACCENT = (0x9A, 0xA3, 0xAD, 255)    # the application's grey accent

# x position, height, in design-grid units. Ascending, last one accented.
# The group is centred in the tile interior (2..62): four 7-wide bars on an
# 11-unit pitch span 40 units, and the tallest is 34, so the baseline sits at
# 32 + 34/2 to leave equal air above and below.
BARS = [(12, 13), (23, 20), (34, 27), (45, 34)]
BAR_W = 7


def render(size: int) -> Image.Image:
    big = size * SS
    scale = big / BASE
    image = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    inset = 2 * scale
    radius = 13 * scale
    draw.rounded_rectangle(
        [inset, inset, big - inset, big - inset],
        radius=radius,
        fill=TILE,
        outline=EDGE,
        width=max(1, int(1.2 * scale)),
    )

    baseline = 49 * scale
    for index, (x, height) in enumerate(BARS):
        left = x * scale
        right = (x + BAR_W) * scale
        top = baseline - height * scale
        colour = ACCENT if index == len(BARS) - 1 else BAR
        draw.rounded_rectangle(
            [left, top, right, baseline],
            radius=(BAR_W * scale) / 2,
            fill=colour,
        )

    return image.resize((size, size), Image.LANCZOS)


def dib_frame(image: Image.Image) -> bytes:
    """32bpp BGRA bottom-up plus a 1bpp AND mask.

    Windows only decodes PNG-compressed icon frames reliably at 256x256; at tray
    sizes they come out as a monochrome blob, so every frame is a classic DIB.
    """
    width, height = image.size
    pixels = image.load()

    header = struct.pack(
        "<IiiHHIIiiII",
        40, width, height * 2, 1, 32, 0, 0, 0, 0, 0, 0,
    )

    xor = bytearray()
    for y in range(height - 1, -1, -1):
        for x in range(width):
            r, g, b, a = pixels[x, y]
            xor += bytes((b, g, r, a))

    row_bytes = ((width + 31) // 32) * 4
    mask = bytearray()
    for y in range(height - 1, -1, -1):
        row = bytearray(row_bytes)
        for x in range(width):
            if pixels[x, y][3] == 0:
                row[x // 8] |= 0x80 >> (x % 8)
        mask += row

    return header + bytes(xor) + bytes(mask)


SIZES = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256]
frames = [(s, dib_frame(render(s))) for s in SIZES]

out = bytearray(struct.pack("<HHH", 0, 1, len(frames)))
offset = 6 + 16 * len(frames)
for size, data in frames:
    out += struct.pack(
        "<BBBBHHII",
        0 if size >= 256 else size, 0 if size >= 256 else size,
        0, 0, 1, 32, len(data), offset,
    )
    offset += len(data)
for _, data in frames:
    out += data

open("assets/icon.ico", "wb").write(bytes(out))
print("wrote assets/icon.ico", len(out), "bytes")

render(256).save("docs/logo.png")
print("wrote docs/logo.png")
