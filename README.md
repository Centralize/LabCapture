# LabCapture

LabCapture takes screenshots and records videos of your screen. You can grab
a whole screen, one window, or an area you drag out. You can mark up a
screenshot with arrows, boxes and blur before you share it, and record your
screen to an MP4 video (with sound) or an animated GIF.

- [Getting started](#getting-started)
- [Taking a screenshot](#taking-a-screenshot)
- [Marking up a screenshot](#marking-up-a-screenshot)
- [Recording a video](#recording-a-video)
- [The library](#the-library)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Settings](#settings)
- [Where your files go](#where-your-files-go)
- [Tips and troubleshooting](#tips-and-troubleshooting)
- [Building LabCapture yourself](#building-labcapture-yourself)
- [License](#license)

---

## Getting started

Start **LabCapture** (`labcapture.exe` on Windows). The window has three pages
along the top:

| Page | What it's for |
|---|---|
| **Screenshot** | Take screenshots and mark them up |
| **Record** | Record your screen to a video |
| **Library** | Browse everything you've captured |

The **gear button** at the top right opens **Settings**.

LabCapture remembers your choices (folders, formats, options) between runs.

> **Works best on:** Windows 10 (version 2004 or later) and Windows 11. That is
> where LabCapture was built and tested. On other systems, screenshots and
> silent videos should work, but recording sound needs Windows.

---

## Taking a screenshot

1. Open the **Screenshot** page.
2. Choose **what** to capture:
   - **Screen**: a whole screen. If you have several screens, pick one from
     the list, or pick **All screens together**.
   - **Window**: one window. Pick it from the list, which shows the windows
     that are open right now. Minimized windows can't be captured.
   - **Area**: part of the screen. You drag out the area after you press the
     button (see below).
3. Pick a **Delay** if you need a moment to get things ready: *None*, *3 s*,
   *5 s* or *10 s*. A countdown appears on screen. Press **Esc** to cancel it.
4. Press **Take screenshot**.

By default, a new screenshot is:

- **copied to the clipboard**, ready to paste into a chat, email or document,
- **saved** in your screenshots folder (see [Where your files go](#where-your-files-go)),
- **shown** in LabCapture, which comes to the front so you can mark it up.

You can turn each of these off under **After capture**.

You can also start a screenshot from the big area in the middle of the page:
**Whole screen**, **Drag an area**, or **Record a video** (which takes you to
the Record page).

### Choosing an area

When you capture an **Area**, the screen freezes and darkens:

- **Drag** with the mouse to choose the area. The size in pixels is shown as you drag.
- Let go to take the screenshot.
- Press **Esc** or **right-click** to cancel.

The area is chosen on the screen your mouse pointer is on.

### LabCapture stays out of its own pictures

While it captures, LabCapture keeps its own window out of the picture, so you
never need to move it out of the way. You can turn this off in
[Settings](#settings).

---

## Marking up a screenshot

After you take a screenshot (or open one from the Library), it appears in the
editor. The toolbar above the picture has:

| Tool | Use it to |
|---|---|
| **Pen** | Draw freely |
| **Highlighter** | Highlight text with a see-through stroke |
| **Box** | Draw a rectangle around something |
| **Arrow** | Point at something |
| **Blur** | Hide private details (it turns the area into large squares) |

- Pick a **colour** from the eight dots, and a line thickness with **Size**.
- Drag on the picture to draw with the chosen tool.
- **Undo** (Ctrl+Z), **Redo** (Ctrl+Y), or **remove all markup** (the bin button).

The markup goes into the saved picture exactly as you see it.

The bar under the picture shows the file name, the picture's size, and whether
your latest changes are saved:

| Button | What it does |
|---|---|
| **Copy** (Ctrl+C) | Copies the picture, with markup, to the clipboard |
| **Save** (Ctrl+S) | Saves the picture with your markup. If it was already saved, the same file is updated. |
| **Save as…** | Saves a copy wherever you like, as PNG or JPEG |
| **Show in folder** | Opens the folder with the file selected |
| **×** | Closes the screenshot. Markup you haven't saved is lost; the screenshot itself was already saved if *Save to your screenshots folder* is on. |

---

## Recording a video

1. Open the **Record** page.
2. Choose **what** to record:
   - **Screen**: a whole screen (choose which one from the list).
   - **Window**: one window. If you move it while recording, the video
     follows it.
   - **Area**: part of a screen. Press **Choose area** and drag over your
     screen. LabCapture remembers the area for next time.
3. Check the **Preview** on the right. It shows what will be recorded and
   refreshes about once a second. You can switch it off with **Live preview**.
4. Pick your options (below), then press **Start recording**.
   With **Count down 3 seconds first** on, a countdown appears first. Press
   **Esc** to cancel it.

While recording:

- The preview area shows the time recorded, the video size and the file size.
- **Pause** and **Resume** leave the paused part out of the video, sound included.
- **Stop** finishes the video. A red timer also appears at the top of the
  window; click it to stop.
- **Alt+Shift+R** starts or stops a recording from any app.

When the video is ready, a card appears with **Play**, **Show in folder** and
the video's length and size. It also shows up in the Library.

If you close LabCapture while recording, it finishes the video first, so the
file isn't left broken.

### Recording options

| Option | Choices | Notes |
|---|---|---|
| **Format** | MP4 video, Animated GIF | MP4 is best for longer videos and can include sound. GIF plays anywhere, but has no sound and is kept under 1280 pixels wide. |
| **Frames per second** | 15, 24, 30, 60 | More frames per second gives smoother motion but bigger files. 30 suits most uses. |
| **Quality** | Small file, Balanced, Sharp | A trade-off between file size and sharpness. |
| **Size** | Full size, Half size | Half size makes the video half as wide and half as tall. That means much smaller files and smoother recording on big screens. |
| **Computer sound** | on/off | Records what plays through your speakers or headphones. MP4 only. On by default. |
| **Microphone** | on/off | Records your default microphone, mixed with the computer sound. MP4 only. Off by default. |
| **Show the mouse pointer** | on/off | Draws the pointer into the video. |
| **Highlight clicks** | on/off | Shows a yellow circle while the left mouse button is held, and a blue one for the right button. |
| **Count down 3 seconds first** | on/off | Gives you a moment before recording starts. |

The line under the preview shows what the file will be, for example
*MP4 · 1920 × 1080 · 30 fps*.

---

## The library

The **Library** page shows your screenshots and recordings, newest first, with
a small preview of each.

- Filter with **All**, **Screenshots** or **Videos**.
- **Click a screenshot** to open it in the editor and mark it up.
- **Click a video or GIF** to play it in your usual video player.
- **Right-click** any item for **Open**, **Mark up in LabCapture** (pictures),
  **Show in folder** and **Delete…**. Deleting asks first, and it removes the
  file for good.
- **Pictures folder** and **Videos folder** open those folders.

The Screenshot page also shows your latest screenshots under **Recent
screenshots** when no screenshot is open.

---

## Keyboard shortcuts

These work **anywhere**, even when LabCapture is in the background or
minimized:

| Shortcut | Does |
|---|---|
| **Alt+Shift+S** | Screenshot of the whole screen (the screen chosen on the Screenshot page) |
| **Alt+Shift+A** | Screenshot of an area (on the screen your pointer is on) |
| **Alt+Shift+W** | Screenshot of the window in front |
| **Alt+Shift+R** | Start or stop recording, using the Record page's choices |

The **Delay** from the Screenshot page applies to the shortcuts too.

Inside LabCapture:

| Shortcut | Does |
|---|---|
| **Esc** | Cancels a countdown, or the area picker |
| **Ctrl+Z / Ctrl+Y** | Undo / redo markup |
| **Ctrl+S** | Save the screenshot |
| **Ctrl+C** | Copy the screenshot |

If another app already uses one of these shortcuts, LabCapture says so on the
Screenshot page and in Settings, and the other shortcuts still work. You can
turn the anywhere-shortcuts off in Settings.

---

## Settings

Open Settings with the **gear button** at the top right.

- **Where captures go:** the folders for screenshots and for videos.
  **Change…** picks another folder; **Open** shows it.
- **Screenshots → File type:** **PNG** (sharpest, best for text and screens)
  or **JPG** (smaller files, fine for photos).
- **Privacy → Keep LabCapture out of captures:** when on, LabCapture's own
  window never appears in your screenshots or videos. On Windows the window
  stays where it is but is invisible to captures. While this is on and you're
  on the Record page or recording, other screen-sharing or recording apps
  (for example a video call) won't see the LabCapture window either.
  On other systems, LabCapture hides for a moment instead.
- **Shortcuts:** turn the anywhere-shortcuts on or off, and see the list.
- **About:** the version of LabCapture.

---

## Where your files go

| What | Where (unless you change it in Settings) | Named like |
|---|---|---|
| Screenshots | Your **Pictures** folder, in **LabCapture** | `Screenshot 2026-11-18 at 14.05.32.png` |
| Recordings | Your **Videos** folder, in **LabCapture** | `Recording 2026-11-18 at 14.07.10.mp4` |

If your Pictures or Videos folder is kept in OneDrive, the LabCapture folders
are inside it too. LabCapture makes the folders the first time it saves
something.

LabCapture also keeps small preview pictures of your recordings, for the
Library, in your user account's local cache folder (`LabCapture\thumbs`).
They are not your videos and can be deleted safely.

---

## Tips and troubleshooting

**The recording warns that frames are being skipped.**
Your computer can't keep up at this size. Choose **Half size**, fewer
**frames per second**, or record an **Area** instead of the whole screen.
Very large screens (such as 3440 × 1440) with a lot of movement are the
hardest case.

**My video has no sound.**
- Sound is recorded in **MP4** only, and only on Windows.
- Check that **Computer sound** (or **Microphone**) is on before you start.
- Computer sound comes from your **default playback device** in Windows' sound
  settings. If you listen through a different device, LabCapture won't hear it.
- The microphone is your **default recording device** in Windows' sound settings.
- When nothing is playing, the video is simply silent; that's expected.
- If a sound source can't start, the "Recording saved" message says which one.

**A window comes out black or empty.**
Some apps (often games and video players) don't allow window capture. Use
**Screen** or **Area** instead.

**A full-screen game isn't recorded.**
Games running in exclusive full-screen mode may not be captured. Switch the
game to *borderless* or *windowed* mode.

**A GIF is very large.**
GIFs store only what changes from frame to frame, so a still screen with a
small moving part gives a small file. A lot of movement (videos, scrolling)
makes GIFs big. Use MP4 for those.

**The shortcuts don't work.**
Check Settings → Shortcuts is on. If another app already uses the same keys,
LabCapture tells you which; close that app or change its shortcut.

**My screenshots are on the clipboard but I can't find the file.**
Check that **Save to your screenshots folder** is on (Screenshot page, *After
capture*), or open the Library and use **Pictures folder**.

---

## Building LabCapture yourself

LabCapture is written in Rust. To build it:

1. Install Rust from <https://rustup.rs>. On Windows you also need the
   "Desktop development with C++" tools from Visual Studio, which the Rust
   installer offers to set up.
2. In this folder, run:

   ```
   cargo build --release
   ```

3. The program is then at `target/release/labcapture.exe` (Windows) or
   `target/release/labcapture` (other systems).

### Command-line option

`labcapture --screenshot <file.png>` opens LabCapture quietly in the
background, without taking focus. It saves a picture of LabCapture's own
window to that file and closes. It's used for automatic checks; you don't need
it for normal use.

---

## License

LabCapture is released under the MIT License. See [LICENSE.md](LICENSE.md).

Copyright (c) 2026 Mikkel Kaas.
