# Ruststone

Ruststone is a Minecraft Java Edition `1.8.9`-targeted Rust client built with Bevy.
The long-term goal is playable PvP (including Bedwars-style gameplay), with protocol compatibility first and feature-completeness in iterations.

## Current progress

### Core gameplay and rendering
- 20 TPS fixed-step local simulation with client-side prediction + reconciliation.
- Basic movement/camera flow, collisions, sneaking, sprinting, jumping.
- World/chunk decoding and meshing with async mesh generation.
- Block texture atlas rendering, water transparency improvements, biome tint support.
- Manual/engine culling toggles, render distance controls, wireframe and performance overlays.

### Networking and gameplay loop
- Connect/login/play loop against 1.8.9 servers.
- Chunk + block updates wired into world state and remeshing.
- Local player movement send + server correction handling.
- Remote entity scaffolding for players, mobs, objects, dropped items and XP orbs.
- Entity click interactions (attack/interact), knockback velocity ingest, item use in air.
- Chat, health/death/respawn flow, hotbar drop (`Q`, `Ctrl+Q`).

### UI (egui)
- Connect screen, chat, pause menu, debug menu, death screen.
- Crosshair + block break progress indicator.
- Survival inventory/hotbar window scaffolding with server-synced item stacks.
- Inventory item textures loaded from the client texture pack.
- Inventory interactions implemented for common survival actions:
  - left/right click
  - shift-click
  - number-key swap
  - Q / Ctrl+Q drop
  - double-click collect
  - outside click drop

## Packet support status (Minecraft 1.8.9 / protocol 47)

Legend:
- `Implemented`: encoded/decoded and used by client logic.
- `Partial`: parsed but currently ignored or only partly used.
- `Not implemented`: currently not handled by Ruststone logic.

### Handshake/Login packets

| State | Direction | Packet | Status | Notes |
| --- | --- | --- | --- | --- |
| Handshake | Serverbound | `Handshake` | Implemented | Sent during connect sequence. |
| Login | Serverbound | `LoginStart` | Implemented | Username login start. |
| Login | Serverbound | `EncryptionResponse` | Not implemented | Offline-mode path currently. |
| Login | Clientbound | `SetInitialCompression` | Implemented | Compression threshold applied. |
| Login | Clientbound | `LoginSuccess_String` / `LoginSuccess_UUID` | Implemented | Transition to play state. |
| Login | Clientbound | `LoginDisconnect` | Not implemented | Disconnect path not explicitly mapped. |
| Login | Clientbound | `EncryptionRequest` | Not implemented | Online-mode auth/encryption not wired yet. |

### Serverbound Play packets

| ID | Packet | Status | Notes |
| --- | --- | --- | --- |
| `0x00` | `KeepAliveServerbound_VarInt` | Implemented | Reply path for keepalive. |
| `0x01` | `ChatMessage` | Implemented | Chat send. |
| `0x02` | `UseEntity_Handsfree` | Implemented | Attack/interact entity packets. |
| `0x03` | `Player` | Not implemented |  |
| `0x04` | `PlayerPosition` | Not implemented |  |
| `0x05` | `PlayerLook` | Not implemented |  |
| `0x06` | `PlayerPositionLook` | Implemented | Main movement send packet. |
| `0x07` | `PlayerDigging_u8` | Implemented | Start/cancel/finish dig + drop item/stack actions. |
| `0x08` | `PlayerBlockPlacement_u8_Item` | Implemented | Place block + right-click-air item use path. |
| `0x09` | `HeldItemChange` | Implemented | Hotbar select + wheel/number keys. |
| `0x0A` | `ArmSwing_Handsfree` | Implemented | Left-click swing animation packet. |
| `0x0B` | `PlayerAction` | Implemented | Sneak/sprint action toggles. |
| `0x0C` | `SteerVehicle` | Not implemented |  |
| `0x0D` | `CloseWindow` | Implemented | Inventory window close. |
| `0x0E` | `ClickWindow_u8` | Implemented | Survival inventory interaction path. |
| `0x0F` | `ConfirmTransactionServerbound` | Implemented | Rejection-ack flow. |
| `0x10` | `CreativeInventoryAction` | Not implemented |  |
| `0x11` | `EnchantItem` | Not implemented |  |
| `0x12` | `SetSign` | Not implemented |  |
| `0x13` | `ClientAbilities_f32` | Not implemented |  |
| `0x14` | `TabComplete_NoAssume` | Not implemented |  |
| `0x15` | `ClientSettings_u8_Handsfree` | Not implemented |  |
| `0x16` | `ClientStatus` | Implemented | Respawn action. |
| `0x17` | `PluginMessageServerbound` | Not implemented |  |
| `0x18` | `SpectateTeleport` | Not implemented |  |
| `0x19` | `ResourcePackStatus` | Not implemented |  |

### Clientbound Play packets

| ID | Packet | Status | Notes |
| --- | --- | --- | --- |
| `0x00` | `KeepAliveClientbound_VarInt` | Implemented | Immediate reply sent. |
| `0x01` | `JoinGame_i8` | Implemented | Local player entity id wiring. |
| `0x02` | `ServerMessage_Position` | Implemented | Chat ingest (`ServerMessage_*` variants). |
| `0x03` | `TimeUpdate` | Not implemented |  |
| `0x04` | `EntityEquipment_u16` | Partial | Parsed, ignored. |
| `0x05` | `SpawnPosition` | Not implemented |  |
| `0x06` | `UpdateHealth` | Implemented | Health/food/death state. |
| `0x07` | `Respawn_Gamemode` | Not implemented |  |
| `0x08` | `TeleportPlayer_NoConfirm` | Implemented | Also handles protocol variants. |
| `0x09` | `SetCurrentHotbarSlot` | Implemented | Hotbar slot sync. |
| `0x0A` | `EntityUsedBed` | Not implemented |  |
| `0x0B` | `Animation` | Not implemented |  |
| `0x0C` | `SpawnPlayer_i32_HeldItem` | Implemented | Also handles other spawn player variants. |
| `0x0D` | `CollectItem_nocount` | Not implemented |  |
| `0x0E` | `SpawnObject_i32_NoUUID` | Implemented | Spawned as typed placeholder visuals. |
| `0x0F` | `SpawnMob_u8_i32_NoUUID` | Implemented | Spawned as typed placeholder visuals. |
| `0x10` | `SpawnPainting_NoUUID` | Not implemented |  |
| `0x11` | `SpawnExperienceOrb_i32` | Implemented | Spawned as placeholder orb visuals. |
| `0x12` | `EntityVelocity` | Implemented | Local player/server entity knockback velocity applied. |
| `0x13` | `EntityDestroy` | Implemented | Also handles `EntityDestroy_u8`. |
| `0x14` | `Entity` | Not implemented |  |
| `0x15` | `EntityMove_i8` | Implemented | Also handles alternate move variants. |
| `0x16` | `EntityLook_VarInt` | Implemented | Also handles alternate look variants. |
| `0x17` | `EntityLookAndMove_i8` | Implemented | Also handles alternate variants. |
| `0x18` | `EntityTeleport_i32` | Implemented | Also handles alternate variants. |
| `0x19` | `EntityHeadLook` | Partial | Parsed, ignored. |
| `0x1A` | `EntityStatus` | Not implemented |  |
| `0x1B` | `EntityAttach_leashed` | Not implemented |  |
| `0x1C` | `EntityMetadata` | Partial | Parsed for dropped-item labels from stack metadata. |
| `0x1D` | `EntityEffect` | Not implemented |  |
| `0x1E` | `EntityRemoveEffect` | Not implemented |  |
| `0x1F` | `SetExperience` | Not implemented |  |
| `0x20` | `EntityProperties` | Partial | Parsed, ignored. |
| `0x21` | `ChunkData_NoEntities_u16` | Implemented | Handled via `ChunkData` decode path. |
| `0x22` | `MultiBlockChange_VarInt` | Implemented | Also handles `MultiBlockChange_u16`. |
| `0x23` | `BlockChange_VarInt` | Implemented | Also handles `BlockChange_u8`. |
| `0x24` | `BlockAction` | Not implemented |  |
| `0x25` | `BlockBreakAnimation` | Not implemented |  |
| `0x26` | `ChunkDataBulk` | Implemented | Chunk ingest/decode path. |
| `0x27` | `Explosion` | Not implemented |  |
| `0x28` | `Effect` | Not implemented |  |
| `0x29` | `NamedSoundEffect_u8_NoCategory` | Not implemented |  |
| `0x2A` | `Particle_VarIntArray` | Not implemented |  |
| `0x2B` | `ChangeGameState` | Not implemented |  |
| `0x2C` | `SpawnGlobalEntity_i32` | Not implemented |  |
| `0x2D` | `WindowOpen` | Implemented | Inventory windows + variants. |
| `0x2E` | `WindowClose` | Implemented | Inventory close sync. |
| `0x2F` | `WindowSetSlot` | Implemented | Inventory slot delta updates. |
| `0x30` | `WindowItems` | Implemented | Inventory bulk updates. |
| `0x31` | `WindowProperty` | Not implemented |  |
| `0x32` | `ConfirmTransaction` | Implemented | Reject/ack path wired. |
| `0x33` | `UpdateSign` | Not implemented |  |
| `0x34` | `Maps_NoTracking` | Not implemented |  |
| `0x35` | `UpdateBlockEntity` | Partial | Parsed, ignored. |
| `0x36` | `SignEditorOpen` | Not implemented |  |
| `0x37` | `Statistics` | Not implemented |  |
| `0x38` | `PlayerInfo` | Implemented | Name/UUID registry updates. |
| `0x39` | `PlayerAbilities` | Not implemented |  |
| `0x3A` | `TabCompleteReply` | Not implemented |  |
| `0x3B` | `ScoreboardObjective` | Not implemented |  |
| `0x3C` | `UpdateScore` | Not implemented |  |
| `0x3D` | `ScoreboardDisplay` | Not implemented |  |
| `0x3E` | `Teams_u8` | Not implemented |  |
| `0x3F` | `PluginMessageClientbound` | Not implemented |  |
| `0x40` | `Disconnect` | Not implemented | Disconnect currently handled via read error path. |
| `0x41` | `ServerDifficulty` | Not implemented |  |
| `0x42` | `CombatEvent` | Not implemented |  |
| `0x43` | `Camera` | Not implemented |  |
| `0x44` | `WorldBorder` | Not implemented |  |
| `0x45` | `Title_notext_component` | Not implemented |  |
| `0x46` | `SetCompression` | Not implemented (play) | Compression is handled during login (`SetInitialCompression`). |
| `0x47` | `PlayerListHeaderFooter` | Not implemented |  |
| `0x48` | `ResourcePackSend` | Not implemented |  |
| `0x49` | `EntityUpdateNBT` | Not implemented |  |

## Known gaps

- Breaking indicator is currently a local estimate and not yet server-confirmed timing.
- Dropped item labels currently rely on metadata parsing plus static fallback naming (not full translation/NBT naming parity).
- Inventory drag-splitting (`mode 5`) not implemented yet.
- Many play packets are still intentionally unimplemented (see matrix above).
- Protocol support target is currently `1.8.9` only.

## Credits and attribution

- Networking/protocol foundation and architecture references were adapted from **Leafish** and related Steven/Stevenarella lineage:
  - https://github.com/Lea-fish/Leafish
  - Leafish is dual-licensed (`MIT` / `Apache-2.0`).
- Binary greedy meshing integration was built from local adaptation study of:
  - `binary_greedy_mesher_demo` in this workspace
  - and its referenced algorithm resources, including:
    - https://github.com/cgerikj/binary-greedy-meshing

Please keep upstream license and attribution requirements when reusing or redistributing derived code.
