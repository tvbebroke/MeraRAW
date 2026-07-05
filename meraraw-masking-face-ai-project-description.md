# MeraRAW — AI Masking & Face Recognition: Project Description

## 1. Purpose

This document is a deep technical breakdown of how Adobe Lightroom's masking system and face-recognition ("People View") features work, intended as a build spec for MeraRAW. It covers the full taxonomy of Lightroom's masking tools, the distinct ML subsystems behind each one, how face recognition/clustering works separately from masking, and concrete architectural recommendations — including specific model families — for implementing an equivalent (or better) system in MeraRAW.

**Framing that matters for implementation:** Lightroom's masking tools are *analytical* AI — they study pixels that already exist in the photo and draw precise boundaries around them for targeted editing. Nothing is invented or generated. This is architecturally a completely different (and much simpler/cheaper) problem than generative AI (e.g., Generative Remove/Fill), and it's the right scope for MeraRAW's masking + face features.

---

## 2. The Full Masking Taxonomy

Lightroom's Masking panel has two families of tools: **rule-based/manual** masks (fast, deterministic, no ML needed) and **AI-powered/semantic** masks (require a trained model per selection type). MeraRAW should implement both, since they're complementary — AI masks alone can't do everything (e.g., a photographer wants "top third of frame" — that's what gradients are for), and manual masks alone are what everyone was frustrated with before AI masking existed.

### 2.1 Manual / rule-based masks (no ML required)
| Tool | Mechanism |
|---|---|
| **Brush** | Freehand paint, size/feather/flow/density controlled, builds a raster alpha mask |
| **Linear Gradient** | Mask value tapers linearly across a user-drawn line — classic "graduated ND filter" simulation |
| **Radial Gradient** | Mask value tapers radially from an ellipse — classic vignette/spotlight simulation |
| **Color Range** | User samples a color (eyedropper); mask = pixels within a tunable distance in color space from the sample(s) |
| **Luminance Range** | Mask built from a tunable range on the luminance histogram (e.g., "only shadows," "only highlights") |
| **Depth Range** | Uses a depth map (from dual-camera Portrait Mode / mobile depth capture / LiDAR) to mask by distance-from-camera rather than color or shape |

These are all cheap, deterministic, and don't need a neural network — straightforward to implement with standard image processing (color-space distance, histogram thresholding, depth-map thresholding, procedural gradient generation). Build these first; they're the foundation every AI mask gets refined on top of (via Add/Subtract/Intersect).

### 2.2 AI-powered / semantic masks (require trained models)
| Tool | What it detects | Underlying ML task |
|---|---|---|
| **Select Subject** | The single most prominent subject (person, animal, object) | Salient object segmentation |
| **Select Sky** | Sky region, including complex edges (branches, hair) | Semantic segmentation (single class) |
| **Select Background** | Everything *except* the detected subject | Derived — literally the inverse of Select Subject |
| **Select Object** | A specific object the user roughly points at (brush scribble or rectangle) | Interactive/promptable segmentation |
| **Select Landscape elements** | Up to 8 classes: Sky, Mountains, Architecture, Vegetation, Water, Snow, Natural Ground, Artificial Ground | Multi-class semantic segmentation |
| **Select People** | Each individual person, then decomposed into up to ~9 sub-regions: Face Skin, Body Skin, Eyebrows, Eye Sclera, Iris & Pupil, Lips, Teeth, Hair, Clothes | Instance segmentation + facial/body part parsing |
| **Select Face (People View)** | *Who* a face belongs to, across the whole library | Face detection → embedding → clustering (a completely separate subsystem from the above — see Section 4) |

Each row above is functionally a **separate model or model head** in Adobe's pipeline — "Select Subject" and "Select People" are not the same network fine-tuned differently; they're solving different segmentation problems (salient single-object vs. multi-instance-plus-part-parsing).

---

## 3. Deep Dive: How the Segmentation Masks Work

### 3.1 Select Subject / Select Sky / Select Background — semantic + salient segmentation
<cite index="33-1">Lightroom uses a class of AI called computer vision — models trained to classify and segment parts of an image.</cite> <cite index="33-1">For Select Sky specifically, the AI has learned from millions of photos what sky pixels typically look like — gradients, colors, position, edges — and draws a precise boundary around them, using semantic segmentation to understand what "sky" looks like even around complex edges like trees and hair, across different lighting conditions.</cite> <cite index="33-1">Select Subject identifies the main subject in the frame — a person, animal, or object — and traces a mask around it using object-recognition models trained on millions of images.</cite>

From a photographer's-eye description of the underlying logic: <cite index="39-1">Lightroom evaluates edges, shapes, contrast, and context to determine what the viewer is most likely to focus on</cite> when picking the subject — i.e., this is a **saliency-driven** subject picker, not just "find any foreground object." This is architecturally consistent with modern salient-object-detection literature (U²-Net-style architectures, or encoder-decoder segmentation networks like DeepLab/U-Net variants that output a per-pixel foreground probability map).

**Practical accuracy notes worth designing around** (from real-world usage):
- Select Subject/Background "work best when your photo has a clearly defined subject" and quality degrades on cluttered or ambiguous scenes.
- Hair edges are a known weak point — <cite index="31-1">users routinely have to Subtract-brush along hair edges to fix halos</cite> where the segmentation boundary doesn't perfectly follow fine strands.
- Object selection (see 3.2) and Background masks are noted as "generally good but can be frustratingly inaccurate" compared to Subject/Sky, which are the most mature/reliable of the set.

### 3.2 Select Object — interactive/promptable segmentation
<cite index="54-1">The Select Object tool is used by either roughly drawing with the brush or using a rectangle selection; the software then detects the edges and creates a mask of the object.</cite> This is a **user-prompted** segmentation task — architecturally the same family of problem as "click a point / draw a box, get a precise object mask," which is the exact problem class that models like Meta's Segment Anything Model (SAM) and its mobile-optimized derivatives (MobileSAM, EfficientSAM) were built for. This is a strong direct reference architecture for MeraRAW's equivalent feature.

### 3.3 Select People — multi-instance + facial/body part parsing
This is the most sophisticated segmentation feature and worth building carefully, since it's the highest-leverage tool for portrait photographers (a stated "waited 10 years for this" feature among power users).

**Pipeline stages, inferred from behavior:**
1. **Person instance detection** — find each individual person in the frame as a separate, selectable instance (a photo with 5 people yields 5 independently selectable circles/thumbnails).
2. **Part parsing per person** — once a person is selected, <cite index="61-1">Lightroom automatically analyzes their features and breaks the mask down into smaller individual components</cite>, drawn from a fixed vocabulary: <cite index="55-1">Face Skin, Body Skin, Eyebrows, Eye Sclera, Iris and Pupil, Lips, Teeth, Hair, and the Entire Person</cite>, and — per some reporting — clothes as well.
3. **Conditional availability** — <cite index="56-1">if a feature isn't visible in the photo it's not offered as an option — e.g., no facial-hair mask if the subject is clean-shaven, no eye options if they're wearing sunglasses</cite>. This means the parsing model outputs *confidence per class* and the UI only surfaces classes above some threshold, rather than always listing every category.
4. **Multi-mask generation in one action** — selecting several regions at once creates a single batch operation (<cite index="53-1">"Create 8 separate masks"</cite> when multiple face parts are chosen at once), each becoming an independently editable, named mask.

This maps cleanly onto the **face-parsing** literature (face segmentation into semantic regions: skin, eyebrows, eyes, nose, mouth, hair) combined with **person/body instance segmentation** for the non-face regions (body skin, clothes). A very close off-the-shelf analog worth evaluating directly for MeraRAW: **MediaPipe Face Mesh / Face Landmarker**, which already outputs dense per-landmark facial geometry (468 points including lips, eyes, iris, eyebrows contours) that can be rasterized into exactly this kind of per-feature mask with modest engineering — a meaningfully faster path than training a segmentation network from scratch. MediaPipe's Selfie Segmentation / Pose models cover the body-skin/hair/clothes side of the same problem.

**Known failure modes to expect and design for:**
- Group shots reduce reliability of per-person facial-feature breakdown.
- Occluded features fail gracefully (sunglasses → no eye masks) rather than hallucinating a mask.
- Non-frontal poses and extreme angles degrade part-parsing accuracy, consistent with any landmark-based facial parsing approach.

---

## 4. Deep Dive: Face Recognition ("People View") — a Separate Subsystem

This is architecturally **unrelated to masking** — it doesn't create an editable mask at all. It's a library-organization feature: detect faces, cluster faces that likely belong to the same person, and let the user assign names, producing searchable keyword metadata. This is the feature the user specifically wants researched, so treat it as its own pipeline with its own three stages.

### 4.1 Stage 1 — Face detection
<cite index="50-1">Lightroom Classic uses imaging characteristics to find faces</cite> and flags a bounding region for each one, running as a background indexing process <cite index="50-1">that continues after the initial pass and automatically picks up faces in any newly added images</cite>. <cite index="52-1">Undetected faces can be manually corrected by drawing a face region by hand</cite>, meaning the product treats automatic detection as a strong prior, not a hard requirement — always keep a manual override path.

### 4.2 Stage 2 — Face embedding + unsupervised clustering
This is the core ML step: convert each detected face into a fixed-length numeric vector (an "embedding") positioned in a space where similar-looking faces land close together, then cluster nearby vectors into groups without knowing in advance how many distinct people exist. <cite index="46-1">Faces are initially loaded into stacks under "Unnamed People,"</cite> and — telling detail — <cite index="46-1">you'll initially find several separate stacks that are actually the same person</cite>, i.e., the clustering is intentionally conservative (favors precision over recall, avoiding false merges) and relies on the user to manually merge clusters it wasn't confident enough to merge automatically. <cite index="47-1">Lightroom can detect when two clusters might actually be the same person and surfaces a "these might be the same person" merge suggestion, which the user accepts or rejects</cite> — a second-pass similarity check on top of the initial clustering.

This two-tier design (conservative auto-clustering + suggested-merge UI) is the right pattern to copy: it avoids the embarrassing failure mode of silently merging two different people, while still reducing manual labeling effort over time.

### 4.3 Stage 3 — Human-in-the-loop learning loop
- <cite index="46-1">Once faces are detected, the user assigns names by typing into a text box under each thumbnail</cite>, which <cite index="46-1">helps the system learn and recognize individuals across the rest of the catalog.</cite>
- As more labels accumulate, <cite index="45-1">the system starts guessing who unlabeled faces are and surfaces accept/reject suggestions</cite> rather than auto-applying — <cite index="46-1">accuracy of these suggestions varies, and can be thrown off by superficial traits like gender presentation, glasses, or hairstyle rather than true identity</cite>, which is a useful honest caveat: embedding-based similarity can latch onto correlated-but-wrong features if the embedding model or clustering threshold isn't well tuned — worth budgeting real QA time for.
- <cite index="47-1">In the cloud product, an unnamed cluster is only surfaced in the UI once it has 5+ photos</cite>, a sensible noise-reduction heuristic MeraRAW should copy directly (don't show a "person" for one blurry, ambiguous face).
- Named identities become **searchable keyword metadata** attached to each photo — this is the actual payoff for the user: "show me all photos of Sarah" becomes a metadata query, not a re-scan.

### 4.4 On-device vs. cloud processing — a real architectural fork
This is the most consequential design decision for MeraRAW, and Lightroom actually ships **two different implementations** depending on product line:
- **Lightroom Classic** (desktop, local catalog): face detection and clustering run **locally on-device**, indexing in the background, no cloud dependency, works fully offline.
- **Lightroom (cloud/CC)**: <cite index="47-1">analysis happens on Adobe's servers — you don't need the app open for it to keep running, but it does require photos to be synced to the cloud first</cite>, and it comes with an explicit consent framing: <cite index="47-1">enabling People View means you're telling Adobe it's okay to build face models on your behalf, and that you have the individuals' approval</cite>; <cite index="47-1">disabling it deletes all face-model data from Adobe's servers</cite>.
- There's a **dedicated, separate privacy toggle**: <cite index="33-1">a "Content Analysis" setting in Adobe's account privacy page that controls whether cloud-synced images get analyzed for ML purposes at all</cite> — independent of the People View toggle itself, and users who keep everything local/Classic-only are described as inherently outside this data flow.

**Recommendation for MeraRAW:** default to **fully on-device** face detection, embedding, and clustering (mirrors Lightroom Classic's model) — this sidesteps the entire consent/server-storage complexity Adobe has to manage for the cloud variant, keeps the feature usable offline, and is very achievable at reasonable accuracy with a lightweight on-device face-embedding model (MobileFaceNet-class models are specifically designed for this: small enough for phone-class inference, embeddings good enough for clustering-grade — not necessarily security-grade — face matching). Only add a cloud/sync-based version later if there's a specific cross-device-library use case that demands it, and treat it as a separate, clearly-consented opt-in exactly as Adobe does.

---

## 5. The Non-Destructive Masking Data Model

Independent of which tool created a mask, Lightroom stores masks as a structured, editable stack rather than baked-in pixels:

- Every mask is a named entry in a **Masks panel/list**, not a permanent pixel edit — <cite index="32-1">adjustments made using the Masking tool are non-destructive and never permanently alter the photo</cite>.
- Masks combine via three primitive operations: **Add** (union), **Subtract** (difference), **Intersect** (intersection) — <cite index="32-1">you can intersect two existing masks and continue editing the resulting combined region</cite>. This is a small, composable operation set — implement these three and most complex real-world masking needs are covered.
- **Invert** flips a mask's coverage in one click (commonly: Select Subject → Invert = instant background mask).
- AI masks are **not static bitmaps** — they're stored as *recipes* (which tool, which parameters, which target person/feature) that get **recomputed** when needed: <cite index="32-1">if a mask needs to be updated — image changed, rotated, a new spot appeared, or it's simply missing — Lightroom recomputes it</cite>, and this recomputation is what makes masks portable: <cite index="36-1">when you apply a preset or paste settings containing an AI mask onto a *different* photo, Lightroom recalculates the mask fresh for that new photo's content</cite> and then applies the associated adjustment values. This is the single most important architectural property to replicate — **a mask is a saved operation + parameters, not a saved bitmap**, which is what makes batch/preset workflows actually work across different images.
- Batch application is explicitly supported: <cite index="32-1">Select Subject, Select Sky, Select Background, Select Objects, and Select People masks can all be applied to multiple selected photos in one click</cite>, each photo getting its own freshly-computed mask from the same recipe.

---

## 6. Recommended Architecture for MeraRAW

### 6.1 Model plan by feature (concrete starting points)
| MeraRAW feature | Suggested model family | Why |
|---|---|---|
| Select Subject | Lightweight salient object segmentation (U²-Net-style or a MobileNet/EfficientNet-backboned encoder-decoder) | Well-studied, mobile-viable, single-class output keeps inference cheap |
| Select Sky | Dedicated binary segmentation model, or a class head on the same subject-segmentation backbone | Sky is a narrow, well-defined visual class — high accuracy is very achievable even with a small model |
| Select Background | No model needed — literal inverse of Select Subject | Free feature once Subject works |
| Select Object (click/scribble-to-mask) | MobileSAM / EfficientSAM (SAM distilled for on-device use) | Purpose-built for exactly this "point/box in, mask out" interaction |
| Select Landscape elements | Multi-class semantic segmentation (DeepLabV3+-style, small backbone) trained on Sky/Mountains/Architecture/Vegetation/Water/Snow/Ground classes | Standard multi-class segmentation problem, doable with a modest labeled dataset |
| Select People (person + parts) | MediaPipe Face Landmarker (face mesh → skin/eyes/lips/eyebrows regions) + MediaPipe Selfie/Pose segmentation (body skin, hair, clothing) | Strong off-the-shelf coverage of nearly the entire "People" mask vocabulary without training from scratch |
| Face Recognition (People View equivalent) | On-device face detector (e.g., a lightweight RetinaFace/BlazeFace-class detector) → MobileFaceNet-class embedding model → incremental clustering (start with a conservative similarity threshold; consider HDBSCAN or simple nearest-centroid with a high confidence bar) | Matches Lightroom Classic's local-only approach; keeps this fully offline and privacy-simple |

### 6.2 Data model
Store every mask as a structured record, not a bitmap:
```
Mask {
  id, name, type: "brush" | "gradient" | "radial" | "color_range" | "luminance_range"
        | "depth_range" | "select_subject" | "select_sky" | "select_background"
        | "select_object" | "select_landscape" | "select_people",
  parameters: { ...tool-specific params, e.g. person_id + feature_list for select_people },
  computed_mask_cache: <bitmap, invalidated on crop/rotate/content change>,
  combine_ops: [ {op: "add"|"subtract"|"intersect", with: mask_id} ],
  adjustments: { exposure, contrast, saturation, texture, ... }
}
```
The `computed_mask_cache` is a performance cache only — the source of truth is `type` + `parameters`, exactly mirroring Lightroom's "recompute on demand" model, which is what makes cross-photo preset/batch application work.

### 6.3 People/Face data model (separate from masks)
```
Person {
  id, display_name (nullable = "unnamed"),
  embedding_centroid: vector,
  face_instances: [ { photo_id, bbox, embedding, confidence, user_confirmed: bool } ]
}
```
- Only surface an unnamed `Person` cluster in the UI once it crosses a minimum count (Adobe uses 5) — reduces noise from one-off/blurry detections.
- Always support **manual face-region drawing** as a fallback when auto-detection misses a face (angle, lighting, occlusion).
- Implement the **conservative-cluster + suggested-merge** pattern rather than aggressive auto-merging — false merges (mislabeling two different people as one) are a much worse user experience than requiring a couple of manual merge taps.

### 6.4 UX principles worth copying directly
- **Add / Subtract / Intersect / Invert** as the universal mask-refinement toolkit — don't invent new primitives.
- **Overlay visualization modes** (solid color, B&W, color-on-B&W, image-on-B&W) so users can verify mask quality before committing — this is cheap to build and meaningfully improves trust in AI-generated masks.
- **Graceful omission, not hallucination**: if a facial feature isn't visible/detectable (sunglasses, facial hair on a clean-shaven subject), simply don't offer that option — never guess.
- **Batch-apply from one recipe**: computing an AI mask once and reusing the *recipe* (not the bitmap) across a whole shoot is the single highest-leverage workflow win for any photographer processing more than a handful of images.

### 6.5 What to explicitly scope out of v1
- Cloud-synced cross-device face recognition (adds consent/server-storage/legal complexity disproportionate to early-stage value — start on-device only).
- Perfect hair-edge segmentation (even Lightroom, the best-in-class tool, still requires manual brush touch-ups here — set expectations accordingly in your own UI, e.g., always leave Subtract/Add easily accessible right next to any AI mask).
- Full landscape 8-class segmentation in v1 — Select Subject + Select Sky + manual gradients cover the large majority of real photographer use cases; add the other 6 landscape classes later if usage data justifies it.

---

## 7. Evaluation Plan

- **Segmentation quality**: IoU (intersection-over-union) against hand-labeled ground truth masks, tracked separately per mask type (Subject, Sky, People-parts) since they're different models with different difficulty profiles.
- **Face clustering quality**: track false-merge rate (two different people grouped as one — the worse failure mode) separately from false-split rate (same person split across multiple clusters — the more tolerable failure mode, since Lightroom's own design explicitly tolerates this and relies on user-driven merging).
- **Edge-case test set**: deliberately include group photos, sunglasses/hats/facial coverings, extreme angles, backlit/silhouette subjects, and low-light portraits — these are the documented failure zones for every part of this feature set.
- **Latency budget on-device**: benchmark each model on your actual minimum-supported device tier, not just flagship hardware, since Select People potentially chains 2–3 models (detection → landmarks → part rasterization) per photo.

---

## 8. Summary

Lightroom's masking system isn't one AI feature — it's **five or six distinct, purpose-built models** (salient subject segmentation, sky segmentation, promptable object segmentation, multi-class landscape segmentation, person+face-part parsing) sitting behind one unified, non-destructive, recipe-based mask data model, plus a **completely separate** face-detection → embedding → clustering pipeline for library organization (People View) that produces searchable metadata rather than editable masks. For MeraRAW, the fastest credible path is: build the manual masks first (cheap, no ML), add Select Subject/Sky next (highest value-to-effort ratio, well-studied architectures), lean on MediaPipe's existing face/body landmark models to cover most of "Select People" without training from scratch, and keep face recognition fully on-device using a lightweight embedding model with conservative clustering — mirroring Lightroom Classic's local-only design rather than its cloud variant's consent-heavy path.
