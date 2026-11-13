# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Project39-ar is an AR/VR "character experience simulator" written in Rust: an OpenVR (SteamVR) app that renders MMD models (e.g. Miku), props and tools over a passthrough camera feed, with Rapier physics and an egui debug/inspector UI. Rendering uses Vulkan through vulkano.

## Build & run

- Requires a **nightly** toolchain (`#![feature(try_blocks)]`, `#![feature(negative_impls)]`, `extract_if`); edition 2024.
- `cargo build` / `cargo run`. The `opencv-camera` feature enables the OpenCV camera driver (`cargo run --features opencv-camera`).
- Run without a headset: `cargo run -- --novr --debug`. In novr mode the camera driver must not be `openvr` (it errors), so use `-c dummy` or set `camera.driver = "dummy"` in `config.toml`.
- `cargo run -- --help` lists all CLI flags; `--example_config` prints a default `config.toml`. CLI flags override `config.toml` (read from the working directory) and are generated from the `Config` struct via the `FromArgs` derive (`derive/`, `src/utils/from_args.rs`). Doc comments on config fields become the help text; nested structs become dotted flags (e.g. `novr.enabled`).
- There is no test suite. `test/` is empty, and `component/test.rs` / `mmd/test.rs` are in-app test components, not `cargo test` tests.
- Dependencies come from forks pinned by git rev (vulkano fork in `[patch.crates-io]`, `mmd-rs`). Don't bump them casually.
- `tools/*.js` are Node scripts (`node tools/cleanup_imports.js`) that rewrite `src/**/*.rs` in place, for sorting imports and cleaning up error variants.
- `tools/openviva/import.py` (Python 3 + numpy + PyYAML, ffmpeg) regenerates `assets/openviva/` from an OpenViva Unity project: `python3 -I tools/openviva/import.py <OpenViva dir> [--data-only]`. It contains its own binary FBX and Unity YAML readers.

## Code style

- Tabs for indentation. Imports are grouped std → external crates → `crate::` → `super::`, with two blank lines before the first item (the format `tools/cleanup_imports.js` enforces).
- Errors: `anyhow::Result` everywhere, with `thiserror` for typed error structs and enums.

## Architecture

### Application loop (`src/application/mod.rs`)
`Application` owns the renderer, the Rapier `PhysicsWorld`, input, VR, the window, the eyes, and every entity (`BTreeMap<u64, Entity>`). Shared state sits behind `RefCell`, and almost every API takes `&Application`. Each frame of `Application::run` goes:
input/VR poll → `setup_loop` (adds pending entities and components, calls `start`) → per entity `before_physics` → `physics.step()` → `after_physics` → egui GUI → `tick` on every component (DFS over the entity tree) → render (eyes, then window mirror or detached POV) → `cleanup_loop` (calls `end`, removes entities whose parent was removed unless they `persist`, cleans up physics handles).
The initial scene (floor, VR root or PC-controlled POV with hands, Miku) is built in `Application::new`.

### Entities and components
- `Entity` (`application/entity/`) holds a transform/state, an optional rigid body and colliders, a parent/children hierarchy, string-keyed `tags` (e.g. `"World"`, `"NoGrab"`, `"Hand"`), and its components. Build entities with `Entity::builder(name)...build()` and add them with `application.add_entity`. They only appear on the next `setup_loop`.
- Components implement `ComponentBase` (via `#[derive(ComponentBase)]`, which needs an `#[inner] inner: ComponentInner` field) plus `Component`, whose lifecycle hooks are `start`, `tick`, `before_render`, `render`, `end`, `on_inspect`/`on_gui`. Hooks take `&self`, so mutable state goes in `Cell`/`RefCell`. `src/component/example.rs` is the minimal template.
- `ComponentInner::new_norender()` vs `from_render_type(...)` decides whether the renderer calls the component's render hooks.
- Cross-references use the weak handles `EntityRef` and `ComponentRef<C>` (ids resolved through `.get(application)`), never borrows. Removing something is deferred: `remove()` marks it, and the cleanup loop finishes the job.
- Physics glue lives in `src/utils/rapier.rs` (extension traits such as `ColliderEx::entity`). Joints are components in `component/physics/`.

### Rendering (`src/renderer/`)
- `Renderer` wraps the vulkano device and queues. `RenderTarget` is implemented both by `Eyes` (VR or novr stereo eye framebuffers plus the camera background with lens undistortion, `application/eyes/`) and by `Window`.
- Vulkan 1.3 with dynamic rendering (no render passes, except the egui overlay, which `egui_winit_vulkano` requires). Every render target, including the window, renders 2-layer multiview (`LAYERS`/`VIEW_MASK`). Pipelines are built with `build_graphics_pipeline`, which targets that layout.
- Each model or effect owns its pipeline, with GLSL shaders inlined through `vulkano_shaders::shader!` in a `pipeline/` submodule next to the component (e.g. `component/model/mmd/pipeline`).
- `AssetsManager` (`renderer/assets_manager/`) loads assets through `renderer.load(SomeAsset::at(path, ...))`, resolving paths against `assets/` with `assets_overrides/` taking precedence (overrides are gitignored). `assets/props.toml` defines the props the toolgun can spawn.

### Toolgun (`src/component/toolgun/`)
`ToolGun` is a component that cycles through `Tool` trait objects (spawner, remover, axis, thruster, weld, rope, ...) registered in `tool.rs::get_all_tools`. A new tool has to be added there. Each tool gets `tick(toolgun, hand, ray, application)` along with render hooks.

### Character (`src/component/miku/`)
A port of OpenViva's character ("Lily"/Shinobu) driving Miku, enabled by the `[character]` section of the model's `model.toml` (humanoid bone mapping, face morph mapping, headpat/face/tummy spheres).
- `anim/`: imported clips (`.p39a`) are stored in a skeleton independent humanoid space (`human.rs`: every bone has an identity rest rotation, character faces +Z, +X is her left). `Animator` emulates Unity's animator (layers legs/torso/face, cross-fades, 1D/2D blend trees, float parameters); `Retarget` maps humanoid poses onto MMD bones, aligning the rest bone directions.
- `character/`: one `Character` struct split across files like OpenViva's partial `Lily` class. `animation.rs` is the animation state machine (priorities, body states, transitions, events) driven by `assets/openviva/animations.json`; `puppet.rs` is the PuppetMaster-like physics: every ragdoll body (`BodyPart` joints) is driven towards the animated pose by setting velocities (pins + muscles), the hips are moved by locomotion, and losing ground starts ragdoll mode, falling, curling up and getting up. Behaviors: `headpat`, `poke`, `proximity`, `handhold`, `gestures`, `idle` (idle, follow), `chopsticks`, plus `look` (look-at) and `voice`.
- In novr mode, `H`/`F` trigger the wave/follow gestures; the Miku inspector has buttons to start the chopsticks game or ragdoll her.
- `P39_MIKU_SCENARIO=<headpat|rough_headpat|poke|tummy|pickup|handhold|follow|wave|chopsticks>` runs a scripted interaction (`character/scenario.rs`) that moves the player's head and hands, printing the character's log. Use with `--novr -c dummy` to test without VR.

### Audio
`Application::audio` (`application/audio.rs`) plays positional sounds with rodio, heard from the POV. Load sounds with `renderer.load(SoundAsset::at(path))`.

### Debug
`src/debug.rs` handles the global debug toggle (`--debug`), string-keyed debug flags (`debug::get_flag_or_default("DebugCollidersDraw")`, defaults set in `setup_default_flags`), the `dprintln!` macro, and immediate-mode debug drawing (`debug::draw_text`, lines/points) rendered by `renderer/debug_renderer`. The egui inspector (`application/gui`) shows components through their `on_inspect` implementations.
