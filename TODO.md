# Todos:

- ✅ proper options menu which saves in toml format
- ✅ metadata stuff -> hovering over items in inventory and showing tooltips
- better logging -> more info and more warnings using tracing


- Bugfixes:
    - ✅ block outline for water -> should not be there
    - ✅ when hand is empty, the hand is still rendered. also when placing blocks, it does not always remove the item directly
    - ✅ shadows are only rendering upto a certain distance. increase or make it configurable until "infinity"
    - ✅ when changing rendering mode, the transparency of leaves and so on breaks.

    - ✅ Grass sides use mask for color tint to only tint the top and not the dirt part of the texture
        -> the mask in in the texture folder

    - ✅ in f5 / front perspective mode the controls are inverted. always go from cameras perspective when deciding walk direction. 

    - sometimes chunks are missing. 

- Animations:
    - ✅ Block breaking animation
    - ✅ correct break duration aproxximation -> also for eating and so on... 


- Movement:
    - 🚧 sprinting still feels "different" and more sticky than original minecraft
        - ✅ phase 1: ground friction now uses vanilla-style per-block slipperiness (ice/slime/packed ice)
    - 🚧 must be exactly the same as original minecraft in every aspect. Browse original source code if needed for constants and timing or logic.
    - 🚧 same with collision
        - ✅ custom block collision boxes added for many non-full blocks (farmland, ladder, snow layer, cactus, soul sand, trapdoor, carpet, rails, doors, fence gates, walls)
        - ⬜ parity pass for remaining edge cases and per-state corner logic
- Reach
    - ✅ the range i can click blocks or entitys feels further than original. keep it the same





- Potion effects:
    - 🚧 at least the ones affecting movement like speed and jump boost
        - ✅ server packet handling + local sim multiplier wiring for Speed + Jump Boost
        - ⬜ exact vanilla parity tuning/edge cases (stacking/timing with other modifiers)

- Inventory, UI:
    - ✅ armor slots
    - ✅ armor bar over hotbar
    - 🚧 chests
        - ✅ dynamic chest/container window layout + click routing by window id
        - ✅ chest blocks are interactable (right-click use path)
        - 🚧 chest world rendering/collision has a functional placeholder model (refine visuals later)
    - later:
        - crafting
        - smelting
        - enchanting
        - anvil
    - 🚧 correct block rendering in inventory (isometric) and hand (3d)
        - ✅ local debug item browser (`F8`) renders block items from blockstate/model quads (rails/ladders/doors now use non-cube geometry)
        - ⬜ refine UV/parity details and switch from debug browser to production inventory icons


- creative mode
    - ✅ no inventory but flying -> for testing world rendering and mesh building performance


- Physics
    - proper knockback prediction -> right now players bug into blocks when receiving knockback 
    - water / swimming
    - item physics -> interpolation for falling items
    - items on the ground dont always have a texture -> missing metadata?
    - falling blocks -> sand, gravel


- Shading:
    - ✅ saturation options / color grading / sunglasses effect
    - ✅ wavy reflecting(ssr) water

- Rendering in general:
    - 🚧 all block models -> doors, fences...
        - ✅ first high-impact custom model batch implemented (farmland, ladder, snow layer, cactus, soul sand, trapdoor, carpet, rails, doors, fence gates, walls)
        - ⬜ remaining special blocks/states
    - ✅ better antialiasing

    - Lods (optional, very late in development, experimental)

    - Entitys:
        - 🚧 vanilla mob model/animation coverage checklist
            - ✅ Zombie
            - ✅ Skeleton
            - ✅ Pig Zombie
            - ✅ Villager
            - ✅ Enderman
            - ✅ Creeper
            - ✅ Pig
            - ✅ Sheep (base + wool layer)
            - ✅ Cow
            - ✅ Mooshroom
            - ✅ Chicken (temporary quadruped scaffold)
            - ✅ Wolf (temporary quadruped scaffold)
            - ✅ Ocelot (temporary quadruped scaffold)
            - ✅ Horse (temporary quadruped scaffold)
            - ✅ Rabbit (temporary quadruped scaffold)
            - ⬜ Spider
            - ⬜ Cave Spider
            - ⬜ Giant
            - ⬜ Slime
            - ⬜ Magma Cube
            - ⬜ Ghast
            - ⬜ Blaze
            - ⬜ Silverfish
            - ⬜ Endermite
            - ⬜ Bat
            - ⬜ Witch
            - ⬜ Wither
            - ⬜ Ender Dragon
            - ⬜ Guardian
            - ⬜ Squid
            - ⬜ Snow Golem
            - ⬜ Iron Golem

- Debugging:
    - ✅ looked-at block debug info in overlay (pos/id/state/meta/model kind/registry/collision boxes)

- Refactoring:
    - ✅ better plugin management, instead of 1000 systems in main.rs
    - think about where macros could reduce repetition
    - more crates? 
    - more unit tests
    

- Optimisations:
    - ✅ use all available threads for bevy scheduler
    - ✅ make timing analysis a compile time feature flag -> no time counting -> more performance

- Networking
    - implement all packets, ignore some that we dont need yet but at least decode them

<!-- - Debugging: 
    - better timing analysis support -->

- Shipping:
    - tested windows support -> checked it, works
    - bake assets into binary -> whole project single executable + accounts + options files 

- Sounds
    - implement full directional sound support:
        - block breaking and placing
        - steps 
        - entity sounds -> e.g. animals

- Chat
    - 🚧 transparent chat in bottom left corner
        - ✅ basic transparent chat overlay while chat input is closed
    - 🚧 color and formatting support
        - ✅ legacy `§` color/style code rendering pass in UI
        - ⬜ richer JSON chat components / hover / click events
    - autocomplete for commands with tab


- Ideas:
    - wasm support?
        - websocket proxy?  
