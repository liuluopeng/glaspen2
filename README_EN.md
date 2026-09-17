[中文](./README.md)

# Glaspen2

<p align="center"><img src="./introduct/icon.png" width="120"></p>

Separates the pen from the mouse.  
Like writing on a glass overlay in front of your monitor.

<p align="center"><img src="./introduct/demo.gif" width="480"></p>

A regular stylus is treated as a mouse by the OS — this app removes that behavior.  
Ideal for remote meetings, teaching, screen annotation, and quick notes.

## Platforms

- macOS
- Windows (reads the pen via Raw HID; pressure works, no Microsoft Ink required)

## Two canvas modes

Switch from the tabs at the top of the settings panel, or the "Infinite canvas" menu item. The two modes store strokes **independently**.

- **Notebook (paged) mode** — one page per screen, classic page flipping
  - `⌥⌘↑` / `⌥⌘↓`: slide the view up/down by a small step (about one browser arrow-key scroll); sliding past a page boundary switches to the neighbouring page automatically. Vertical movement only.
  - `⌘⌃J` / `⌘⌃K`: flip a whole page to previous / next (with a slide animation).
  - While scrolling, the neighbouring pages and page numbers are drawn too, with a heavier boundary line between pages.
  - Settings "Notebook" tab: a vertical minimap along the right edge (thumbnails of ~10 nearby pages).
- **Infinite canvas (free doodle) mode** — one canvas with no borders (currently a single global canvas)
  - `⌥⌘↑ / ⌥⌘↓ / ⌥⌘← / ⌥⌘→` pan the lens.
  - `⌘⌃scroll` pans, `⌥⇧scroll` zooms anchored at the pointer (capped at 100%), `⌘⌃PageUp` / `⌘⌃PageDown` zoom from the keyboard.

## Features

- **Quick GIF recording**
  Hold `⌘⌃R`, doodle, release — the doodle is turned into an animated GIF and copied to the clipboard.
  Configurable in settings: frame rate (10–50 fps), resolution (25%–100%), speed (0.5×–20×) and ending (stop on the last stroke / hold 1 s then loop / loop immediately), with a grey estimated file size.

- **Handwritten messages**
  WeChat and Douyin display small GIFs as stickers. Doodle on screen, then paste into WeChat to send a handwritten message (up to ~50 characters).
  Holding `⌘⌃3` also sends the handwriting as a single message.

<p align="center"><img src="./introduct/chat.jpg" width="480"></p>

- **Export** with background / without background / Xournal notes / SVG / GIF / PDF
  The infinite canvas can be exported on its own: a **paged PDF** (split by screen size) or a **whole SVG** (content bounding box, independent of the current lens).

- **Grid** adjustable spacing; a heavier boundary line is drawn only at multiples of the screen size (page boundary in Notebook mode, one per screen on the infinite canvas).

- **Stroke outline** a contrast outline around strokes so they read on light or dark backgrounds (render-only switch).

- **Color / width** full-saturation palette + 8 width presets.

- **Frosted glass background** blurs the desktop to make strokes stand out (mouse/keyboard still work with other apps).

- **Pressure monitor / rainbow indicator / launch at login.**

- **Bezier smoothing** via ink-stroke-modeler removes hand tremor and supports pressure width.

## Keyboard shortcuts

| Function | macOS | Windows |
| --- | --- | --- |
| New canvas | `⌘ + ⌃ + C` | `Ctrl + Alt + C` |
| Undo last stroke | `⌘ + ⌃ + Z` | `Ctrl + Alt + Z` |
| Toggle drawing | `⌘ + ⌃ + V` | `Ctrl + Alt + V` |
| Toggle canvas mode (fixed ↔ ethereal) | `⌘ + ⌃ + X` | `Ctrl + Alt + X` |
| Previous / next page (whole page) | `⌘ + ⌃ + J` / `⌘ + ⌃ + K` | `Ctrl + Alt + J` / `Ctrl + Alt + K` |
| Export SVG + GIF (copies to clipboard) | `⌘ + ⌃ + G` | `Ctrl + Alt + G` |
| Copy current doodle as SVG | `⌘ + ⌃ + S` | |
| Frosted glass toggle | `⌘ + ⌃ + B` | `Ctrl + Alt + B` |
| Open settings | `⌘ + ⌃ + ,` | |
| Quit | | `Ctrl + Alt + Q` |
| Quick GIF recording (hold) | `⌘ + ⌃ + R` | `Ctrl + Alt + R` (hold) |
| Record handwritten message (hold) | `⌘ + ⌃ + 3` | |
| Notebook: slide up/down (auto page switch) | `⌥ + ⌘ + ↑` / `⌥ + ⌘ + ↓` | |
| Infinite canvas: pan (four directions) | `⌥ + ⌘ + arrows` | `Ctrl + Alt + arrows` |
| Infinite canvas: zoom (cursor anchored) | `⌥ + ⇧ + scroll` | `Ctrl + Alt + scroll` |
| Infinite canvas: keyboard zoom | `⌘ + ⌃ + PageUp` / `⌘ + ⌃ + PageDown` | `Ctrl + Alt + PageUp` / `Ctrl + Alt + PageDown` |

## Installation

Follow the on-screen guide to grant Accessibility and Screen Recording permissions (used for global shortcuts and for saving screenshots with background).

## Dev Environment

- macOS: Rust + Flutter
- Windows: Rust + Flutter
