# Toggleable Fancy Lighting Architecture for a Bevy Voxel Engine

## Goal

Add shaderpack-style lighting (sun shadows, SSAO, fog, water shading) while preserving a very fast baseline renderer. The system must scale by quality tier and be cleanly toggleable.

Core principle:
- Keep a cheap baked-light pipeline that always works.
- Add advanced effects as separate render passes or pipelines.
- Ensure each feature has bounded and measurable cost.
- Avoid mixing dynamic lighting complexity into the baseline path.

---

## 1. Baseline Lighting (Fast Path)

This path must remain extremely cheap and predictable.

World data:
- Store skylight and blocklight per voxel (4–8 bits each).
- Maintain light propagation inside chunk data.
- Keep lighting data cache-friendly (SoA layout recommended).

Meshing stage:
- Compute per-vertex ambient occlusion using 3-neighbor voxel AO.
- Sample voxel light volume per vertex.
- Pack AO + light into vertex color or packed integer attribute.
- Avoid runtime light sampling in fragment shader.

Shader:
- Simple lambert-style directional term for sun.
- Multiply by baked light + AO.
- No dynamic shadow sampling.
- No additional render passes.

This path should scale to large view distances with minimal GPU cost.

---

## 2. Directional Sun Shadows (Fancy Mode)

Use Cascaded Shadow Maps (CSM) for the main directional light.

Why:
- Stable solution for large terrains.
- Preserves shadow resolution near camera.
- Industry-standard approach for open worlds.

Configurable parameters:
- Cascades: 2–4 maximum.
- Shadow distance: 64–128 blocks.
- Resolution: 1024² (low) to 2048² (high).
- PCF taps: 4 (low) to 16 (high).

Shadow optimizations:
- Render only opaque voxel geometry into shadow maps.
- Skip foliage and transparent blocks in shadow pass.
- Use simplified shadow caster mesh if possible.
- Drop tiny decorative features in far cascades.
- Snap cascade projections to texel grid for stability.
- Avoid updating shadows every frame if sun is static.

Performance reality:
Each cascade re-renders visible geometry. 3 cascades roughly means up to 3x shadow pass geometry cost. Keep cascade count conservative.

---

## 3. Screen-Space Effects (Scalable Enhancements)

These add significant visual depth at moderate cost.

SSAO:
- Half-resolution for low/medium settings.
- 4–8 samples (low), 12–16 (high).
- Blur + depth-aware filter.
- Toggleable independently of shadows.

Fog:
- Height-based or distance-based fog.
- Use to hide far shadow aliasing.
- Cheap exponential or linear model is sufficient.

Volumetrics (optional):
- Light scattering approximation in view space.
- Should be gated behind highest quality tier.

---

## 4. Water Rendering Strategy

Treat water as a separate material/pipeline.

Features:
- Animated normal mapping.
- Fresnel term.
- Depth-based absorption.
- Optional refraction sampling from scene color buffer.

Performance control:
- Simplify distant water geometry.
- Avoid per-voxel side faces for large bodies at distance.
- Keep water out of shadow casting where acceptable.

---

## 5. Renderer Architecture

Separate pipelines rather than branching heavily inside one shader.

Fast Pipeline:
- Baked light + AO.
- No shadow maps.
- Minimal fragment work.

Fancy Pipeline:
- Adds shadow map sampling.
- Optional SSAO composite.
- Fog and tone mapping.
- Water shading pipeline.

Switching modes:
- Swap render pipelines and bind groups.
- Do not spawn/despawn light entities repeatedly.
- Avoid runtime shader branching for large feature toggles.

---

## 6. Quality Ladder

Fast:
- Baked voxel light + AO.
- Fog only.

Standard:
- Fast + basic directional lighting math.
- No shadow maps.

Fancy Low:
- 2 cascades.
- 1024² shadow maps.
- Half-resolution SSAO.
- 4-tap PCF.

Fancy High:
- 3–4 cascades.
- 2048² near cascade.
- Full-resolution SSAO.
- 9–16 tap PCF.
- Water refraction.
- Optional volumetrics.

---

## 7. Additional Performance Safeguards

- Budget mesh uploads per frame.
- Budget shadow map updates per frame.
- Cap dynamic point lights aggressively.
- Prefer baked emissive light for torches.
- Track p99 frame time, not just average FPS.
- Profile shadow pass draw calls separately from main pass.

---

## 8. Implementation Order Recommendation

1. Ensure baked voxel light + AO path is stable and fast.
2. Add 2-cascade shadow mapping with strict shadow distance cap.
3. Introduce half-resolution SSAO.
4. Add shadow caster LOD simplification.
5. Expand quality ladder and tune cascade splits.
6. Only then add advanced water or volumetric effects.

---

## Summary

Maintain a fast, baked-light baseline path.
Implement directional shadows using conservative cascaded shadow maps.
Add screen-space effects for perceived quality.
Architect everything as separable, toggleable pipelines.
Expose clear quality tiers to scale cost predictably.

The goal is not maximum visual fidelity at all times, but controllable, bounded complexity that preserves performance stability.
