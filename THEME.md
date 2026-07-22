# MeraRAW Design System & Styling Guide

This document explains the color tokens, design system, theme architecture, and guidelines for styling components in the **MeraRAW** UI (`meraraw-ui-svelte`).

---

## 🎨 Semantic Color Tokens

All colors in MeraRAW are centralized in [`src/app.css`](file:///Users/avadhootkolee/Documents/Development/Meratech/meraraw-ui-svelte/src/app.css) using Tailwind CSS v4 `@theme` tokens. 

### 1. Containers & Surfaces
| Token Name | Tailwind Class | Modern Glass Default | Classic Lightroom | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `--color-window` | `bg-window` | `#272727` | `#161618` | Main application background frame |
| `--color-panel` | `bg-panel` | `#111113` | `#1e1e20` | Primary sidebar / rail container |
| `--color-panel-2` | `bg-panel-2` | `#232325` | `#252528` | Secondary panel container |
| `--color-panel-3` | `bg-panel-3` | `#2e2e30` | `#2d2d30` | Tertiary panel container / modal card |
| `--color-surface-input` | `bg-surface-input` | `rgba(0,0,0,0.25)` | `#121214` | Text fields, search bars, textareas |
| `--color-card` | `bg-card` | `#1e1e20` | `#1e1e20` | Photo thumbnail card background |
| `--color-card-active` | `bg-card-active` | `#2b2b2e` | `#2b2b2e` | Active photo card background |

### 2. Borders & Dividers
| Token Name | Tailwind Class | Modern Glass Default | Classic Lightroom | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `--color-border-subtle` | `border-border-subtle` | `rgba(255,255,255,0.05)` | `#141416` | Panel borders & section separators |
| `--color-border-input` | `border-border-input` | `rgba(255,255,255,0.10)` | `#333336` | Input outlines and button borders |
| `--color-border-hover` | `border-border-hover` | `rgba(255,255,255,0.25)` | `#4a4a4d` | Hover / active input borders |

### 3. Typography & Text Contrast
| Token Name | Tailwind Class | Modern Glass Default | Classic Lightroom | Purpose |
| :--- | :--- | :--- | :--- | :--- |
| `--color-text-primary` | `text-text-primary` | `rgba(255,255,255,0.92)` | `#e0e0e2` | Primary body text & headings |
| `--color-text-secondary` | `text-text-secondary` | `rgba(255,255,255,0.65)` | `#a0a0a5` | Secondary labels & metadata |
| `--color-text-muted` | `text-text-muted` | `rgba(255,255,255,0.40)` | `#707075` | Disabled text & placeholders |

### 4. Interactive Elements & Buttons
| Token Name | Tailwind Class | Default Value | Purpose |
| :--- | :--- | :--- | :--- |
| `--color-accent` | `bg-accent` | `#ffffff` | Primary accent color |
| `--color-button-bg` | `bg-button-bg` | `rgba(255,255,255,0.06)` | Standard button background |
| `--color-button-hover` | `bg-button-hover` | `rgba(255,255,255,0.12)` | Standard button hover state |

---

## 🌓 Theme Architecture & Switching

Theme state is managed globally via Nanostores in [`src/stores/ui.ts`](file:///Users/avadhootkolee/Documents/Development/Meratech/meraraw-ui-svelte/src/stores/ui.ts):

- **Modern Liquid Glass (Default)**: Uses blurred frosted glass backdrops (`.glass-backdrop`), subtle semi-transparent borders, and rounded corners (`rounded-[22px]`).
- **Classic Lightroom Look (`.classic-look`)**: Toggled by `classicLook` store. Re-assigns the root design token variables to flat dark solid shades and sharp corners (`border-radius: 0px`).

### How to Edit Colors
To change a color across the entire application:
1. Open [`src/app.css`](file:///Users/avadhootkolee/Documents/Development/Meratech/meraraw-ui-svelte/src/app.css).
2. Locate the token under `@theme` (for default Modern mode) or under `.classic-look` (for Classic mode).
3. Modify the color value. All components using Tailwind semantic classes or CSS custom variables will update automatically.

---

## 🛠 Best Practices for New Components

1. **Use Semantic Classes**: Avoid hardcoding hex values like `bg-[#1a1a1c]` or `border-[#333336]`. Use semantic tokens such as `bg-panel`, `border-border-subtle`, or `text-text-primary`.
2. **Glass Primitives**: Wrap floating toolbars and side panels using [`GlassPanel.svelte`](file:///Users/avadhootkolee/Documents/Development/Meratech/meraraw-ui-svelte/src/lib/components/primitives/GlassPanel.svelte) or [`GlassedButton.svelte`](file:///Users/avadhootkolee/Documents/Development/Meratech/meraraw-ui-svelte/src/lib/components/primitives/GlassedButton.svelte) so they automatically honor backdrop filters in Modern Mode and flatten cleanly in Classic Mode.
3. **Resizing Rails**: Never place static `overflow-hidden` on expanding rail wrapper components as defined in project rules (`AGENTS.md`).
