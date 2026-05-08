#!/bin/bash
# Force software rendering and blacklist GPU to ensure ZERO VRAM usage
export LIBGL_ALWAYS_SOFTWARE=1
export MESA_LOADER_DRIVER_OVERRIDE=kms_swrast
export WGPU_BACKEND=gl
export WARP_FORCE_SOFTWARE=1

# Memory management: suggest the OS to be aggressive about reclaiming memory
export MALLOC_TRIM_THRESHOLD_=131072

# Enable logging to verify performance and adapter selection
export RUST_LOG="info,wgpu=warn,wgpu_hal=warn,naga=warn,warp::terminal::view=info"

echo "Starting Warp in Software-Only Mode (Zero VRAM)..."
echo "Adapter: Force Software (llvmpipe/swrast)"
echo "Target: target/release/warp-oss"

# Build and run the OSS version
# Using --release for better performance with software rendering
cargo run --release -p warp --bin warp-oss --features gui -- "$@"
