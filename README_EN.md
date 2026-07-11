[中文](./README.md)

# Glaspen2

<p align="center"><img src="./introduct/icon.svg" width="120"></p>

Separates the pen from the mouse.  
Like writing on a glass overlay in front of your monitor.

<p align="center"><img src="./introduct/demo.gif" width="480"></p>

A regular stylus is treated as a mouse by the OS — this app removes that behavior.  
Ideal for remote meetings, teaching, screen annotation, and quick notes.

## Platforms

- macOS
- Windows (Microsoft Ink must be enabled in tablet driver settings)

## Features

- **Handwritten messages**  
  WeChat and Douyin display small GIFs as stickers. You can doodle on screen, then paste into WeChat to send a handwritten message (up to ~50 characters).

<p align="center"><img src="./introduct/chat.jpg" width="480"></p>

- Export with background / without background / Xournal format
- Adjustable stroke width and color
- Frosted glass background (mouse and keyboard still work with other apps while blurred)
- Bezier smoothing to reduce hand tremor

## Keyboard Shortcuts

| Function                               | macOS       | Windows          |
| -------------------------------------- | ----------- | ---------------- |
| Clear screen                           | `⌘ + ⌃ + C` | `Ctrl + Alt + C` |
| Toggle drawing                         | `⌘ + ⌃ + V` | `Ctrl + Alt + V` |
| Previous page                          | `⌘ + ⌃ + J` | `Ctrl + Alt + J` |
| Next page                              | `⌘ + ⌃ + K` | `Ctrl + Alt + K` |
| Export SVG + GIF (copies to clipboard) | `⌘ + ⌃ + G` | `Ctrl + Alt + G` |
| Frosted glass toggle                   | `⌘ + ⌃ + B` | `Ctrl + Alt + B` |
| Open settings                          | `⌘ + ⌃ + ,` |                  |
| Quit                                   |             | `Ctrl + Alt + Q` |

## Installation

Follow the on-screen guide to grant Accessibility and Screen Recording permissions.

## Dev Environment

- macOS: Rust + Flutter
- Windows: Rust + C# + Flutter
