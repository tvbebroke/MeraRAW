# MeraRAW Crop Tool — Implementation Specification

> Based on Adobe Lightroom Classic / Lightroom CC crop behavior.
> This document is the single source of truth for implementing the crop tool in MeraRAW.
> Feed this into Cursor to guide implementation.

---

## 1. Overview

The crop tool is a non-destructive image editing tool that allows users to reframe, resize, straighten, rotate, and flip their photos. All crop data is stored as metadata (not baked into the image), meaning the original pixels are always preserved and the crop can be modified or removed at any time.

The crop tool has four major subsystems:

1. **Crop Frame** — interactive bounding box with handles for resizing
2. **Aspect Ratio** — constrained or free aspect ratio control
3. **Straighten / Rotate** — rotation via drag, slider, or ruler tool
4. **Composition Overlays** — visual guides (Rule of Thirds, Golden Ratio, etc.)

---

## 2. Crop Frame

### 2.1 Activation

The crop tool is activated from the toolbar or via a keyboard shortcut. When activated:

- A crop bounding box appears over the image with corner and edge handles.
- The area outside the crop is dimmed (semi-transparent dark overlay) to help the user focus on the cropped composition.
- The crop tool panel appears in the right sidebar showing all controls.

Deactivation happens when the user presses Enter/Return (apply), Esc (cancel), or clicks another tool.

### 2.2 Crop Handles

The bounding box has **8 handles**:

| Handle         | Position          | Behavior                                                         |
| -------------- | ----------------- | ---------------------------------------------------------------- |
| Top-left       | Corner            | Resizes width + height simultaneously                            |
| Top-right      | Corner            | Resizes width + height simultaneously                            |
| Bottom-left    | Corner            | Resizes width + height simultaneously                            |
| Bottom-right   | Corner            | Resizes width + height simultaneously                            |
| Top            | Edge midpoint     | Resizes height only (moves top edge)                             |
| Bottom         | Edge midpoint     | Resizes height only (moves bottom edge)                          |
| Left           | Edge midpoint     | Resizes width only (moves left edge)                             |
| Right          | Edge midpoint     | Resizes width only (moves right edge)                            |

**Corner handle behavior with locked aspect ratio:** When the aspect ratio is locked, dragging a corner handle resizes proportionally — both width and height change together to maintain the ratio. Edge handles are disabled or snap to maintain the ratio as well.

**Corner handle behavior with unlocked aspect ratio:** When unlocked, corner handles resize width and height independently (freeform crop). Edge handles adjust only one dimension.

**Modifier key behaviors:**

- **Alt/Option + drag corner:** Crops symmetrically from center (all four sides move equally inward/outward, maintaining center position).
- **Shift + drag corner:** Temporarily constrains to the current aspect ratio even if the ratio is unlocked.

### 2.3 Panning the Image Within the Crop

When the user moves the cursor inside the crop area (not on a handle), the cursor changes to a **hand/grab icon**. Clicking and dragging moves the image within the crop frame (the crop frame stays fixed, the image moves underneath). This lets the user reposition the subject without changing the crop dimensions.

The image cannot be dragged beyond its own edges — the system prevents revealing empty/transparent space outside the image bounds.

### 2.4 Dimmed Region

Everything outside the crop bounding box is rendered with a semi-transparent dark overlay (typically ~50% black). This is cosmetic only and helps the user evaluate the final framing. The dim level may optionally be configurable.

### 2.5 "Lights Out" Mode

Pressing L twice blacks out everything except the cropped area for distraction-free evaluation. Press L again to cycle back.

---

## 3. Aspect Ratio System

### 3.1 Lock/Unlock Toggle

A padlock icon controls whether the crop is constrained to an aspect ratio or freeform.

- **Locked (closed padlock):** Dragging any handle maintains the selected aspect ratio. The user cannot change proportions, only size.
- **Unlocked (open padlock):** Dragging handles creates any rectangular shape freely.

Keyboard shortcut to toggle: **A**.

### 3.2 Preset Aspect Ratios

The aspect ratio dropdown offers these presets:

| Label       | Ratio  | Use Case                                       |
| ----------- | ------ | ---------------------------------------------- |
| As Shot     | varies | Original camera sensor ratio                   |
| Original    | varies | Same as the image's native ratio               |
| 1 x 1       | 1:1    | Square — Instagram, profile photos             |
| 4 x 5       | 4:5    | Instagram portrait, 8×10 print                 |
| 5 x 7       | 5:7    | 5×7 print                                      |
| 2 x 3       | 2:3    | Standard DSLR/mirrorless ratio, 4×6 print      |
| 4 x 3       | 4:3    | Micro Four Thirds, older digital, iPad screen   |
| 5 x 4       | 5:4    | Large format film, 16×20 print                 |
| 16 x 9      | 16:9   | Widescreen/cinematic, YouTube, TV              |
| Custom      | user   | User enters width and height values            |

**"Enter Custom" option:** The user types two numbers (e.g., `7` and `5`). This ratio is saved and appears in the dropdown for reuse in the session. Custom ratios persist across images in the same session.

### 3.3 Orientation Flip

The aspect ratio can be flipped between landscape and portrait orientation:

- **Keyboard shortcut: X** (only works while the crop tool is active)
- Example: A 4×5 portrait crop becomes 5×4 landscape, and vice versa.
- The crop frame rotates 90°, and the image is re-evaluated within the new frame.

### 3.4 "As Shot" vs "Original"

- **As Shot:** The aspect ratio of the original camera file (e.g., 3:2 for most DSLRs/mirrorless, 4:3 for some compacts).
- **Original:** If the image has already been cropped once and the user selects a new ratio, "Original" restores the previously-applied crop's ratio (not necessarily the sensor ratio).

### 3.5 Important: Aspect Ratio ≠ Output Size

The crop tool only sets proportions (the ratio between width and height). It does NOT set the final output pixel dimensions or print size. Output sizing is handled separately in the export dialog.

---

## 4. Straighten and Rotate

### 4.1 Rotation by Dragging Outside the Crop Frame

When the cursor is positioned just outside the crop bounding box, it changes to a **curved double-arrow rotation cursor**. Clicking and dragging rotates the image around its center. A fine grid appears inside the crop area during rotation to aid alignment.

The rotation is continuous (not snapped to fixed increments), providing smooth real-time feedback. The crop frame remains axis-aligned (horizontal/vertical) — the image rotates within it. After rotation, the crop auto-adjusts to avoid showing empty corners.

### 4.2 Angle Slider

A slider control labeled "Angle" or "Straighten" in the crop tool panel allows precise numeric rotation input. Range is typically **-45° to +45°** in fine increments (0.01° precision). The slider updates the rotation in real time.

### 4.3 Straighten / Ruler Tool

A ruler/level tool is available for automated straightening:

1. Click the ruler icon in the crop panel.
2. Click and drag a line on the image along something that should be horizontal (like a horizon) or vertical (like a building edge).
3. On release, the image automatically rotates so that the drawn line becomes perfectly horizontal or vertical.

**Shortcut:** Hold **Cmd/Ctrl** while cursor is inside the crop area — the cursor changes to the ruler tool. Drag along the horizon, release, and the image straightens. Releasing the modifier key returns to normal crop mode.

### 4.4 Auto-Crop After Rotation

When the image is rotated, the corners of the original rectangular image no longer fill the crop frame. The system automatically adjusts (shrinks) the crop to ensure no empty/transparent areas are visible. This is commonly called "constrain crop" behavior.

If the user rotates significantly, more of the image edges are lost. The crop frame shrinks to compensate. The user can optionally disable constrain-crop to allow seeing the empty corners (useful for special effects).

---

## 5. Rotate and Flip (Discrete Transforms)

These are separate from the continuous rotation above. They are discrete 90° rotations and mirror flips:

| Action              | Effect                        |
| ------------------- | ----------------------------- |
| Rotate Left 90°     | Rotates image 90° counter-clockwise |
| Rotate Right 90°    | Rotates image 90° clockwise         |
| Flip Horizontal     | Mirrors the image left-to-right     |
| Flip Vertical       | Mirrors the image top-to-bottom     |

These are typically button controls (icons) in the crop panel. They operate on the full image, not just the crop area.

---

## 6. Composition Guide Overlays

### 6.1 Overview

While the crop tool is active, a composition overlay grid is displayed on top of the crop area. This is purely visual — it helps the user align key elements for better composition but does not affect the image data.

### 6.2 Available Overlays

Cycle through overlays by pressing **O**. The 7 standard overlays are:

| # | Overlay Name     | Description                                                                                   |
|---|------------------|-----------------------------------------------------------------------------------------------|
| 1 | **Grid**          | Simple even grid (6×6 or similar), useful for general alignment                              |
| 2 | **Thirds**        | Rule of Thirds — 3×3 grid dividing the frame into 9 equal rectangles                         |
| 3 | **Diagonal**      | Lines from corners to opposite edges, useful for leading-line compositions                    |
| 4 | **Triangle**      | Golden Triangle — diagonal line from corner to corner + perpendicular lines from other corners |
| 5 | **Golden Ratio**  | Phi Grid — similar to thirds but lines are closer to center (ratio 1:0.618:1)                |
| 6 | **Golden Spiral** | Fibonacci spiral — guides the eye in a natural curve toward a focal point                     |
| 7 | **Aspect Ratios** | Shows outlines of multiple common aspect ratios simultaneously (e.g., 4×5, 1×1, 16×9)        |

### 6.3 Overlay Rotation/Flip

Some overlays are asymmetric (Golden Triangle, Golden Spiral). Pressing **Shift+O** rotates/flips these overlays:

- **Golden Triangle:** Flips horizontally (diagonal goes from top-left→bottom-right vs top-right→bottom-left).
- **Golden Spiral:** Rotates through 4 orientations (spiral origin in each corner) × 2 mirror directions = up to 8 positions.

### 6.4 Overlay Visibility Control

The overlay can be set to:

- **Auto:** Shows only when the crop tool is active and the user is interacting (dragging handles, etc.).
- **Always:** Overlay is always visible while the crop tool is open.
- **Never:** Overlay is hidden (user may want an unobstructed view).

Toggle overlay visibility with **H**.

### 6.5 Choosing Which Overlays to Cycle

Under the menu Tools → Crop Guide Overlay → Choose Overlays to Cycle, the user can select which overlays appear when pressing O. This prevents cycling through unwanted overlays and speeds up the workflow.

---

## 7. Non-Destructive Architecture

### 7.1 Data Model

The crop is stored as metadata, never modifying the original image pixels. The crop data structure should contain:

```
CropData {
    // Crop rectangle (normalized 0.0–1.0 coordinates relative to original image)
    top:    f64,   // 0.0 = top edge of original
    left:   f64,   // 0.0 = left edge of original
    bottom: f64,   // 1.0 = bottom edge of original
    right:  f64,   // 1.0 = right edge of original

    // Rotation angle in degrees (-45.0 to +45.0 for straightening)
    angle:  f64,

    // Discrete transforms
    rotate_90_steps: i32,   // 0, 1, 2, or 3 (number of 90° CW rotations)
    flip_horizontal: bool,
    flip_vertical:   bool,

    // Aspect ratio state
    aspect_ratio_locked: bool,
    aspect_ratio: Option<(u32, u32)>,  // None = freeform, Some((w, h)) = constrained

    // Constrain crop (auto-shrink after rotation)
    constrain_crop: bool,
}
```

### 7.2 Reset

The crop can be fully reset at any time, restoring the original uncropped image. The reset operation clears all crop fields back to their defaults (full image, no rotation, no flip).

Keyboard shortcut for full reset: **Ctrl+Alt+Shift+R** (Windows) / **Cmd+Option+Shift+R** (Mac).

### 7.3 Sidecar Storage

Crop data is saved in the app's sidecar file (e.g., `.rrdata` or equivalent). The crop is applied at render/export time, never written back to the source file. Users can re-edit the crop indefinitely.

---

## 8. Keyboard Shortcuts Summary

| Shortcut                         | Action                                                   |
| -------------------------------- | -------------------------------------------------------- |
| **R**                            | Activate crop tool (from anywhere in Develop module)     |
| **Enter / Return**               | Apply crop and close tool                                |
| **Esc**                          | Cancel crop changes and close tool                       |
| **X**                            | Flip crop orientation (landscape ↔ portrait)             |
| **A**                            | Toggle aspect ratio lock/unlock                          |
| **O**                            | Cycle through composition overlays                       |
| **Shift+O**                      | Rotate/flip asymmetric overlays (Golden Spiral, Triangle)|
| **H**                            | Toggle overlay visibility                                |
| **Shift+A**                      | Apply current aspect ratio to new photo                  |
| **Alt/Opt + drag corner**        | Crop symmetrically from center                           |
| **Shift + drag corner**          | Temporarily constrain to aspect ratio                    |
| **Cmd/Ctrl + drag inside crop**  | Activate ruler/straighten tool                           |
| **L** (press twice)              | Lights Out mode (black out everything outside crop)      |
| **Ctrl+Alt+Shift+R / Cmd+Opt+Shift+R** | Reset crop to original                            |

---

## 9. Interaction Flow (User Journey)

```
User presses R
    → Crop tool activates
    → Bounding box + handles appear over image
    → Area outside crop is dimmed
    → Overlay grid appears (if set to Auto or Always)
    → Crop panel opens in right sidebar

User drags corner handle
    → If aspect locked: resize proportionally
    → If aspect unlocked: resize freely
    → Live preview updates in real time
    → Overlay grid scales with crop frame

User moves cursor inside crop area
    → Cursor changes to hand
    → Drag to pan image within the fixed crop frame
    → Image cannot be dragged past its edges

User moves cursor outside crop area (near edge)
    → Cursor changes to rotation arrow
    → Drag to rotate image
    → Fine grid appears during rotation
    → Crop auto-shrinks to avoid showing empty corners

User selects aspect ratio from dropdown
    → Crop frame snaps to new ratio
    → Frame resizes to fit within current image bounds
    → Previous crop dimensions are adjusted proportionally

User presses O
    → Overlay cycles: Grid → Thirds → Diagonal → Triangle → Golden Ratio → Golden Spiral → Aspect Ratios → Grid...

User presses Enter
    → Crop is applied (stored as metadata)
    → Tool deactivates
    → Image displays cropped view throughout the app
```

---

## 10. Rendering / Export Considerations

When rendering or exporting the image, the crop is applied in this order:

1. **Load original image pixels**
2. **Apply discrete transforms** (90° rotations, flips)
3. **Apply continuous rotation** (straighten angle)
4. **Apply crop rectangle** (extract the defined region)
5. **Scale to output dimensions** (set in export dialog, not in crop tool)
6. **Apply all other adjustments** (exposure, color grading, etc.)

The crop is typically applied early in the pipeline because it determines which region of the image is processed. Processing only the cropped region can save GPU/CPU time on large images, though the full image must be available for re-cropping.

---

## 11. Edge Cases and Behavior Notes

- **Crop cannot exceed image bounds.** The crop rectangle is always fully contained within the image. Handles snap to image edges.
- **Minimum crop size.** Enforce a minimum crop size (e.g., 50×50 pixels or a percentage of the original) to prevent zero-area crops.
- **Aspect ratio + rotation interaction.** When the image is rotated and the aspect ratio is locked, the crop frame must shrink to fit within the rotated image bounds while maintaining the ratio.
- **Copy/paste crop settings.** Users should be able to copy crop settings from one image and paste them to another. The crop rectangle coordinates are relative (0.0–1.0), so they transfer across images of different absolute sizes. The aspect ratio, angle, and flip settings transfer directly.
- **Batch cropping.** When syncing adjustments across multiple images, crop settings should be syncable. The user should verify positioning per-image since subjects will be in different positions.
- **Undo/redo.** Every crop adjustment (handle drag, rotation, aspect ratio change, flip, etc.) should create a history entry for undo/redo.
- **High-DPI / Retina displays.** Crop handles, overlay lines, and dimming must render at the correct resolution on high-DPI displays. Handle hit targets should be sized for comfortable interaction (minimum ~10px logical on desktop, ~44px on touch).
- **Touch support.** On touch devices, crop handles should have larger hit targets. Pinch-to-zoom should be supported within the crop tool. Two-finger rotation could optionally map to the straighten rotation.

---

## 12. UI Component Reference (PhotoCrop Pattern)

The PhotoCrop library (github.com/albinmathew/PhotoCrop) demonstrates a common mobile crop architecture with two key components:

- **PhotoView layer:** Handles image display, zoom (multi-touch + double-tap), and scroll/pan with smooth fling. The image moves under a static viewport.
- **CropOverlayView layer:** Renders the resizable crop window on top of the PhotoView. The overlay stays static while the user zooms/pans the image underneath.

Key architectural insight: The crop window is a static overlay while the image is the element that moves, zooms, and rotates underneath it. This is the "Facebook/Telegram style" crop — the opposite of the Lightroom desktop style where the crop frame moves over a fixed image. For a desktop RAW editor like MeraRAW, use the **Lightroom pattern** (crop frame moves over fixed image) rather than the mobile pattern.

Configurable properties from PhotoCrop worth considering for MeraRAW:

| Property       | Type      | Description                                              |
| -------------- | --------- | -------------------------------------------------------- |
| guideLines     | boolean   | Whether to show rule-of-thirds guides inside the crop    |
| drawCircle     | boolean   | Whether the crop shape is circular (for avatar crops)    |
| cornerRadius   | dimension | Rounded corners on the crop overlay                      |
| borderColor    | color     | Color of the crop boundary lines                         |
| overlayColor   | color     | Color/opacity of the dimmed area outside the crop        |
| marginSide     | dimension | Minimum margin between crop edge and viewport edge       |
| marginTop      | dimension | Minimum margin at top                                    |

---

## 13. Geometry / Perspective Correction (Separate from Crop)

Lightroom also has a Geometry panel (Transform tools) that is related to but separate from the crop tool. This is a secondary priority for MeraRAW but documented here for awareness:

- **Upright modes:** Auto, Level, Vertical, Full, Guided — automatically correct perspective distortion.
- **Transform sliders:** Distortion, Vertical, Horizontal, Rotate, Aspect, Scale, X Offset, Y Offset.
- **Constrain Crop checkbox:** When perspective corrections are applied, the image may show empty corners. This checkbox auto-crops to hide them.
- **Guided Upright:** User draws 2–4 lines on the image to define what should be horizontal/vertical, and the perspective warps to match.

These are computationally more complex (affine/projective transforms) and should be implemented as a separate feature after the basic crop tool is complete.

---

## 14. Implementation Priority Order

For MeraRAW, implement in this order:

1. **Basic crop frame** with 8 handles, drag to resize, and dimmed overlay
2. **Pan image within crop** (hand tool)
3. **Aspect ratio lock/unlock** with preset ratios
4. **Orientation flip** (X key)
5. **Straighten by dragging outside crop frame**
6. **Angle slider**
7. **Ruler/straighten tool**
8. **Composition overlays** (start with Rule of Thirds, then add others)
9. **Discrete rotate/flip** (90° rotation buttons, horizontal/vertical flip)
10. **Custom aspect ratio input**
11. **Copy/paste crop settings across images**
12. **Overlay cycling and rotation** (O and Shift+O)
13. **Constrain crop after rotation**
14. **Geometry/perspective tools** (future phase)
