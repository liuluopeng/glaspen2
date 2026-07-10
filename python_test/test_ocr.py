"""
Verify PP-OCRv6 with PaddleOCR Python package + ONNX Runtime.

Downloads models automatically from HuggingFace on first run.
"""
import sys
from pathlib import Path

try:
    from paddleocr import PaddleOCR
except ImportError:
    print("[setup] Installing PaddleOCR and dependencies...")
    import subprocess
    subprocess.check_call(
        [sys.executable, "-m", "pip", "install", "-r", "requirements.txt"]
    )
    from paddleocr import PaddleOCR


def test_image(path: str, label: str):
    """Run PP-OCRv6 on a single image and print results."""
    if not Path(path).exists():
        print(f"[{label}] SKIP: {path} not found")
        return

    print(f"\n{'='*60}")
    print(f"[{label}] Testing: {path}")
    print(f"{'='*60}")

    ocr = PaddleOCR(
        text_detection_model_name="PP-OCRv6_medium_det",
        text_recognition_model_name="PP-OCRv6_medium_rec",
        engine="onnxruntime",
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=True,
    )

    result = ocr.predict(path)

    if not result:
        print(f"[{label}] No results")
        return

    for page_num, page in enumerate(result):
        print(f"\n--- Page {page_num} ---")
        # Try structured extraction first
        extracted = []
        try:
            data = page.json if hasattr(page, 'json') else None
            if data and isinstance(data, list):
                for item in data:
                    if isinstance(item, list):
                        for region in item:
                            if isinstance(region, list) and len(region) >= 2:
                                txt = region[1][0] if isinstance(region[1], list) else ""
                                conf = region[1][1] if isinstance(region[1], list) and len(region[1]) > 1 else 1.0
                                extracted.append((txt, conf))
                    elif isinstance(item, dict):
                        txt = item.get("text", item.get("transcription", ""))
                        conf = item.get("confidence", item.get("score", 0))
                        extracted.append((txt, conf))
            elif isinstance(data, dict):
                for k, v in data.items():
                    print(f"  {k}: {v}")
        except Exception:
            pass

        if extracted:
            for txt, conf in extracted:
                print(f"  '{txt}'  (conf={conf:.3f})")
        else:
            # Fallback: print raw
            print(f"  (raw) {str(page)[:500]}")

        # Save outputs
        try:
            out_dir = Path("output")
            out_dir.mkdir(exist_ok=True)
            page.save_to_img(str(out_dir))
            page.save_to_json(str(out_dir))
            print(f"  Saved to {out_dir}/")
        except Exception as e:
            print(f"  Save failed: {e}")


if __name__ == '__main__':
    test_image("test_handwritten_synthetic.png", "synthetic")
    test_image("test_handwritten_from_db.png", "from_db")

    # Also test any other PNGs in directory
    for f in sorted(Path('.').glob('*.png')):
        if f.name not in ('test_handwritten_synthetic.png', 'test_handwritten_from_db.png'):
            test_image(str(f), f.stem)
