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

- Animations:
    - ✅ Block breaking animation
    - ✅ correct break duration aproxximation -> also for eating and so on... 


- Movement:
    - 🚧 sprinting still feels "different" and more sticky than original minecraft
        - ✅ phase 1: ground friction now uses vanilla-style per-block slipperiness (ice/slime/packed ice)
    - 🚧 must be exactly the same as original minecraft in every aspect. Browse original source code if needed for constants and timing or logic.
    - same with collision
- Reach
    - ✅ the range i can click blocks or entitys feels further than original. keep it the same





- Potion effects:
    - at least the ones affecting movement like speed and jump boost  

- Inventory, UI:
    - ✅ armor slots
    - ✅ armor bar over hotbar
    - chests
    - later:
        - crafting
        - smelting
        - enchanting
        - anvil
    - correct block rendering in inventory (isometric) and hand (3d)


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
    - all block models -> doors, fences...
    - ✅ better antialiasing

    - Lods (optional, very late in development, experimental)

    - Entitys: 
        - other animals with animations 

- Refactoring:
    - ✅ better plugin management, instead of 1000 systems in main.rs
    - think about where macros could reduce repetition
    - more crates? 
    - more unit tests
    

- Optimisations:
    - ✅ use all available threads for bevy scheduler
    - ✅ make timing analysis a compile time feature flag -> no time counting -> more performance

<!-- - Debugging: 
    - better timing analysis support -->

- Shipping:
    - tested windows support -> checked it, works
    - bake assets into binary -> whole project single executable + accounts + options files 

- Ideas:
    - wasm support?
        - websocket proxy?  
