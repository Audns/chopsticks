# Chopsticks

A keyboard-driven mouse cursor for Wayland. Navigate your screen with letter keys and click without touching the mouse.

## What It Does

Chopsticks displays a full-screen overlay grid over your display. You press letter keys to narrow down to a precise screen location, and it teleports the cursor there and clicks. Built for tiling window managers and keyboard-centric workflows on wlroots-based compositors.

## How It Works

The screen is divided into a **26×26 grid** labeled with letters `a` through `z`. Each cell is identified by a two-letter coordinate (first = row, second = column).

### Three-Stage Selection

| Stage | Keys | What Happens |
|-------|------|-------------|
| **1** | Press `a-z` | Selects a **row**. The full grid dims; only that row shows column labels `a-z`. |
| **2** | Press `a-z` | Selects a **cell** within the row. The cell zooms into an 8-subcell precision grid. |
| **3** | Press `y,u,i,o,h,j,k,l,n,m,,,.` | Selects a **sub-cell**. The overlay closes and a left-click is emitted at that exact spot. |
|       | Press `Space`                    | Clicks the **center** of the selected cell directly, skipping the 12-subcell grid. |

**Escape** exits at any time. Invalid keys reset to stage 1.

### Precision Grid Layout

```text
y  u  i  o
h  j  k  l
n  m  ,  .
    ·        ← Space = center of cell
```

## Installation

### Dependencies

- A **Wayland** compositor supporting:
  - `zwlr_layer_shell_v1`
  - `zwlr_virtual_pointer_manager_v1`
- [JetBrainsMono Nerd Font](https://github.com/ryanoasis/nerd-fonts) at `/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Bold.ttf`
- Rust toolchain

### Build

```bash
cargo build --release
```

### Run

```bash
./target/release/chopsticks
```

> **Note:** Only one instance can run at a time. A second launch will exit immediately.

## Configuration

Create `~/.config/chopsticks/config.toml`:

```toml
# Background opacity for each stage (0-255, 0 = transparent)
idle_bg_opacity = 0
row_bg_opacity = 0
cell_bg_opacity = 0

# Grid and text appearance
grid_color = "888888"
text_color = "FFFFFF"
grid_opacity = 255
font_size_divisor = 2

# Fractional scaling support
scale_ratio = 1.0       # e.g. 1.5 for 150% scaling
window_width = 2560     # Physical monitor width
window_height = 1440    # Physical monitor height
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `idle_bg_opacity` | `u8` | `0` | Background opacity in idle state |
| `row_bg_opacity` | `u8` | `0` | Background opacity when row is selected |
| `cell_bg_opacity` | `u8` | `0` | Background opacity when cell is selected |
| `grid_color` | string | `"888888"` | Grid line color (hex) |
| `text_color` | string | `"FFFFFF"` | Label text color (hex) |
| `grid_opacity` | `u8` | `255` | Grid line opacity |
| `font_size_divisor` | `u32` | `2` | Font size divisor relative to cell size |
| `scale_ratio` | `f64` | `1.0` | HiDPI scale factor (e.g. `1.5`) |
| `window_width` | `u32` (optional) | auto | Physical monitor width |
| `window_height` | `u32` (optional) | auto | Physical monitor height |

All fields are optional. If `window_width`/`window_height` are omitted, the program auto-detects from the compositor.

## How It Works Internally

- **Overlay**: Creates a fullscreen `zwlr_layer_shell_v1` overlay with exclusive keyboard focus
- **Rendering**: Draws directly into a Wayland shared memory buffer using software rasterization
- **Text**: Uses `swash` for font rasterization with alpha blending
- **Clicking**: Emits absolute pointer motion and button press/release via `zwlr_virtual_pointer_manager_v1`
- **Scaling**: When `scale_ratio` is set, the grid renders at logical size (`physical / scale`) and click coordinates are multiplied back up to land correctly on scaled displays
