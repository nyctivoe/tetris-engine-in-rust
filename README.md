# Tetris Engine In Rust

This repo attempts to replicate the [tetr.io](https://tetr.io) tetris engine in the Rust programming language.

## Coverage

- board geometry, collision, spawn, hold, lock, combo, B2B, surge, and spin detection
- garbage queue management, garbage cancellation, timer ticks, and garbage push-up top-out
- reachable-placement BFS with Python-compatible metadata and final-result dedupe
- parity fixture loading and comparison against Python-exported canonical JSON
- reproducible Rust/Python BFS timing harness

Defaults:

- `spin_mode = all_spin`
- `b2b_mode = surge`
- board representation stays flat as `[i8; 400]`

## Python Integration

The Rust engine can be built as a Python extension module.

```zsh
uvx maturin develop --release
```

The extension module name is `tetrisEngine_rs`.
