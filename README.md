# Dimenshot

**Dimenshot** is a 3D arcade shooter built in Rust with the [Bevy](https://bevyengine.org/) game engine. A giant, continuously rotating hollow sphere suspended in space is constructed from **860 individual curved bricks**. Your objective is to dismantle the entire shell brick by brick—blasting holes through the near side to shoot across the hollow interior and shatter the inner wall on the far side.

![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?logo=rust)
![Bevy](https://img.shields.io/badge/Bevy-0.20.1-blue)
![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)

---

## Key Features

- **Procedural Hollow Brick Globe**
  - The sphere is subdivided into **26 latitude bands** arranged in a staggered running-bond masonry pattern (~860 bricks total), keeping brick proportions nearly square from the equator to the poles.
  - Every brick is a custom-generated 3D curved wedge with a chamfered outer bevel, radial side walls, and a flush inner surface that prevents light leaks while intact.
  - Once outer bricks are knocked away, shots pass straight through the openings into the hollow interior—either ricocheting off the glowing central core or destroying bricks on the far inner wall from the inside out.

- **$\mathcal{O}(1)$ Spherical Spatial Lookup & Raycasting**
  - Neither bullet collision detection nor cursor raycasting iterates over the 860 brick entities.
  - Points in globe-local space are mapped directly to `(band, index)` via spherical coordinates (`asin` / `atan2`) in constant time.

- **GPU-Friendly Instanced Meshes**
  - Because all bricks within a latitude band are congruent up to a Y-axis rotation, only **26 unique meshes** are uploaded to the GPU and shared across all 860 bricks.

- **Live-Tunable 3D Rotation**
  - Adjust the globe's rotation speed on the fly from `0.0x` (frozen) up to `6.0x`, invert the spin direction, or cycle between three rotation axis modes:
    - **Tilt** — Fixed diagonal axis.
    - **Tumble** — Continuously precessing multi-axis rotation.
    - **Roll** — Horizontal rolling axis.

- **Sharp, High-Contrast 3D Graphics**
  - **HDR Pipeline** with `Tonemapping::TonyMcMapface` and subtle `Bloom::NATURAL`.
  - **4x MSAA** paired with **Contrast Adaptive Sharpening (CAS)** for crisp geometric edges without aliasing.
  - **Clearcoat PBR Materials** and a **4096×4096 Shadow Map** so bricks cast sharp shadows across the hollow interior.
  - **Reactive Physics FX**: Detached bricks inherit impact momentum and tumble away under gravity alongside flying masonry chips, muzzle flashes, impact sparks, and trauma-based camera shake.

- **Adaptive Vertical Arcade Window**
  - Automatically queries the primary monitor at startup and sizes the window to **75% (3/4) of the screen height** in a vertical portrait aspect ratio (`0.62`), centered on the display.
  - Keeps the window hidden for the first few frames until the swapchain is ready, eliminating the startup white flash.

---

## Controls

| Input | Action |
| :--- | :--- |
| **Mouse Move** | Aim turret & highlight target brick under reticle |
| **Left Click / `Space` (Hold)** | Fire continuous tracer rounds |
| **Mouse Wheel** or **`+` / `-`** or **`[` / `]`** | Increase / decrease globe rotation speed (`0.0x` – `6.0x`) |
| **`0`** | Pause / resume globe rotation |
| **`1` / `2` / `3`** | Rotation speed presets (`1.0x`, `2.0x`, `3.5x`) |
| **`T`** | Reverse rotation direction |
| **`M`** | Cycle rotation axis mode (`Tilt` / `Tumble` / `Roll`) |
| **`R`** | Reset round and rebuild the globe |
| **`F`** | Toggle live FPS counter |

---

## Getting Started

### Prerequisites

- **Rust Toolchain**: `rustc >= 1.97.1` (Rust 2024 edition)
- A GPU supporting **Vulkan**, **DirectX 12**, or **Metal**

Update your Rust toolchain if needed:

```bash
rustup update stable
```

### Build & Run

For maximum performance (full LTO and `opt-level = 3`):

```bash
cargo run --release
```

For fast iterative builds during development (dependencies are still compiled with `opt-level = 3` in `Cargo.toml` so gameplay remains smooth):

```bash
cargo run
```

---

## Project Structure

```text
Dimenshot/
├── Cargo.toml          # Dependencies (Bevy 0.20.1) & build optimization profiles
└── src/
    ├── main.rs         # App entry point, renderer/lighting resources, plugin registration
    ├── globe.rs        # Procedural curved brick mesh generator, hollow sphere layout & O(1) lookup
    ├── game.rs         # Turret aiming, bullet physics, brick detachment, spin & axis controls
    ├── fx.rs           # Tumbling brick debris, masonry chips, sparks, starfield & camera rig
    ├── hud.rs          # Responsive UI overlay (brick counter, timer, accuracy, spin bar)
    ├── window_fit.rs   # Automatic 3/4 monitor-height vertical window sizing & centering
    ├── rng.rs          # Lightweight, zero-dependency xorshift64* PRNG
    └── dev.rs          # Optional demo autopilot & automated screenshot capture
```

---

## Automated Demo & Screenshot Mode

Dimenshot includes environment-variable hooks in [`src/dev.rs`](src/dev.rs) for automated testing and showcasing:

- `DIMENSHOT_DEMO=1` — Enables an autonomous virtual cursor that sweeps across the globe and fires continuously.
- `DIMENSHOT_SHOT=<path>` — Captures a window screenshot after `DIMENSHOT_FRAMES` frames (default: `240`) and exits cleanly.

Example (PowerShell):

```powershell
$env:DIMENSHOT_DEMO="1"
$env:DIMENSHOT_FRAMES="240"
$env:DIMENSHOT_SHOT="preview.png"
cargo run --release
```

---

## License

This project is open-source and available under the [MIT License](https://opensource.org/licenses/MIT).
