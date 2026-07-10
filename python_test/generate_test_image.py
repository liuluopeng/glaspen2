"""Generate test images with handwritten text for OCR testing."""
import numpy as np
from PIL import Image, ImageDraw, ImageFont
import sqlite3
import struct
from pathlib import Path


def draw_handwritten_text(text: str, font_size: int = 40) -> Image.Image:
    """Create a white image with simulated handwritten text."""
    padding = 20
    line_height = font_size + 10
    lines = text.split('\n')
    img_w = max(len(line) for line in lines) * (font_size // 2) + padding * 2
    img_h = len(lines) * line_height + padding * 2

    img = Image.new('RGB', (int(img_w), int(img_h)), 'white')
    draw = ImageDraw.Draw(img)

    # Try to find a font that works
    font = None
    font_paths = [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Helvetica.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ]
    for fp in font_paths:
        if Path(fp).exists():
            try:
                font = ImageFont.truetype(fp, font_size)
                break
            except Exception:
                continue
    if font is None:
        font = ImageFont.load_default()

    for i, line in enumerate(lines):
        y = padding + i * line_height
        draw.text((padding, y), line, fill='black', font=font)

    return img


def render_strokes_from_db(db_path: str, screen_id: int = None) -> Image.Image:
    """Render strokes from a glaspen2 SQLite database to an image."""
    conn = sqlite3.connect(f'file:{db_path}?mode=ro', uri=True)
    cur = conn.cursor()

    # Get latest screen with points if not specified
    if screen_id is None:
        cur.execute("""
            SELECT s.id FROM screens s
            WHERE (SELECT COUNT(*) FROM points WHERE stroke_id IN (SELECT id FROM strokes WHERE screen_id = s.id)) > 0
            ORDER BY s.id DESC LIMIT 1
        """)
        row = cur.fetchone()
        if row is None:
            conn.close()
            print("[gen] No screens with data found")
            return None
        screen_id = row[0]

    print(f"[gen] Loading strokes from screen {screen_id}")

    # Get screen dimensions
    cur.execute("SELECT screen_w, screen_h FROM screens WHERE id = ?", (screen_id,))
    srow = cur.fetchone()
    screen_w, screen_h = srow or (1920, 1080)

    # Get all strokes for this screen
    cur.execute(
        "SELECT id, color_r, color_g, color_b, width_scale FROM strokes WHERE screen_id = ? ORDER BY id",
        (screen_id,)
    )
    strokes = cur.fetchall()

    # Get points for each stroke
    all_points = []
    bbox = [float('inf'), float('inf'), float('-inf'), float('-inf')]

    for sid, r, g, b, ws in strokes:
        cur.execute(
            "SELECT x, y, width, t FROM points WHERE stroke_id = ? ORDER BY seq",
            (sid,)
        )
        pts = cur.fetchall()
        if not pts:
            continue
        for x, y, w, t in pts:
            all_points.append((sid, r, g, b, x, y, w))
            bbox[0] = min(bbox[0], x)
            bbox[1] = min(bbox[1], y)
            bbox[2] = max(bbox[2], x)
            bbox[3] = max(bbox[3], y)

    conn.close()

    if not all_points:
        print(f"[gen] No points found for screen {screen_id}")
        return None

    pad = 30
    crop_x = max(0, int(bbox[0]) - pad)
    crop_y = max(0, int(bbox[1]) - pad)
    crop_w = int(bbox[2] - bbox[0] + pad * 2)
    crop_h = int(bbox[3] - bbox[1] + pad * 2)

    print(f"[gen] {len(all_points)} points, bbox=({crop_w}x{crop_h}) @ ({crop_x},{crop_y})")

    # Render to image
    img = Image.new('RGB', (crop_w, crop_h), 'white')
    pixels = img.load()

    # Group points by stroke
    from collections import defaultdict
    strokes_pixels = defaultdict(list)
    for sid, r, g, b, x, y, w in all_points:
        px = int(x - crop_x)
        py = int(y - crop_y)
        cr = int(r * 255)
        cg = int(g * 255)
        cb = int(b * 255)
        strokes_pixels[sid].append((px, py, cr, cg, cb, int(max(w, 1))))

    for sid, pts in strokes_pixels.items():
        draw = ImageDraw.Draw(img)
        for i in range(len(pts)):
            x, y, cr, cg, cb, w = pts[i]
            if i > 0:
                px, py, _, _, _, _ = pts[i - 1]
                draw.line([(px, py), (x, y)], fill=(cr, cg, cb), width=max(w, 1))
            # Draw dot for the first point or isolated points
            r2 = max(w // 2, 1)
            draw.ellipse([x - r2, y - r2, x + r2, y + r2], fill=(cr, cg, cb))

    scale = 2.0
    img_large = img.resize((int(crop_w * scale), int(crop_h * scale)), Image.LANCZOS)
    print(f"[gen] Rendered {crop_w}x{crop_h} (scaled to {img_large.size})")
    return img_large


def save_image(img: Image.Image, path: str):
    """Save image and print info."""
    img.save(path)
    print(f"[gen] Saved {path} ({img.size[0]}x{img.size[1]})")


if __name__ == '__main__':
    import sys

    # 1. Generate synthetic handwritten text
    img1 = draw_handwritten_text("你好世界\nHello World\n测试中文手写识别")
    save_image(img1, "test_handwritten_synthetic.png")

    # 2. Try to load from DB
    db_paths = [
        "../target/debug/glaspen2.db",
        str(Path.home() / "Library/Application Support/glaspen2/glaspen2.db"),
    ]
    for db_path in db_paths:
        if Path(db_path).exists():
            print(f"\n[gen] Found DB: {db_path}")
            img2 = render_strokes_from_db(db_path)
            if img2:
                save_image(img2, "test_handwritten_from_db.png")
            break
    else:
        print("[gen] No DB found, skipping DB render")
