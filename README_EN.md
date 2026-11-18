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

- **Notebook (paged) mode** — one page per screen, classic page flipping (whole-page flip with the native window fade)
  - `⌘⌃~` / `⌘⌃1`: flip a whole page to previous / next (configurable in Settings → Shortcuts).
  - Page grid: **drag to reorder / batch multi-select delete**, plus lasso **move / copy / paste** in the full-page view.
- **Infinite canvas (free doodle) mode** — one canvas with no borders (currently a single global canvas)
  - `⌘⌃I` toggles free doodle (infinite canvas).
  - Left-hand camera: `⌥W / ⌥S / ⌥A / ⌥D` to pan; `⌥Z / ⌥X` to zoom
    (both configurable in Settings → Shortcuts).
  - `⌥⌘scroll` zooms anchored at the pointer (capped at 100%).
  - Panel "Edit content": lasso / drag to move / copy / paste / delete (shares the page clipboard).

## Features

- **Quick GIF recording**
  Hold `⌘⌃R`, doodle, release — the doodle is turned into an animated GIF and copied to the clipboard.
  Frame rate / resolution / playback speed are configurable in settings.

<p align="center"><img src="./introduct/chat.jpg" width="480"></p>

- **Export** screenshot with background / screenshot without background / Xournal notes / SVG / GIF / PDF
  The infinite canvas has its own exports: a **paged PDF** sized to the screen, or a **whole-canvas SVG** (content bounding box, unaffected by the current lens).

- **Grid** adjustable spacing; a heavier boundary line is drawn only at multiples of the screen size (page boundary in Notebook mode, one per screen on the infinite canvas).
  Optional **column guides**: halves or a 3×3 grid (purely visual).

- **Stroke outline / soft shadow** outline = black-and-white marching-ants dashes so strokes read on light or dark backgrounds; soft shadow = a gentle dark halo under the strokes, keeps them visible on same-color backgrounds. Both can be combined.

- **Color / width** full-saturation palette + 8 width presets.

- **Frosted glass background** blurs the desktop to make strokes stand out (mouse/keyboard still work with other apps).
  An experimental **true see-through** mode makes the cards fully transparent — the desktop behind the panel shows right through.

- **Pen buttons** with the driver set to Pen/Eraser or Eraser, pressing the pen button turns it into an eraser automatically (no configuration); set to Keyboard Key with a glaspen shortcut to flip pages / undo.

- **Pressure monitor / launch at login.**

- **Update check / auto-update** one-click check against GitHub releases; on macOS one-click auto-update (with automatic rollback).

- **Bezier smoothing** via ink-stroke-modeler removes hand tremor and supports pressure width.

## Architecture

```mermaid
flowchart LR
    subgraph native["Native layer"]
        MAC["macOS shell
ObjC: global pen/mouse events
transparent overlay · menu bar"]
        WIN["Windows overlay
Win32 window · Raw HID pen input"]
    end
    subgraph core["glaspen-core (Rust, platform-neutral)"]
        ENGINE["stroke smoothing (modeler)
cairo rendering · export
PDF / SVG / GIF / XOJ"]
        DB[("SQLite notebook
pages / strokes / settings")]
    end
    PANEL["Settings panel (Flutter)
Settings · Notebook · Free doodle"]

    MAC -- "FFI" --> core
    WIN -- "FFI" --> core
    PANEL <-. "FRB" .-> core
```

- **Native layer**: macOS uses ObjC (global events + overlay window), Windows is pure Rust Win32 — the doodling experience lives entirely here.
- **glaspen-core**: the platform-neutral shared core — storage, smoothing, rendering, export and updates.
- **Settings panel**: Flutter, talking to Rust in-process via FRB; by default only "Settings + Notebook" tabs are shown, advanced capabilities opt-in.

## Keyboard shortcuts

| Function | macOS | Windows |
| --- | --- | --- |
| New canvas | `⌘ + ⌃ + C` | `Ctrl + Alt + C` |
| Undo last stroke | `⌘ + ⌃ + Z` | `Ctrl + Alt + Z` |
| Toggle drawing | `⌘ + ⌃ + V` | `Ctrl + Alt + V` |
| Toggle canvas mode (fixed ↔ ethereal) | `⌘ + ⌃ + X` | `Ctrl + Alt + X` |
| Previous / next page (whole page) | `⌘ + ⌃ + ~ / 1` | `Ctrl + Alt + ~` / `Ctrl + Alt + 1` |
| Toggle free doodle (infinite canvas) | `⌘ + ⌃ + I` | `Ctrl + Alt + I` |
| Export SVG + GIF (copies to clipboard) | `⌘ + ⌃ + G` | `Ctrl + Alt + G` |
| Frosted glass toggle | `⌘ + ⌃ + B` | `Ctrl + Alt + B` |
| Open settings | `⌘ + ⌃ + D` | `Ctrl + Alt + D` |
| Quit | | `Ctrl + Alt + Q` |
| Quick GIF recording (hold) | `⌘ + ⌃ + R` | `Ctrl + Alt + R` (hold) |
| Infinite canvas: pan (four directions) | `⌥ + W / A / S / D` | `Ctrl + Alt + arrows` |
| Infinite canvas: keyboard zoom | `⌥ + Z / X` | `Ctrl + Alt + PgUp / PgDn` |
| Infinite canvas: zoom (cursor anchored) | `⌥ + ⌘ + scroll` | `Ctrl + Alt + scroll` |

## Installation

Follow the on-screen guide to grant Accessibility and Screen Recording permissions (used for global shortcuts and for saving screenshots with background).

## Dev Environment

- macOS: Rust + Flutter
- Windows: Rust + Flutter
