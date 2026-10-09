"""Signed-distance mask of the wordmark, in the shader-mode encoding.

1920x1080 RGB PNG. Distance d in pixels (negative inside the glyphs), clamped to
+-256, stored as the 16-bit value (d / 512 + 0.5) * 65535 split over R (high byte)
and G (low byte). `maskDistance` in common.sksl decodes it.
"""
import sys
import numpy as np
from PIL import Image, ImageDraw, ImageFont
from scipy.ndimage import distance_transform_edt

font_path, out, text = sys.argv[1], sys.argv[2], sys.argv[3]
target_width = float(sys.argv[4]) if len(sys.argv) > 4 else 900.0
W, H, SS = 1920, 1080, 4  # supersample for a smooth edge

size = 200
font = ImageFont.truetype(font_path, size * SS)
box = font.getbbox(text)
scale = target_width * SS / (box[2] - box[0])
font = ImageFont.truetype(font_path, int(size * SS * scale))
box = font.getbbox(text)
img = Image.new("L", (W * SS, H * SS), 0)
draw = ImageDraw.Draw(img)
x = (W * SS - (box[2] - box[0])) / 2 - box[0]
y = (H * SS - (box[3] - box[1])) / 2 - box[1]
draw.text((x, y), text, font=font, fill=255)

inside = np.asarray(img) > 127
outside_d = distance_transform_edt(~inside)
inside_d = distance_transform_edt(inside)
d = (outside_d - inside_d) / SS
d = d.reshape(H, SS, W, SS).mean(axis=(1, 3))
d = np.clip(d, -256.0, 255.99)
u = np.round((d / 512.0 + 0.5) * 65535.0).astype(np.uint32)
rgb = np.zeros((H, W, 3), dtype=np.uint8)
rgb[..., 0] = (u >> 8).astype(np.uint8)
rgb[..., 1] = (u & 255).astype(np.uint8)
Image.fromarray(rgb).save(out)
print(out, "d range", d.min(), d.max())
