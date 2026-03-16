# Plan: ASUS ROG Armoury Crate — Full Feature Parity + Premium UI on Linux

## Context

The `asusctl` project provides Linux support for ASUS ROG laptop features. The current GUI (`rog-control-center`) uses Slint and works on all distros, but has a basic functional look. **Target hardware: ASUS ROG Strix G15 G513IC** (matched as `G513I` in aura_support.ron — 4-zone RGB + 6 lightbar zones, modes: Static, Breathe, RainbowCycle, RainbowWave, Pulse).

**Goals:**
1. **Redesign the UI** to match or exceed ASUS Armoury Crate's premium dark gaming aesthetic
2. **Add Fn key / hotkey daemon** — handle ROG button, Fn+F5 (profile), Fn+F4 (Aura), macro keys
3. **Add AI Noise Cancellation** via PipeWire + RNNoise
4. **Add Aura Creator** — timeline-based RGB lighting editor
5. **Add Scenario Profiles** — auto-switching based on running apps
6. Works on **all Linux distros** (Fedora KDE, Pop!_OS, Ubuntu, Arch, etc.)

**Critical gap:** asusctl currently has **ZERO hotkey handling** — no Fn key detection, no ROG button, no evdev/input integration. The MANUAL.md tells users to bind keys externally. This is the biggest missing piece vs Armoury Crate.

---

## Phase 1: Premium Gaming UI Overhaul (Weeks 1-4)

The current UI is 900x500, Noto Sans, basic Slint palette colors, minimal animations. Armoury Crate uses a dark carbon/steel theme with glowing RGB accents, custom-shaped widgets, and smooth animations.

### 1.1 Custom Theme System
**Files**: New `ui/theme.slint`, modify all existing `.slint` files

Create a `ROGTheme` global replacing raw `Palette` usage:
- **Background**: Deep charcoal gradient (`#0d0d0d` → `#1a1a1a`), subtle carbon-fiber pattern via tiled image
- **Surface**: Semi-transparent dark panels (`rgba(30, 30, 30, 0.85)`) with 1px glowing borders
- **Accent**: ROG red (`#FF0032`) as primary, with configurable user accent (synced with Aura color)
- **Glow effects**: Drop shadows with accent color (`blur: 12px`, `color: rgba(255,0,50,0.3)`)
- **Text**: White (`#FFFFFF`) primary, `#B0B0B0` secondary, accent for highlights
- **Fonts**: Keep Noto Sans but add weight hierarchy: 700 for headers, 500 for labels, 400 for body

### 1.2 Redesign Sidebar Navigation
**File**: [ui/widgets/sidebar.slint](rog-control-center/ui/widgets/sidebar.slint)

- Width: 200px (up from 160px)
- ROG logo at top (SVG/PNG asset) with subtle glow animation
- Menu items: icon + text, with glowing left-edge indicator on active item
- Hover: accent-colored underline slides in (animated 200ms ease-out)
- Active: full left border glow + slight background highlight
- Smooth page transitions (fade + slide, 250ms)

### 1.3 Redesign Common Widgets
**File**: [ui/widgets/common.slint](rog-control-center/ui/widgets/common.slint)

- **RogSlider** (replace SystemSlider): Custom-drawn track with gradient fill (dark → accent), glowing thumb, value tooltip on hover
- **RogToggle** (replace SystemToggle): Pill-shaped toggle with smooth slide animation (200ms), glow when active
- **RogDropdown** (replace SystemDropdown): Dark dropdown with accent border on focus, smooth expand animation
- **RogButton**: Rounded corners (8px), gradient background, glow on hover, press animation (scale 0.97)
- **RogCard**: Container with subtle border glow, slight elevation shadow, rounded corners (12px)

### 1.4 Redesign Fan Curve Graph
**File**: [ui/widgets/graph.slint](rog-control-center/ui/widgets/graph.slint)

- Dark grid with thin accent-colored lines (`rgba(255,0,50,0.15)`)
- Curve line: 3px with glow effect (accent color shadow)
- Draggable nodes: 14px circles with pulsing glow on hover, smooth drag (no snapping)
- Gradient fill under curve (accent color → transparent)
- Animated transitions when switching profiles

### 1.5 Redesign Color Picker
**File**: [ui/widgets/colour_picker.slint](rog-control-center/ui/widgets/colour_picker.slint)

- 2D saturation/value canvas (click/drag) + hue ring or strip
- Live color preview orb with glow matching selected color
- Hex input with accent border
- Smooth real-time updates

### 1.6 Redesign All Pages

**System page** ([ui/pages/system.slint](rog-control-center/ui/pages/system.slint)):
- Grouped in RogCard sections with headers
- PPT sliders with gradient fills showing power level
- Profile selector: large buttons with icons (Balanced/Performance/Quiet) — active one glows

**Aura page** ([ui/pages/aura.slint](rog-control-center/ui/pages/aura.slint)):
- Visual keyboard layout preview showing current RGB zones (G513IC: 4 keyboard zones + 6 lightbar zones)
- Mode selection as visual cards (icon + name + preview animation)
- Color picker prominent and large

**AniMe page** ([ui/pages/anime.slint](rog-control-center/ui/pages/anime.slint)):
- Matrix preview showing current animation
- Animation previews as thumbnail cards

**Fan Curves page** ([ui/pages/fans.slint](rog-control-center/ui/pages/fans.slint)):
- Full-width graph with premium styling from 1.4
- Profile tabs as styled segment control (not basic TabWidget)

### 1.7 Assets
- ROG logo SVG for sidebar
- Icon set for sidebar items (system, keyboard, hotkeys, display, fan, audio, settings, info)
- Optional: subtle background texture/pattern image

### 1.8 Window Resize
- Increase default size to ~1100x650 for better content breathing room
- Make window resizable with min-size constraints

---

## Phase 2: Fn Key / Hotkey Daemon (Weeks 5-8)

asusctl has **no input event handling at all**. The `MANUAL.md` says users should bind `fn+f5` to `asusctl profile -n` externally. This phase adds a proper hotkey daemon.

### 2.1 Create `rog-hotkeys` crate
- **Input backend**: `evdev` crate (Rust bindings for Linux evdev)
- Monitor `/dev/input/event*` devices for ASUS-specific key events
- Detect input devices via udev (match ASUS vendor ID `0x0B05` or `asus::kbd_backlight`)
- Run as part of `asusd` (system daemon, needs `/dev/input` access)

### 2.2 Key events to handle (G513IC)

| Key Combo | Evdev Code | Default Action |
|-----------|-----------|----------------|
| **Fn+F5** | `KEY_PROG3` / platform profile | Cycle thermal profiles (Balanced→Performance→Quiet) |
| **Fn+F4** | `KEY_KBDILLUMTOGGLE` | Cycle Aura modes |
| **ROG button** | `KEY_PROG1` / `KEY_LAUNCH1` | Open rog-control-center (customizable) |
| **Fn+↑/↓** | Keyboard brightness keys | Cycle Aura brightness (Off→Low→Med→High) |
| **Fn+F12** | Airplane mode | Toggle via rfkill |
| **Fn+F7/F8** | Screen brightness | Passthrough to DE (already handled) |
| **Fn+F1-F3** | Volume/mute | Passthrough to DE (already handled) |

**Implementation:**
- `evdev::Device::open()` on ASUS keyboard input device
- Async event loop (tokio) reading `InputEvent`s
- Map key codes → D-Bus calls to `asusd` (profile switch, Aura cycle, etc.)
- Configurable key→action mappings in `/etc/asusd/hotkeys.ron`
- Keys handled by the DE (volume, brightness) are passed through — no conflict

### 2.3 ROG Button customization
- Default: launch `rog-control-center`
- Configurable via D-Bus: `xyz.ljones.Hotkeys.set_rog_button_action(action)`
- Actions: open GUI, cycle profile, toggle noise cancel, run custom command
- Config: `/etc/asusd/hotkeys.ron`

### 2.4 Macro key support
- Record key sequences (key down/up events with timing)
- Store macros in `~/.config/rog/macros.ron`
- Replay via virtual input device (`uinput` crate)
- D-Bus interface: `xyz.ljones.Macros` — `record_start`, `record_stop`, `play`, `list`, `delete`
- Runs in `asusd-user` (user-level, uses uinput for playback)

### 2.5 Slint UI page
- New `ui/pages/hotkeys.slint`
- Key binding table: shows Fn key → action mappings, each row editable
- ROG button action selector (dropdown with custom command input)
- Macro list with record/play/delete controls
- Visual macro recorder: shows captured keys in real-time during recording
- Premium styling matching Phase 1 theme

### 2.6 udev rule update
**File**: [data/asusd.rules](data/asusd.rules)
- Add rule to tag ASUS keyboard input devices for the hotkey daemon
- Grant `asusd` group access to relevant `/dev/input` devices

### 2.7 CLI
- `asusctl hotkey --list` — show current bindings
- `asusctl hotkey --set fn-f5 "profile -n"` — rebind a key
- `asusctl hotkey --rog-button "launch rog-control-center"` — set ROG button action
- `asusctl macro --record <name>` / `--play <name>` / `--list` / `--delete <name>`

---

## Phase 3: AI Noise Cancellation (Weeks 9-13)

### 3.1 Create `rog-noise-cancel` crate
- **Engine**: PipeWire filter chain via `pipewire-rs`
- **Model**: RNNoise (BSD-licensed, real-time noise suppression) via `rnnoise-c` bindings
- Two filter nodes:
  - **Mic**: physical mic → RNNoise → virtual source
  - **Speaker**: app output → RNNoise → physical sink
- Registers as virtual source/sink in PipeWire graph
- Fallback: PulseAudio `module-echo-cancel` if no PipeWire

### 3.2 D-Bus interface in `asusd-user`
- Interface: `xyz.ljones.NoiseCancel` (session bus)
- Properties: `mic_enabled: bool`, `speaker_enabled: bool`, `suppression_level: i32` (0-100)
- D-Bus proxy: new [rog-dbus/src/zbus_noise_cancel.rs](rog-dbus/src/zbus_noise_cancel.rs)

### 3.3 Slint UI page
- New `ui/pages/noise_cancel.slint`
- Two large toggle cards: Mic and Speaker (with icons, glow when active)
- Suppression strength slider with visual feedback
- Real-time audio level meter (horizontal bars, before/after)
- PipeWire status indicator
- Add to sidebar in [rog-control-center/src/ui/mod.rs](rog-control-center/src/ui/mod.rs)

### 3.4 CLI: `asusctl noise-cancel --mic on|off --speaker on|off --level 75`

---

## Phase 4: Aura Creator (Weeks 14-19)

### 4.1 Data model in `rog-aura`
**File**: new [rog-aura/src/creator.rs](rog-aura/src/creator.rs)
- `AuraKeyframe`: zone, color, effect, duration, easing
- `AuraLayer`: ordered keyframes (compositable)
- `AuraTimeline`: layers + playback settings (loop, speed)
- `AuraCreatorProject`: RON-serializable
- Extends existing types without breaking `AuraModeNum`

### 4.2 Daemon support
**File**: new [asusd/src/ctrl_aura_creator.rs](asusd/src/ctrl_aura_creator.rs)
- New D-Bus methods on `xyz.ljones.Aura`:
  - `set_aura_timeline(AuraTimeline)` — playback as USB packets
  - `preview_keyframe(AuraKeyframe)` — live preview
  - `list/save/delete_timeline`
- Timer-driven playback converting keyframes → `direct_addressing_raw`
- Storage: `/etc/asusd/aura_timelines/*.ron`

### 4.3 Slint UI page
- New `ui/pages/aura_creator.slint`
- **Timeline**: horizontal bar with draggable keyframe markers, scrubber
- **Layers panel**: add/remove/reorder
- **Keyframe editor**: color picker + zone selector + duration + easing
- **Preview controls**: play/pause/stop
- **Save/load**: project list
- Premium styling: glowing keyframe markers, gradient timeline track

### 4.4 CLI: `asusctl aura creator --load/--save/--play/--list`

---

## Phase 5: Scenario Profiles (Weeks 20-24)

### 5.1 Create `rog-scenarios` crate
- `ScenarioRule`: condition (process name regex, window class) → action (profile, fan curve, Aura mode, GPU mode, PPT)
- `ScenarioMonitor`: polls `/proc` via `procfs` (5s interval)
- Config: `~/.config/rog/scenarios.ron` via `StdConfig`

### 5.2 Integration in `asusd-user`
- D-Bus interface: `xyz.ljones.Scenarios` (session bus)
- Methods: `add_rule`, `remove_rule`, `list_rules`, `set_enabled`
- Signal: `active_scenario_changed`
- On match → calls `asusd` to apply profile; on exit → reverts

### 5.3 Slint UI page
- New `ui/pages/scenarios.slint`
- Rule list as styled cards with enable/disable toggles
- Per-rule editor: process input + profile dropdowns
- "Add from running apps" button
- Active scenario indicator with glow

### 5.4 CLI: `asusctl scenario --list/--add/--remove`

---

## Phase 6: System Tray + Polish (Weeks 25-27)

### 6.1 Enhanced tray
**File**: [rog-control-center/src/tray.rs](rog-control-center/src/tray.rs)
- Quick submenu: profile switcher, charge limit, Aura brightness, noise cancel toggle
- Dynamic icon based on profile/GPU mode
- Works on KDE Plasma (native SNI), GNOME (appindicator), XFCE

### 6.2 Feature detection
- Extend [rog-control-center/src/ui/mod.rs:97-108](rog-control-center/src/ui/mod.rs#L97-L108)
- Add sidebar entries for: Hotkeys, Noise Cancel, Aura Creator, Scenarios
- Conditionally shown by D-Bus interface availability

### 6.3 Testing across distros
- Fedora KDE Plasma, Pop!_OS, Ubuntu GNOME, Arch
- Verify PipeWire, tray, hotkeys, and UI rendering on each

---

## New Crate Structure

Add to [Cargo.toml](Cargo.toml) workspace members:
```
"rog-hotkeys"        # Fn key / hotkey daemon + macro engine
"rog-noise-cancel"   # PipeWire + RNNoise engine
"rog-scenarios"      # App-aware profile switching
```

New workspace dependencies:
```toml
evdev = "0.12"       # Linux input event handling
uinput = "0.1"       # Virtual input device for macro playback
pipewire = "0.8"     # PipeWire Rust bindings
procfs = "0.16"      # Process monitoring
```

---

## Key Files to Modify

| File | Change |
|------|--------|
| [Cargo.toml](Cargo.toml) | Add workspace members + deps |
| [rog-control-center/Cargo.toml](rog-control-center/Cargo.toml) | Add new crate deps |
| [ui/widgets/common.slint](rog-control-center/ui/widgets/common.slint) | Redesign all widgets with gaming aesthetic |
| [ui/widgets/sidebar.slint](rog-control-center/ui/widgets/sidebar.slint) | Premium sidebar with icons + glow |
| [ui/widgets/graph.slint](rog-control-center/ui/widgets/graph.slint) | Glowing curve, gradient fill, premium nodes |
| [ui/widgets/colour_picker.slint](rog-control-center/ui/widgets/colour_picker.slint) | 2D picker + glow preview |
| [ui/main_window.slint](rog-control-center/ui/main_window.slint) | New size, theme, page transitions |
| [ui/pages/system.slint](rog-control-center/ui/pages/system.slint) | Card-based layout, profile selector cards |
| [ui/pages/aura.slint](rog-control-center/ui/pages/aura.slint) | Visual keyboard zone preview (G513IC zones), mode cards |
| [ui/pages/fans.slint](rog-control-center/ui/pages/fans.slint) | Styled segment tabs, premium graph |
| New: `ui/theme.slint` | ROGTheme global (colors, spacing, fonts) |
| New: `ui/pages/hotkeys.slint` | Fn key bindings + macro editor |
| New: `ui/pages/noise_cancel.slint` | Noise cancel page |
| New: `ui/pages/aura_creator.slint` | Timeline editor page |
| New: `ui/pages/scenarios.slint` | Scenario profiles page |
| [rog-control-center/src/ui/mod.rs](rog-control-center/src/ui/mod.rs) | Add 4 new page setups + sidebar entries |
| [rog-control-center/src/tray.rs](rog-control-center/src/tray.rs) | Enhanced submenu |
| [rog-aura/src/lib.rs](rog-aura/src/lib.rs) | AuraTimeline/Keyframe/Layer types |
| [rog-dbus/src/](rog-dbus/src/) | New: zbus_hotkeys.rs, zbus_noise_cancel.rs, zbus_scenarios.rs, zbus_macros.rs; extend zbus_aura.rs |
| [asusd/src/daemon.rs](asusd/src/daemon.rs) | Register hotkey listener + new D-Bus interfaces |
| [asusd-user/](asusd-user/) | Macro playback, noise cancel manager, scenario monitor |
| [asusctl/](asusctl/) | New subcommands: hotkey, macro, noise-cancel, scenario, aura creator |
| [data/asusd.rules](data/asusd.rules) | Add input device rules for hotkey daemon |
| [Makefile](Makefile) | Asset install, new pages in translation |

## Reuse Existing Code

- `set_ui_callbacks!` macro ([rog-control-center/src/ui/mod.rs:25-65](rog-control-center/src/ui/mod.rs#L25-L65)) — wire new D-Bus properties to new pages
- `show_toast` — feedback for all actions
- `list_iface_blocking` ([rog-dbus/src/lib.rs](rog-dbus/src/lib.rs)) — feature detection for new interfaces
- `StdConfig` trait ([config-traits/](config-traits/)) — RON config for hotkeys, scenarios, macros
- `direct_addressing_raw` in [rog-dbus/src/zbus_aura.rs](rog-dbus/src/zbus_aura.rs) — Aura Creator live preview
- `mocking` feature flag — extend for hardware-less testing
- udev device detection pattern in [rog-platform/](rog-platform/) — reuse for input device discovery

---

## Verification

1. `cargo build --workspace` — all crates compile
2. `cargo build --features mocking` — UI runs without hardware
3. **Visual**: Compare screenshots against Armoury Crate reference
4. **Hotkeys on G513IC**: Verify Fn+F5 cycles profiles, Fn+F4 cycles Aura, ROG button opens GUI
5. **Macro**: Record a key sequence, replay it, verify correct timing
6. **Noise cancel**: `pw-cli` test with recorded audio, measure CPU < 5%
7. **Aura Creator**: Verify timeline playback sends correct USB packets
8. **Scenarios**: Process detection + auto-switch with Steam/games
9. **Tray**: Works on KDE Plasma, GNOME+appindicator, XFCE
10. **Distros**: Fedora 41+ KDE, Pop!_OS, Ubuntu 24.04, Arch

---

## Risks

| Risk | Mitigation |
|------|-----------|
| Slint custom widget complexity for gaming look | Slint supports Path, shadows, gradients, animations — sufficient |
| evdev key codes vary across ASUS models | Auto-detect via udev; G513IC codes known; make mappings configurable |
| uinput permissions for macro playback | Document uinput group membership requirement |
| `pipewire-rs` version compatibility | Target Fedora's bundled PipeWire; test on multiple distros |
| RNNoise vs ASUS proprietary quality | Best OSS option; future: DeepFilterNet for higher quality |
| Fan curve canvas drag precision | Unit test coordinate mapping independently |
| `ksni` tray on GNOME (no native SNI) | Document appindicator extension requirement |
| Armoury Crate UI changes in future | Design inspired-by, not pixel-perfect clone |
| Fn key conflicts with DE | Only handle ASUS-specific keys; passthrough standard media/brightness |
