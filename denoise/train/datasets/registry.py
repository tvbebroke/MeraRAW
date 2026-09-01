"""Public denoise dataset registry — URLs, licenses, layout hints."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

Split = Literal["train", "val", "test"]


@dataclass(frozen=True)
class DatasetSpec:
    name: str
    kind: Literal["paired", "clean_only", "raw_pairs"]
    license: str
    url: str
    notes: str
    # Expected tree under MERANOISE_DATA_ROOT after manual/script download
    layout: str


REGISTRY: dict[str, DatasetSpec] = {
    "sidd": DatasetSpec(
        name="sidd",
        kind="paired",
        license="CC-BY-NC (research); check SIDD license before commercial ship",
        url="https://www.eecs.yorku.ca/~kamel/sidd/dataset.php",
        notes="Smartphone Image Denoising Dataset — real noisy/clean pairs (MAT blocks).",
        layout="sidd/SIDD_Medium_Raw/*.MAT or sidd/SIDD_Small_sRGB_Only/*.png pairs",
    ),
    "nind": DatasetSpec(
        name="nind",
        kind="paired",
        license="MIT (nind-denoise upstream); verify NIND redistribution terms",
        url="https://github.com/m-tassano/nind-denoise",
        notes="Natural Image Noise Dataset — same scene low/high ISO JPEG pairs.",
        layout="nind/NIND/*.jpg (paired filenames)",
    ),
    "dnd": DatasetSpec(
        name="dnd",
        kind="paired",
        license="CC-BY-NC (DND benchmark)",
        url="https://noise.visinf.tu-darmstadt.de/downloads/",
        notes="Darmstadt Noise Dataset — real camera noise benchmark.",
        layout="dnd/original_png/ + dnd/noisy_png/",
    ),
    "sid": DatasetSpec(
        name="sid",
        kind="raw_pairs",
        license="MIT (SID paper code release)",
        url="https://github.com/cchen156/Learning-to-See-in-the-Dark",
        notes="See-in-the-Dark — Sony/Fuji short/long exposure RAW pairs.",
        layout="sid/Sony/{short,long}/*.ARW and sid/Fuji/{short,long}/*.RAF",
    ),
    "polyu": DatasetSpec(
        name="polyu",
        kind="paired",
        license="Research use; check PolyU real-noise dataset terms",
        url="https://github.com/csjunxu/PolyU-Real-World-Noisy-Images-Dataset",
        notes="Real-world noisy images with mean clean references.",
        layout="polyu/OriginalImages/ + polyu/CroppedImages/",
    ),
    "div2k": DatasetSpec(
        name="div2k",
        kind="clean_only",
        license="DIV2K license (research)",
        url="https://data.vision.ee.ethz.ch/cvl/DIV2K/",
        notes="High-quality clean images — synthetic Poisson–Gaussian noise injection.",
        layout="div2k/DIV2K_train_HR/*.png",
    ),
    "flickr2k": DatasetSpec(
        name="flickr2k",
        kind="clean_only",
        license="Research",
        url="https://github.com/limbee/NTIRE2017",
        notes="Flickr2K clean set for synthetic noise augmentation.",
        layout="flickr2k/Flickr2K/*.png",
    ),
    "fivek": DatasetSpec(
        name="fivek",
        kind="clean_only",
        license="MIT-Adobe FiveK (research)",
        url="https://data.csail.mit.edu/graphics/fivek/",
        notes="Retouched clean masters — strong for texture preservation training.",
        layout="fivek/raw_photos/ or exported TIFF masters",
    ),
}


def list_datasets() -> list[str]:
    return sorted(REGISTRY.keys())
