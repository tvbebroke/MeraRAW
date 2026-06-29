# Keyboard Shortcut Catalog — meraraw

> Complete directory of every Lightroom Classic shortcut (source: Adobe helpx, last updated 2025‑10‑30), re‑documented for the meraraw editor. Key→action mappings are factual reference; every description is original.

**Totals:** 287 bindings — `core` 117 · `optional` 116 · `out_of_scope` 54.

**Relevance key:** `core` = implement first (global/Library/Develop) · `optional` = nice-to-have · `out_of_scope` = Lightroom output modules (Book/Slideshow/Print/Web/Map) a raw editor usually omits.

**Columns:** Action · Windows · macOS · Scope · Relevance · What it does · Notes


## Panels & chrome

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Show/hide side panels | `Tab` | `Tab` | global | core | Collapse/expand both side panel columns to maximize the image area. |  |
| Show/hide all panels | `Shift+Tab` | `Shift+Tab` | global | core | Hide every panel (side, filmstrip, module bar) for a clean canvas. |  |
| Show/hide toolbar | `T` | `T` | global | core | Toggle the contextual toolbar under the image. |  |
| Show/hide module picker | `F5` | `F5` | global | optional | Toggle the top module-switcher bar. | Module concept; map to your top nav. |
| Show/hide filmstrip | `F6` | `F6` | global | core | Toggle the bottom filmstrip strip of thumbnails. |  |
| Show/hide left panels | `F7` | `F7` | global | core | Toggle the left panel column only. |  |
| Show/hide right panels | `F8` | `F8` | global | core | Toggle the right panel column only. |  |
| Toggle solo mode | `Alt-click panel` | `Option-click panel` | global | optional | Solo mode auto-collapses other panels when one is opened. | Click-modifier, not a keychord. |
| Open panel without closing soloed panel | `Shift-click panel` | `Shift-click panel` | global | optional | Open an extra panel while solo mode is on. | Click-modifier. |
| Open/close all panels | `Ctrl-click panel` | `Cmd-click panel` | global | optional | Expand or collapse every panel in the column at once. | Click-modifier. |
| Open/close left panels top→bottom | `Ctrl+Shift+0–5` | `Cmd+Ctrl+0–5` | global | optional | Toggle individual left panels by index. | Numeric index → panel. |
| Open/close right panels (Library/Develop) | `Ctrl+0–9` | `Cmd+0–9` | module:library,module:develop | core | Toggle individual right panels by index. | Index map differs per module. |
| Redo | `Ctrl+Shift+Z` | `Cmd+Shift+Z` | global | core | Re-apply the last undone action. |  |
| Redo (Windows only) | `Ctrl+Y` | `n/a` | global | core | Windows alternate redo binding. | Win-only alias. |
| Undo | `Ctrl+Z` | `Cmd+Z` | global | core | Revert the last action; LrC undo is unlimited within a session. |  |

## Module navigation

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Go to Library | `Ctrl+Alt+1` | `Cmd+Opt+1` | global | core | Jump to the browse/cull workspace. |  |
| Go to Develop | `Ctrl+Alt+2` | `Cmd+Opt+2` | global | core | Jump to the edit workspace. |  |
| Go to Map | `Ctrl+Alt+3` | `Cmd+Opt+3` | global | out_of_scope | Geotagging workspace. | Skip unless you add maps. |
| Go to Book | `Ctrl+Alt+4` | `Cmd+Opt+4` | global | out_of_scope | Photo-book layout workspace. | Output module. |
| Go to Slideshow | `Ctrl+Alt+5` | `Cmd+Opt+5` | global | out_of_scope | Slideshow workspace. | Output module. |
| Go to Print | `Ctrl+Alt+6` | `Cmd+Opt+6` | global | out_of_scope | Print layout workspace. | Output module. |
| Go to Web | `Ctrl+Alt+7` | `Cmd+Opt+7` | global | out_of_scope | Web-gallery workspace. | Output module. |
| Go back / forward | `Ctrl+Alt+Left / Ctrl+Alt+Right` | `Cmd+Opt+Left / Cmd+Opt+Right` | global | core | Navigate module history backward/forward. |  |
| Go back to previous module | `Ctrl+Alt+Up` | `Cmd+Opt+Up` | global | core | Return to the last module used. |  |

## Views & screen modes

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Enter Loupe view | `E` | `E` | module:library | core | Single large photo view. |  |
| Enter Grid view | `G` | `G` | module:library | core | Thumbnail grid of the catalog/folder. |  |
| Enter Compare view | `C` | `C` | module:library | core | Side-by-side select-vs-candidate comparison. |  |
| Enter Survey view | `N` | `N` | module:library | core | Multi-photo survey to narrow a selection. |  |
| Open selected photo in Develop | `D` | `D` | module:library | core | Send the active photo to the edit workspace. |  |
| Cycle Lights Out modes | `L / Shift+L` | `L / Shift+L` | global | optional | Dim/black-out the UI around the photo; Shift reverses. |  |
| Toggle Lights Dim | `Ctrl+Shift+L` | `Cmd+Shift+L` | global | optional | Jump straight to the dim state. |  |
| Cycle screen modes | `F` | `F` | global | core | Cycle normal/full-screen window modes. |  |
| Previous screen mode | `Shift+F` | `Shift+F` | global | optional | Step back through screen modes (macOS). | macOS-listed. |
| Normal/full-screen, hide panels | `Ctrl+Shift+F` | `Cmd+Shift+F` | global | optional | Full-screen with panels hidden. |  |
| Go to Normal screen mode | `Ctrl+Alt+F` | `Cmd+Opt+F` | global | optional | Force the standard windowed mode. |  |
| Cycle info overlay | `I` | `I` | global | core | Cycle the on-image info overlay sets. |  |
| Show/hide info overlay | `Ctrl+I` | `Cmd+I` | global | core | Toggle the on-image metadata overlay. |  |
| Open Reference view | `Shift+R` | `Shift+R` | module:develop | core | Pin a reference photo beside the one being edited. |  |
| Enable loupe overlay | `Ctrl+Alt+O` | `Cmd+Opt+O` | module:library | optional | Show layout/guide overlay over the loupe. |  |
| Enable & choose loupe overlay | `Ctrl+Shift+Alt+O` | `Cmd+Shift+Opt+O` | module:library | optional | Pick which loupe overlay to display. |  |
| Show Grid view styles | `Ctrl+Shift+X` | `Cmd+Shift+X` | module:library | optional | Cycle compact/expanded grid cell styles. |  |
| Zoom to 100% | `Ctrl+Alt+0` | `Cmd+Opt+0` | global | core | Jump to 1:1 pixel zoom. |  |
| Open People View | `O` | `O` | module:library | optional | Face-recognition grouping view. | Collides with Develop O (overlay). |
| Toggle HDR view | `Shift+H` | `Shift+H` | global | optional | Toggle HDR preview rendering. | Requires HDR pipeline. |

## Secondary window

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Open secondary window | `F11` | `Cmd+F11` | global | optional | Open a second display window. | Same as Library shortcuts + Shift. |
| Enter Grid (secondary) | `Shift+G` | `Shift+G` | secondary | optional | Grid view on the secondary window. |  |
| Enter Loupe (secondary) | `Shift+E` | `Shift+E` | secondary | optional | Normal loupe on the secondary window. |  |
| Enter locked Loupe (secondary) | `Ctrl+Shift+Enter` | `Cmd+Shift+Return` | secondary | optional | Lock the loupe to the current photo. |  |
| Enter Compare (secondary) | `Shift+C` | `Shift+C` | secondary | optional | Compare view on secondary window. |  |
| Enter Survey (secondary) | `Shift+N` | `Shift+N` | secondary | optional | Survey view on secondary window. |  |
| Enter Slideshow (secondary) | `Ctrl+Alt+Shift+Enter` | `Cmd+Opt+Shift+Return` | secondary | out_of_scope | Slideshow on secondary window. |  |
| Full-screen secondary | `Shift+F11` | `Cmd+Shift+F11` | secondary | optional | Full-screen the second display. | Needs 2nd monitor. |
| Show/hide filter bar (secondary) | `Shift+\` | `Shift+\` | secondary | optional | Toggle filter bar on secondary. |  |
| Zoom in/out (secondary) | `Ctrl+Shift+= / Ctrl+Shift+-` | `Cmd+Shift+= / Cmd+Shift+-` | secondary | optional | Zoom the secondary window. |  |

## Photos & catalog management

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Import photos from disk | `Ctrl+Shift+I` | `Cmd+Shift+I` | global | core | Open the import dialog. |  |
| Open catalog | `Ctrl+O` | `Cmd+Shift+O` | global | optional | Open a different catalog. | Catalog concept; map to library/db. |
| Open Preferences | `Ctrl+,` | `Cmd+,` | global | core | Application preferences. |  |
| Open Catalog Settings | `Ctrl+Alt+,` | `Cmd+Opt+,` | global | optional | Per-catalog settings. |  |
| New subfolder (tethered) | `Ctrl+Shift+T` | `Cmd+Shift+T` | module:library | out_of_scope | Segment tethered capture into a subfolder. | Tethering-specific. |
| Hide/show tether bar | `Ctrl+T` | `Cmd+T` | module:library | out_of_scope | Toggle the tethered-capture bar. | Tethering-specific. |
| Create new folder | `Ctrl+Shift+N` | `Cmd+Shift+N` | module:library | core | Make a new folder on disk in the library tree. |  |
| Create virtual copy | `Ctrl+'` | `Cmd+'` | module:library,module:develop | core | Create a non-destructive virtual duplicate for alternate edits. |  |
| Show in Explorer/Finder | `Ctrl+R` | `Cmd+R` | module:library,module:develop | core | Reveal the original file in the OS file manager. | Collides with Book/Print/Web Ctrl+R. |
| Next/previous photo in filmstrip | `Right / Left` | `Right / Left` | global | core | Step through the filmstrip selection. |  |
| Select multiple folders/collections | `Shift-click / Ctrl-click` | `Shift-click / Cmd-click` | module:library | core | Range or discrete multi-select in the source tree. | Click-modifier. |
| Rename photo | `F2` | `F2` | module:library | core | Rename the selected file. |  |
| Delete selected photo(s) | `Backspace or Delete` | `Delete` | module:library | core | Remove from catalog (prompts for disk delete). | DESTRUCTIVE — confirm dialog required. |
| Remove from catalog | `Alt+Backspace` | `Opt+Delete` | module:library | core | Remove the reference but keep the file on disk. |  |
| Delete & move to Recycle/Trash | `Ctrl+Alt+Shift+Backspace` | `Cmd+Opt+Shift+Delete` | module:library | core | Remove from catalog and send file to OS trash. | DESTRUCTIVE. |
| Delete rejected photo(s) | `Ctrl+Backspace` | `Cmd+Delete` | module:library | core | Purge all photos flagged as reject. | DESTRUCTIVE — batch. |
| Edit in Photoshop | `Ctrl+E` | `Cmd+E` | module:library,module:develop | optional | Hand off to an external pixel editor. | External-editor hook. |
| Open in other editor | `Ctrl+Alt+E` | `Cmd+Opt+E` | module:library,module:develop | optional | Hand off to a secondary external editor. |  |
| Export selected photo(s) | `Ctrl+Shift+E` | `Cmd+Shift+E` | global | core | Open the export dialog. |  |
| Export with previous settings | `Ctrl+Alt+Shift+E` | `Cmd+Opt+Shift+E` | global | core | Re-run export with the last-used settings, no dialog. |  |
| Open plug-in manager | `Ctrl+Alt+Shift+,` | `Cmd+Opt+Shift+,` | global | optional | Manage installed plug-ins. |  |
| Print selected photo | `Ctrl+P` | `Cmd+P` | global | out_of_scope | Send to print. | Output module. |
| Open Page Setup | `Ctrl+Shift+P` | `Cmd+Shift+P` | global | out_of_scope | Printer page setup. | Output module. |
| Go to next image | `Ctrl+Right` | `Cmd+Right` | global | core | Advance selection (catalog order). |  |
| Go to previous image | `Ctrl+Left` | `Cmd+Left` | global | core | Step selection back. |  |
| Tethered capture | `F12` | `F12` | global | out_of_scope | Trigger a tethered shutter capture. | Tethering-specific. |
| Headless enhance | `Ctrl+Alt+Shift+I` | `Ctrl+Alt+Shift+I` | global | core | Run Enhance (Denoise/Super Res) with last settings, no dialog. | AI/ML feature. |
| Open enhance dialog | `Ctrl+Alt+I` | `Ctrl+Alt+I` | global | core | Open the Enhance dialog (Denoise/Super Resolution/Raw Details). | AI/ML feature. |
| HDR merge | `Ctrl+H` | `Ctrl+H` | global | optional | Merge selected exposures into an HDR DNG. |  |
| Headless HDR merge | `Ctrl+Shift+H` | `Ctrl+Shift+H` | global | optional | HDR merge with last settings, no dialog. | Collides w/ Develop 'always show overlay'. |
| Pano merge | `Ctrl+M` | `Ctrl+M` | global | optional | Stitch selected frames into a panorama DNG. |  |
| Headless pano merge | `Ctrl+Shift+M` | `Ctrl+Shift+M` | global | optional | Pano merge with last settings, no dialog. |  |
| Open as Smart Object in PS | `Ctrl+Alt+X` | `Cmd+Opt+X` | global | optional | Send to Photoshop as a Smart Object. | External-editor hook. |
| E-mail photos | `n/a` | `Cmd+Shift+M` | global | optional | Email selected photos (macOS). | macOS-only; collides w/ pano on mac. |

## Comparing & selecting (Library)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Switch to Loupe | `E or Enter` | `E or Return` | module:library | core | Go to single-photo view. |  |
| Switch to Grid | `G or Esc` | `G or Esc` | module:library | core | Go to thumbnail grid. |  |
| Switch to Compare | `C` | `C` | module:library | core | Select-vs-candidate comparison. |  |
| Switch to Survey | `N` | `N` | module:library | core | Multi-photo survey. |  |
| Grid→Loupe | `Spacebar or E` | `Spacebar or E` | module:library | core | Open the focused thumbnail in loupe. |  |
| Swap select & candidate (Compare) | `Down` | `Down` | module:library | core | Promote the candidate to select. |  |
| Next select & candidate (Compare) | `Up` | `Up` | module:library | core | Advance the comparison pair. |  |
| Toggle Zoom | `Z` | `Z` | global | core | Toggle between fit and zoomed preview. |  |
| Zoom in/out (Loupe) | `Ctrl+= / Ctrl+-` | `Cmd+= / Cmd+-` | module:library | core | Step zoom level in loupe. |  |
| Scroll zoomed photo | `Page Up / Page Down` | `Page Up / Page Down` | global | optional | Pan a zoomed image vertically. | Also Develop/Web. |
| Go to start/end of grid | `Home / End` | `Home / End` | module:library | core | Jump to first/last thumbnail. |  |
| Play impromptu slideshow | `Ctrl+Enter` | `Cmd+Return` | global | optional | Quick full-screen slideshow of the selection. |  |
| Rotate right (CW) | `Ctrl+]` | `Cmd+]` | global | core | Rotate the photo 90° clockwise. |  |
| Rotate left (CCW) | `Ctrl+[` | `Cmd+[` | global | core | Rotate the photo 90° counter-clockwise. |  |
| Increase/decrease thumbnail size | `= / -` | `= / -` | module:library | core | Grow/shrink grid thumbnails. |  |
| Scroll grid thumbnails | `Page Up / Page Down` | `Page Up / Page Down` | module:library | optional | Page through the grid. |  |
| Toggle cell extras | `Ctrl+Shift+H` | `Cmd+Shift+H` | module:library | optional | Show/hide grid cell info badges/extras. |  |
| Show/hide badges | `Ctrl+Alt+Shift+H` | `Cmd+Opt+Shift+H` | module:library | optional | Toggle thumbnail badge icons. |  |
| Cycle Grid views | `J` | `J` | module:library | optional | Cycle compact/expanded/none grid styles. |  |
| Open Library view options | `Ctrl+J` | `Cmd+J` | module:library | optional | Grid/loupe view option dialog. |  |
| Select multiple discrete | `Ctrl-click` | `Cmd-click` | module:library | core | Add/remove individual photos to selection. | Click-modifier. |
| Select multiple contiguous | `Shift-click` | `Shift-click` | module:library | core | Range-select photos. | Click-modifier. |
| Select all | `Ctrl+A` | `Cmd+A` | module:library | core | Select every photo in the view. |  |
| Deselect all | `Ctrl+D` | `Cmd+D / Cmd+Shift+A` | module:library | core | Clear the selection. |  |
| Select only active photo | `Ctrl+Shift+D` | `Cmd+Shift+D` | module:library | core | Reduce selection to the active photo. |  |
| Deselect active photo | `/` | `/` | module:library | optional | Drop the active photo from a multi-selection. |  |
| Add prev/next to selection | `Shift+Left / Shift+Right` | `Shift+Left / Shift+Right` | module:library | core | Extend selection by one in either direction. |  |
| Select flagged photos | `Ctrl+Alt+A` | `Cmd+Opt+A` | module:library | core | Select all picks. |  |
| Deselect unflagged photos | `Ctrl+Alt+Shift+D` | `Cmd+Opt+Shift+D` | module:library | optional | Trim selection to flagged only. |  |
| Group into stack | `Ctrl+G` | `Cmd+G` | module:library | optional | Stack the selected photos. |  |
| Unstack | `Ctrl+Shift+G` | `Cmd+Shift+G` | module:library | optional | Break the current stack apart. |  |
| Toggle stack | `S` | `S` | module:library | optional | Collapse/expand a stack. | Collides w/ Develop soft-proof S. |
| Move to top of stack | `Shift+S` | `Shift+S` | module:library | optional | Promote photo to stack top. |  |
| Move up in stack | `Shift+[` | `Shift+[` | module:library | optional | Reorder within a stack. |  |
| Move down in stack | `Shift+]` | `Shift+]` | module:library | optional | Reorder within a stack. |  |

## Rating, flagging & filtering

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Set star rating | `1–5` | `1–5` | global | core | Apply 1–5 star rating to the selection. |  |
| Set rating & advance | `Shift+1–5` | `Shift+1–5` | global | core | Rate then auto-advance to next photo (fast culling). |  |
| Remove star rating | `0` | `0` | global | core | Clear the star rating. |  |
| Remove rating & advance | `Shift+0` | `Shift+0` | global | optional | Clear rating then advance. |  |
| Increase/decrease rating | `] / [` | `] / [` | global | core | Bump rating up/down by one star. | Collides w/ brush-size in Develop. |
| Assign red label | `6` | `6` | global | optional | Apply red color label. |  |
| Assign yellow label | `7` | `7` | global | optional | Apply yellow color label. |  |
| Assign green label | `8` | `8` | global | optional | Apply green color label. |  |
| Assign blue label | `9` | `9` | global | optional | Apply blue color label. |  |
| Assign label & advance | `Shift+6–9` | `Shift+6–9` | global | optional | Label then advance. |  |
| Flag as pick | `P` | `P` | global | core | Mark as pick. |  |
| Flag as pick & advance | `Shift+P` | `Shift+P` | global | core | Pick then advance (culling). |  |
| Flag as reject | `X` | `X` | global | core | Mark as reject. | Collides w/ Develop crop-orientation X. |
| Flag as reject & advance | `Shift+X` | `Shift+X` | global | core | Reject then advance. |  |
| Unflag | `U` | `U` | global | core | Clear the pick/reject flag. |  |
| Unflag & advance | `Shift+U` | `Shift+U` | global | optional | Unflag then advance. |  |
| Increase/decrease flag status | `Ctrl+Up / Ctrl+Down` | `Cmd+Up / Cmd+Down` | global | optional | Step pick↔unflagged↔reject. |  |
| Cycle flag settings | `` (back quote)` | `` (back quote)` | global | optional | Cycle through flag states. |  |
| Refine photos | `Ctrl+Alt+R` | `Cmd+Opt+R` | module:library | optional | Promote picks/reset flags to refine a cull. | Collides w/ Develop reset-crop. |
| Show/hide Filter bar | `\` | `\` | module:library | core | Toggle the Library filter bar. |  |
| Open multiple filters | `Shift-click labels` | `Shift-click labels` | module:library | optional | Combine filter criteria. | Click-modifier. |
| Toggle filters on/off | `Ctrl+L` | `Cmd+L` | module:library | core | Enable/disable active filters. |  |
| Find photo | `Ctrl+F` | `Cmd+F` | module:library | core | Focus the text search field. |  |

## Collections / Quick Collection

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Create new collection | `Ctrl+N` | `Cmd+N` | module:library | optional | Make a saved collection. | Collides w/ Develop new-snapshot. |
| Add to Quick Collection | `B` | `B` | global | optional | Toss the photo into the temporary Quick Collection. |  |
| Add to Quick Collection & advance | `Shift+B` | `Shift+B` | global | optional | Add then advance. |  |
| Show Quick Collection | `Ctrl+B` | `Cmd+B` | module:library | optional | View the Quick Collection contents. |  |
| Save Quick Collection | `Ctrl+Alt+B` | `Cmd+Opt+B` | module:library | optional | Persist Quick Collection as a saved collection. |  |
| Clear Quick Collection | `Ctrl+Shift+B` | `Cmd+Shift+B` | module:library | optional | Empty the Quick Collection. |  |
| Set as target collection | `Ctrl+Alt+Shift+B` | `Cmd+Opt+Shift+B` | module:library | optional | Choose which collection B adds to. |  |

## Metadata & keywords (Library)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Add keywords | `Ctrl+K` | `Cmd+K` | module:library | core | Focus the keyword entry field. |  |
| Edit keywords | `Ctrl+Shift+K` | `Cmd+Shift+K` | module:library | optional | Open the keywording panel for editing. |  |
| Set a keyword shortcut | `Ctrl+Alt+Shift+K` | `Cmd+Opt+Shift+K` | module:library | optional | Define the one-tap keyword. |  |
| Add/remove keyword shortcut | `Shift+K` | `Shift+K` | module:library | optional | Toggle the shortcut keyword on the selection. |  |
| Enable painting | `Ctrl+Alt+K` | `Cmd+Opt+K` | module:library | optional | Painter tool to brush metadata onto thumbnails. |  |
| Add keyword from set | `Alt+1–9` | `Opt+1–9` | module:library | optional | Apply a keyword from the active keyword set. |  |
| Cycle keyword sets | `Alt+0 / Alt+Shift+0` | `Opt+0 / Opt+Shift+0` | module:library | optional | Switch active keyword set. |  |
| Copy/paste metadata | `Ctrl+Alt+Shift+C / +V` | `Cmd+Opt+Shift+C / +V` | module:library | core | Copy metadata from one photo and paste to others. |  |
| Save metadata to file | `Ctrl+S` | `Cmd+S` | module:library | core | Write metadata/edits to XMP sidecar or file header. | Ties to XMP/ACR sidecar model. |
| Open Spelling dialog | `n/a` | `Cmd+:` | module:library | out_of_scope | Spell-check (macOS). | macOS text service. |
| Check spelling | `n/a` | `Cmd+;` | module:library | out_of_scope | Spell-check (macOS). | macOS text service. |
| Open Character palette | `n/a` | `Cmd+Opt+T` | module:library | out_of_scope | System character palette (macOS). | macOS text service. |
| Visual Search | `Ctrl+Alt+Shift+F` | `Cmd+Opt+Shift+F` | module:library | optional | Search by visual similarity. | AI/ML feature. |
| Edit Face name | `Shift+O` | `Shift+O` | module:library | optional | Name a detected face in People view. |  |

## Develop module — global edit ops

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Toggle Use Generative AI | `Alt+Shift+G` | `Opt+Shift+G` | module:develop | optional | Switch the Remove tool to generative fill mode. | AI/ML feature. |
| Toggle Detect Aware | `Alt+Shift+O` | `Opt+Shift+O` | module:develop | optional | Toggle content-aware detection for Remove. | AI/ML feature. |
| Cycle Generative Remove variations | `Alt+Side arrows` | `Opt+Side arrows` | module:develop | optional | Step through generated fill candidates. | AI/ML feature. |
| Convert to grayscale | `V` | `V` | module:develop | core | Toggle B&W treatment. |  |
| Auto tone | `Ctrl+U` | `Cmd+U` | module:develop | core | Apply automatic tone (exposure/contrast/etc.). | ML auto. |
| Auto white balance | `Ctrl+Shift+U` | `Cmd+Shift+U` | module:develop | core | Apply automatic white balance. | ML auto. |
| Copy/paste Develop settings | `Ctrl+Shift+C / +V` | `Cmd+Shift+C / +V` | module:develop | core | Copy a recipe of edits and paste onto other photos. |  |
| Paste settings from previous | `Ctrl+Alt+V` | `Cmd+Opt+V` | module:develop | core | Apply the immediately previous photo's settings. |  |
| Copy After→Before | `Ctrl+Alt+Shift+Left` | `Cmd+Opt+Shift+Left` | module:develop | optional | Set the before state to the current edit. |  |
| Copy Before→After | `Ctrl+Alt+Shift+Right` | `Cmd+Opt+Shift+Right` | module:develop | optional | Reset the edit to the before state. |  |
| Swap Before/After | `Ctrl+Alt+Shift+Up` | `Cmd+Opt+Shift+Up` | module:develop | optional | Exchange before and after states. |  |
| Nudge slider (small) | `Up / Down or + / -` | `Up / Down or + / -` | module:develop | core | Fine adjust the focused slider. |  |
| Nudge slider (large) | `Shift+Up / Shift+Down` | `Shift+Up / Shift+Down` | module:develop | core | Coarse adjust the focused slider. |  |
| Cycle Basic panel settings | `./ ,` | `./ ,` | module:develop | core | Move focus forward/back through Basic sliders. |  |
| Reset a slider | `Double-click slider name` | `Double-click slider name` | module:develop | core | Return one slider to default. | Click action. |
| Reset a group of sliders | `Alt-click group name` | `Opt-click group name` | module:develop | core | Reset a whole panel section. | Click-modifier. |
| Reset all settings | `Ctrl+Shift+R` | `Cmd+Shift+R` | module:develop | core | Revert the photo to defaults. |  |
| Sync settings | `Ctrl+Shift+S` | `Cmd+Shift+S` | module:develop | core | Sync chosen settings across selected photos (with dialog). |  |
| Sync (no dialog) | `Ctrl+Alt+S` | `Cmd+Opt+S` | module:develop | core | Sync using the last sync selections. |  |
| Toggle Auto Sync | `Ctrl-click Sync` | `Cmd-click Sync` | module:develop | optional | Make slider changes apply live to all selected. | Click-modifier. |
| Enable Auto Sync | `Ctrl+Alt+Shift+A` | `Cmd+Opt+Shift+A` | module:develop | optional | Turn on auto-sync mode. |  |
| Match total exposures | `Ctrl+Alt+Shift+M` | `Cmd+Opt+Shift+M` | module:develop | optional | Normalize exposure across a selection. |  |

## Develop module — tools

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| White Balance tool | `W` | `W` | global | core | Eyedropper to set neutral white balance. |  |
| Crop tool | `R` | `R` | global | core | Enter crop/straighten. |  |
| Constrain aspect ratio (crop) | `A` | `A` | module:develop | core | Lock the crop aspect ratio. |  |
| Crop to previous ratio | `Shift+A` | `Shift+A` | module:develop | optional | Reuse the last crop ratio. |  |
| Crop from center | `Alt-drag` | `Opt-drag` | module:develop | optional | Grow the crop symmetrically from center. | Click-modifier. |
| Cycle crop grid overlay | `O` | `O` | module:develop | optional | Cycle thirds/golden/etc. crop guides. | Collides w/ Library People view. |
| Cycle crop overlay orientation | `Shift+O` | `Shift+O` | module:develop | optional | Rotate the crop guide orientation. |  |
| Toggle crop orientation | `X` | `X` | module:develop | core | Swap crop between portrait/landscape. | Collides w/ Library reject. |
| Reset crop | `Ctrl+Alt+R` | `Cmd+Opt+R` | module:develop | core | Clear the crop. | Collides w/ Library refine. |
| Guided Upright tool | `Shift+T` | `Shift+T` | module:develop | optional | Draw guides to correct perspective. | Context-overloaded Shift+T (see conflicts). |
| Spot Removal tool | `Q` | `Q` | module:develop | core | Heal/clone blemishes and dust. |  |
| Toggle Clone/Heal | `Shift+T` | `Shift+T` | module:develop | core | Switch Spot Removal between clone and heal. | Same key, tool-dependent. |
| Adjustment Brush | `K` | `K` | global | core | Freehand local-adjustment brush. |  |
| Graduated Filter | `M` | `M` | module:develop | core | Linear gradient local adjustment. |  |
| Toggle Mask Edit/Brush | `Shift+T` | `Shift+T` | module:develop | core | Switch gradient/radial mask between edit and brush. | Same key, tool-dependent. |
| Increase/decrease brush size | `] / [` | `] / [` | module:develop | core | Resize the active brush. |  |
| Increase/decrease brush feather | `Shift+] / Shift+[` | `Shift+] / Shift+[` | module:develop | core | Soften/harden the brush edge. |  |
| Switch brush A/B | `/` | `/` | module:develop | core | Toggle the two brush presets. |  |
| Temporary eraser | `Alt-drag` | `Opt-drag` | module:develop | core | Erase mask while held. | Click-modifier. |
| Paint straight line | `Shift-drag` | `Shift-drag` | module:develop | optional | Constrain a brush stroke to horizontal/vertical. | Click-modifier. |
| Adjust Amount via pin | `Drag pin right/left` | `Drag pin right/left` | module:develop | optional | Scale the local adjustment by dragging its pin. | Drag action. |
| Show/hide local pin | `H` | `H` | module:develop | core | Toggle the adjustment pin marker. | Also 'never show overlay'. |
| Show/hide mask overlay | `O` | `O` | module:develop | core | Toggle the colored mask overlay. |  |
| Cycle mask overlay color | `Shift+O` | `Shift+O` | module:develop | optional | Change the mask overlay tint. |  |

## Develop module — targeted adjustment (TAT) & masks

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| TAT: Tone Curve | `Ctrl+Alt+Shift+T` | `Cmd+Opt+Shift+T` | module:develop | optional | Drag on image to adjust the tone curve at that tone. |  |
| TAT: Hue | `Ctrl+Alt+Shift+H` | `Cmd+Opt+Shift+H` | module:develop | optional | Drag on image to shift hue of the sampled color. |  |
| TAT: Saturation | `Ctrl+Alt+Shift+S` | `Cmd+Opt+Shift+S` | module:develop | optional | Drag on image to adjust saturation of sampled color. |  |
| TAT: Luminance | `Ctrl+Alt+Shift+L` | `Cmd+Opt+Shift+L` | module:develop | optional | Drag on image to adjust luminance of sampled color. |  |
| TAT: Grayscale Mix | `Ctrl+Alt+Shift+G` | `Cmd+Opt+Shift+G` | module:develop | optional | Drag to adjust B&W channel mix for sampled color. |  |
| Deselect TAT | `Ctrl+Alt+Shift+N` | `Cmd+Opt+Shift+N` | module:develop | optional | Turn off the targeted adjustment tool. |  |
| Show clipping | `J` | `J` | module:develop | core | Toggle highlight/shadow clipping warnings. | Collides w/ Library cycle-grid J. |
| Toggle Loupe/1:1 zoom | `Spacebar or Z` | `Spacebar or Z` | module:develop | core | Toggle fit and 1:1 preview. |  |
| Zoom in/out | `Ctrl+= / Ctrl+-` | `Cmd+= / Cmd+-` | module:develop | core | Step zoom in Develop. |  |
| Before/After left-right | `Y` | `Y` | module:develop | core | Side-by-side before/after. |  |
| Before/After top-bottom | `Alt+Y` | `Opt+Y` | module:develop | optional | Stacked before/after. |  |
| Before/After split | `Shift+Y` | `Shift+Y` | module:develop | optional | Split-screen before/after. |  |
| View Before only | `\` | `\` | module:develop | core | Show the unedited state while held/toggled. |  |
| New snapshot | `Ctrl+N` | `Cmd+N` | module:develop | optional | Save a named state of the current edit. | Collides w/ Library new-collection. |
| New preset | `Ctrl+Shift+N` | `Cmd+Shift+N` | module:develop | core | Save current settings as a reusable preset. |  |
| New preset folder | `Ctrl+Alt+N` | `Cmd+Opt+N` | module:develop | optional | Organize presets into a folder. |  |
| Open Develop view options | `Ctrl+J` | `Cmd+J` | module:develop | optional | Develop view/overlay options. |  |
| Create Radial filter | `Shift+M` | `Shift+M` | module:develop | core | Radial gradient local adjustment. |  |
| Create Luminance mask | `Shift+Q` | `Shift+Q` | module:develop | optional | Luminance-range mask. |  |
| Crop to original | `Ctrl+Alt+Shift+R` | `Cmd+Opt+Shift+R` | module:develop | optional | Reset crop to the full original frame. |  |
| Create color range mask | `Shift+J` | `Shift+J` | module:develop | optional | Color-range mask. |  |
| Cycle Overlay mode (crop/mask) | `Alt+O` | `Opt+O` | module:develop | optional | Switch overlay rendering mode. |  |
| Lock zoom position | `Shift+Ctrl+=` | `Shift+Cmd+=` | module:develop | optional | Pin the zoom focus point. | Also Library. |
| Open/close Masking | `Shift+W` | `Shift+W` | module:develop | core | Toggle the Masking panel. |  |
| Hide/unhide gamut warning (soft proof) | `Shift+S` | `Shift+S` | module:develop | optional | Toggle gamut destination warning. | Soft-proofing. |
| Expand/collapse soft proofing | `S` | `S` | module:develop | optional | Toggle soft-proof view. | Collides w/ Library toggle-stack. |
| Never show overlay/pins | `H` | `H` | module:develop | core | Hide pins/overlays permanently. |  |
| Always show overlay/pins | `Ctrl+Shift+H` | `Cmd+Shift+H` | module:develop | optional | Always show pins/overlays. | Collides w/ headless HDR. |
| Cycle transform options | `Ctrl+Tab` | `Ctrl+Tab` | module:develop | optional | Step through Upright transform modes. |  |
| Copy a Mask | `Ctrl+C` | `Cmd+C` | module:develop | optional | Duplicate a mask to reuse. |  |

## Help

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Display current module shortcuts | `Ctrl+/` | `Cmd+/` | global | core | Overlay the shortcut cheat-sheet for the active context. |  |
| Hide current module shortcuts | `Click` | `Click` | global | core | Dismiss the shortcut overlay. | Click action. |
| Go to current module Help | `Ctrl+Alt+/` | `Cmd+Opt+Shift+/` | global | optional | Open online help for the active module. |  |
| Open Community Help | `F1` | `F1` | global | optional | Open the help/community site. |  |

## Book module (out of scope)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Double page view | `Ctrl+R` | `Cmd+R` | module:book | out_of_scope | Book spread view. |  |
| Multi-page view | `Ctrl+E` | `Cmd+E` | module:book | out_of_scope | Book multi-page view. |  |
| Next/previous view mode | `Ctrl+= / Ctrl+-` | `Cmd+= / Cmd+-` | module:book | out_of_scope | Cycle book view modes. |  |
| Single page view | `Ctrl+T` | `Cmd+T` | module:book | out_of_scope | Single page view. |  |
| Zoomed single-page view | `Ctrl+U` | `Cmd+U` | module:book | out_of_scope | Zoomed page view. |  |
| Create Saved Book | `Ctrl+S` | `Cmd+S` | module:book | out_of_scope | Save the book. |  |
| Update metadata captions | `Ctrl+M` | `Cmd+M` | module:book | out_of_scope | Refresh caption text. |  |
| Copy/paste book layout | `Ctrl+Shift+C / +V` | `Cmd+Shift+C / +V` | module:book | out_of_scope | Copy a page layout. |  |
| Remove page | `Ctrl+Shift+Backspace` | `Cmd+Shift+Backspace` | module:book | out_of_scope | Delete a page. |  |
| Select photo cells | `Ctrl+Shift+Alt+A` | `Cmd+Shift+Opt+A` | module:book | out_of_scope | Select image cells. |  |
| Select text cells | `Ctrl+Alt+A` | `Cmd+Opt+A` | module:book | out_of_scope | Select text cells. |  |
| Show/hide filter text | `Ctrl+Shift+H` | `Cmd+Shift+H` | module:book | out_of_scope | Toggle filter text. |  |
| Show/hide text safe area | `Ctrl+Shift+U` | `Cmd+Shift+U` | module:book | out_of_scope | Toggle safe-area guides. |  |

## Slideshow module (out of scope)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Play slideshow | `Enter` | `Return` | module:slideshow | out_of_scope | Start playback. |  |
| Pause slideshow | `Spacebar` | `Spacebar` | module:slideshow | out_of_scope | Pause playback. |  |
| Preview slideshow | `Alt+Enter` | `Opt+Return` | module:slideshow | out_of_scope | Preview in panel. |  |
| End slideshow | `Esc` | `Esc` | module:slideshow | out_of_scope | Stop playback. |  |
| Next/previous slide | `Right / Left` | `Right / Left` | module:slideshow | out_of_scope | Step slides. |  |
| Export PDF slideshow | `Ctrl+J` | `Cmd+J` | module:slideshow | out_of_scope | Export to PDF. |  |
| Export JPEG slideshow | `Ctrl+Shift+J` | `Cmd+Shift+J` | module:slideshow | out_of_scope | Export to JPEG. |  |
| Export video slideshow | `Ctrl+Alt+J` | `Cmd+Opt+J` | module:slideshow | out_of_scope | Export to video. |  |
| New template / folder | `Ctrl+N / Ctrl+Shift+N` | `Cmd+N / Cmd+Shift+N` | module:slideshow | out_of_scope | Create template/folder. |  |
| Save settings | `Ctrl+S` | `Cmd+S` | module:slideshow | out_of_scope | Save slideshow. |  |

## Print module (out of scope)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Print | `Ctrl+P` | `Cmd+P` | module:print | out_of_scope | Send to printer. |  |
| Print one copy | `Ctrl+Alt+P` | `Cmd+Opt+P` | module:print | out_of_scope | Print a single copy. |  |
| Page Setup / Print Settings | `Ctrl+Shift+P / Ctrl+Alt+Shift+P` | `Cmd+Shift+P / Cmd+Opt+Shift+P` | module:print | out_of_scope | Printer dialogs. |  |
| Navigate pages | `Ctrl+Shift+Left/Right, Ctrl+Left/Right` | `Cmd+Shift+Left/Right, Cmd+Left/Right` | module:print | out_of_scope | Move between pages. |  |
| Toggle guides/rulers/bleed/margins/cells/dims | `Ctrl+Shift+H/…` | `Cmd+Shift+H/…` | module:print | out_of_scope | Layout guide toggles. |  |
| New template/folder, save | `Ctrl+N / Ctrl+Shift+N / Ctrl+S` | `Cmd+N / Cmd+Shift+N / Cmd+S` | module:print | out_of_scope | Template management. |  |

## Map module (out of scope)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Next/previous track | `Ctrl+Alt+T / Ctrl+Alt+Shift+T` | `Cmd+Opt+T / Cmd+Opt+Shift+T` | module:map | out_of_scope | Tracklog navigation. |  |
| Search | `Ctrl+F` | `Cmd+F` | module:map | out_of_scope | Search the map. |  |
| Delete all location data | `Ctrl+Backspace` | `Cmd+Delete` | module:map | out_of_scope | Strip geotags. |  |
| Map styles (Hybrid/Road/Sat/Terrain/Light/Dark) | `Ctrl+1–6` | `Cmd+1–6` | module:map | out_of_scope | Switch base map style. |  |
| Lock pins | `Ctrl+K` | `Cmd+K` | module:map | out_of_scope | Lock map pins. |  |
| Show/hide map info / preset overlay | `I / O` | `I / O` | module:map | out_of_scope | Toggle map overlays. |  |

## Web module (out of scope)

| Action | Windows | macOS | Scope | Rel | What it does | Notes |
|---|---|---|---|---|---|---|
| Reload gallery | `Ctrl+R` | `Cmd+R` | module:web | out_of_scope | Rebuild the web gallery preview. |  |
| Preview in browser | `Ctrl+Alt+P` | `Cmd+Opt+P` | module:web | out_of_scope | Open gallery in a browser. |  |
| Export gallery | `Ctrl+J` | `Cmd+J` | module:web | out_of_scope | Export the gallery. |  |
| New template/folder, save | `Ctrl+N / Ctrl+Shift+N / Ctrl+S` | `Cmd+N / Cmd+Shift+N / Cmd+S` | module:web | out_of_scope | Template management. |  |
| Open advanced settings | `Ctrl+Shift+Alt+/` | `Cmd+Shift+Opt+/` | module:web | out_of_scope | Advanced gallery settings. |  |
