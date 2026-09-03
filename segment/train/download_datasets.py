"""Download public saliency datasets for MeraSubject training."""

from __future__ import annotations

import argparse
import io
import zipfile
from pathlib import Path

import requests
from tqdm import tqdm

# DUTS mirrors occasionally move; override with --duts-url if needed.
DUTS_URL = "http://saliencydetection.net/duts/download/DUTS-TR.zip"
ECSSD_IMG = "https://www.cse.cuhk.edu.hk/leojia/projects/hsaliency/data/ECSSD/images.zip"
ECSSD_GT = "https://www.cse.cuhk.edu.hk/leojia/projects/hsaliency/data/ECSSD/ground_truth_mask.zip"


def fetch(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.exists() and dest.stat().st_size > 1000:
        print(f"✓ exists {dest}")
        return
    print(f"→ {url}")
    with requests.get(url, stream=True, timeout=120) as r:
        r.raise_for_status()
        total = int(r.headers.get("content-length", 0))
        with open(dest, "wb") as f, tqdm(total=total, unit="B", unit_scale=True) as bar:
            for chunk in r.iter_content(chunk_size=1 << 20):
                if chunk:
                    f.write(chunk)
                    bar.update(len(chunk))


def unzip(zpath: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zpath) as z:
        z.extractall(out)
    print(f"✓ extracted → {out}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=Path("data"))
    ap.add_argument("--duts", action="store_true")
    ap.add_argument("--ecssd", action="store_true")
    ap.add_argument("--duts-url", default=DUTS_URL)
    args = ap.parse_args()
    if not args.duts and not args.ecssd:
        args.duts = args.ecssd = True

    root = args.out
    root.mkdir(parents=True, exist_ok=True)

    if args.duts:
        z = root / "DUTS-TR.zip"
        try:
            fetch(args.duts_url, z)
            unzip(z, root / "DUTS-TR")
        except Exception as e:
            print(f"! DUTS download failed ({e}). Place DUTS-TR manually under {root}/DUTS-TR")

    if args.ecssd:
        try:
            zi = root / "ecssd_images.zip"
            zg = root / "ecssd_gt.zip"
            fetch(ECSSD_IMG, zi)
            fetch(ECSSD_GT, zg)
            unzip(zi, root / "ECSSD" / "images")
            unzip(zg, root / "ECSSD" / "masks")
        except Exception as e:
            print(f"! ECSSD download failed ({e}). Place images/masks under {root}/ECSSD")

    print("Done. Point configs at data/ and run train.py")


if __name__ == "__main__":
    main()
