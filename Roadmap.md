# Ruststone Roadmap

This roadmap is derived from `TODO.md` and grouped by priority and implementation dependency.

## 1. Gameplay Correctness (Highest Priority)

- Movement parity with vanilla 1.8.9
  - remove sticky sprint feel
  - match timing/constants/logic with decompiled source
- Collision parity with vanilla 1.8.9
  - edge cases for stairs/slabs/fences/doors
  - remove client/server desync edge cases
- Perspective control fix
  - in front third-person (`F5` front), movement should be camera-relative (not inverted)

## 2. Core Combat/Physics Stability

- Knockback prediction + reconciliation
  - prevent players clipping into blocks after knockback
- Water/swimming behavior parity
  - acceleration, drag, buoyancy, jump-out behavior matching vanilla
- Falling block simulation
  - sand/gravel as entities + server sync behavior
- Item entity physics polish
  - interpolation/smoothing for dropped items
  - verify metadata mapping for item textures on ground

## 3. Inventory + Interaction Expansion

- Containers
  - chests (open, sync slots, click handling)
- Crafting/smelting/enchanting/anvil (later stage)
  - server packet support first
  - UI flows second
- Item/block rendering in UI and hand
  - correct 3D hand model rendering
  - correct isometric inventory block rendering

## 4. Rendering Coverage

- Finish missing block models and states
  - doors, fence variants, remaining special blocks
- Entity model coverage
  - additional animals + basic animation sets
- Optional late-stage rendering experiments
  - LODs (experimental, opt-in)

## 5. Effects + Attributes

- Potion effects impacting movement
  - at minimum: speed + jump boost
  - apply in local sim and reconcile with server updates

## 6. Refactoring + Reliability

- Macro pass where repetition is high
  - model/texture mappings
  - packet mapping boilerplate
- Test coverage expansion
  - movement invariants
  - reconcile/replay correctness
  - inventory click transaction correctness
- Optional crate boundary cleanup
  - split systems further only if it reduces coupling

## 7. Shipping

- Single executable packaging
  - embed assets in binary
  - keep options/accounts external and compatible
- Keep Windows support validated per release

## 8. Stretch Goals

- WASM support research
  - networking strategy (likely websocket proxy)

---

## Suggested Execution Order

1. Movement/collision parity + front-perspective controls.
2. Knockback + water/swim + falling blocks.
3. Chest/container networking and UI.
4. Remaining block/entity render coverage.
5. Refactor/tests + packaging pass.

## Step 1 Status

- ✅ Front-perspective controls (camera-relative) implemented.
- 🚧 Movement parity in progress (phase 1 done: block slipperiness/friction parity).
- ⬜ Collision parity edge cases (stairs/slabs/fences/doors) still pending.
