# 02 — System Requirements

**Applies to:** Desktop v2.0 / Mobile v4.0 — **[BUILD: October 2018]**

## Purpose
Platform/runtime constraints that bound any reimplementation targeting parity.

## Constraints to honor
- **Desktop:** GPU-accelerated rendering pipeline; Adobe lists a supported-GPU
  requirement for the editing surface. A software fallback path is expected when GPU is
  unavailable.
- **Mobile (iOS):** minimum **iOS 10.0**; camera-capture features assume a device with at
  least a **12‑megapixel** camera. `[BUILD: mobile v3.x–v4.0]`
- **Format support:** raw (most DSLR/mirrorless), JPEG, TIFF, PNG, DNG; **HEIC** support
  on Windows added at desktop **v1.5 (Aug 2018)**.

## IMPL
- Architect the render pipeline GPU-first with a CPU fallback; the document treats GPU as
  expected, not optional, for interactive editing.
- Gate HEIC decode behind a platform capability check (it was a staged rollout: macOS
  first, Windows at v1.5).
- Maintain a **supported-camera / supported-lens** table as external data (Adobe ships
  these as updatable lists, not hardcoded). See files 06 (Optics) and 12.
