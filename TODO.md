# Todos:

- Block borders
- proper options menu which saves in toml format
- metadata stuff -> hovering over items in inventory and showing tooltips

- Bugfixes:
    - block outline for water -> should not be there

- Inventory, UI:
    - armor slots
    - armor bar over hotbar
    - chests
    - later:
        - crafting
        - smelting
        - enchanting
        - anvil


- creative mode
    - no inventory but flying -> for testing world rendering and mesh building performance


- Physics
    - proper knockback prediction -> right now players bug into blocks when receiving knockback 
    - water / swimming
    - item physics
    - falling blocks -> sand, gravel


- Shading:
    - saturation options / color grading / sunglasses effect
    - wavy reflecting(ssr) water

- Rendering in general:
    - all block models -> doors, fences...
    - better antialiasing
    - Lods (optional, very late in development, experimental)

    - Entitys: 
        - other animals with animations

- Refactoring:
    - better plugin management, instead of 1000 systems in main.rs
    - think about where macros could reduce repetition
    - more crates? 
    - more unit tests
    

- Optimisations:
    - use all available threads for bevy scheduler
    - make timing analysis a compile time feature flag -> no time counting -> more performance

- Debugging: 
    - better timing analysis support

- Shipping:
    - tested windows support
    - bake assets into binary -> whole project single executable + accounts + options files 

- Ideas:
    - wasm support?
        - websocket proxy?  