# GSoda - Fast 3D G-code Viewer

A high-performance 3D G-code visualizer built with Rust, Macroquad, and the gcode parsing library.

## Features

- **Fast rendering** - Precomputes geometry once, minimal per-frame overhead
- **3D visualization** - Interactive orbit camera with mouse controls
- **Color-coded paths** - Blue for extrusion moves, red for travel moves
- **Layer filtering** - Toggle to view specific layer ranges
- **Auto-scaling** - Automatically fits model to viewport
- **Cross-platform** - Runs on Linux, macOS, and Windows

## Building

```bash
cargo build --release
```

## Usage

```bash
cargo run --release -- <gcode-file>
```

Example:
```bash
cargo run --release -- auto1.gcode
```

## Controls

| Input | Action |
|-------|--------|
| **Mouse drag** | Rotate camera around model |
| **Mouse scroll** | Zoom in/out |
| **R** | Reset camera to default position |
| **L** | Toggle layer filtering on/off |
| **Up/Down arrows** | Adjust visible layer height (when filtering enabled) |
| **Esc** | Quit application |

## Architecture

- **Parser** (`parse_gcode`): Processes G-code with the `gcode` crate, tracking G0/G1 moves and extrusion state
- **Geometry** (`LineSegment`): Precomputes all line segments with extrusion flags
- **Camera** (`Camera`): Implements orbit controls with yaw/pitch/distance
- **Renderer**: Uses Macroquad's 3D drawing functions for efficient line rendering

## Performance

The viewer is optimized for large G-code files:
- Parses and caches all geometry upfront
- Minimal per-frame computation (only camera updates)
- Release builds use LTO and high optimization levels
- Efficient layer filtering with Z-coordinate culling

## Dependencies

- **macroquad** - Simple and easy to use game library for 3D graphics
- **gcode** - Robust G-code parser supporting standard commands
- **anyhow** - Idiomatic error handling

## License

MIT
