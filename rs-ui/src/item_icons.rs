use super::*;

#[derive(Resource)]
pub struct ItemIconCache {
    loaded: HashMap<(i32, i16), egui::TextureHandle>,
    missing: HashSet<(i32, i16)>,
    block_model_resolver: BlockModelResolver,
    block_texture_images: HashMap<String, Option<egui::ColorImage>>,
    logged_stone_fallback: HashSet<(i32, i16)>,
    logged_model_fallback: HashSet<(i32, i16)>,
}

impl Default for ItemIconCache {
    fn default() -> Self {
        Self {
            loaded: HashMap::new(),
            missing: HashSet::new(),
            block_model_resolver: BlockModelResolver::new(default_model_roots()),
            block_texture_images: HashMap::new(),
            logged_stone_fallback: HashSet::new(),
            logged_model_fallback: HashSet::new(),
        }
    }
}

impl ItemIconCache {
    pub(crate) fn texture_for_stack(
        &mut self,
        ctx: &egui::Context,
        stack: &InventoryItemStack,
    ) -> Option<egui::TextureId> {
        let key = (stack.item_id, stack.damage);
        if let Some(handle) = self.loaded.get(&key) {
            return Some(handle.id());
        }
        if self.missing.contains(&key) {
            return None;
        }

        let is_block_item = u16::try_from(stack.item_id)
            .ok()
            .and_then(block_registry_key)
            .is_some();

        let candidates = item_texture_candidates(stack.item_id, stack.damage);
        let mut first_candidate_image: Option<(String, egui::ColorImage)> = None;
        let mut has_explicit_item_texture = false;
        for rel_path in &candidates {
            let full_path = texturepack_textures_root().join(&rel_path);
            if !full_path.exists() {
                continue;
            }
            if rel_path.starts_with("items/") {
                has_explicit_item_texture = true;
            }
            let Some(color_image) = load_color_image(&full_path) else {
                continue;
            };
            first_candidate_image = Some((rel_path.clone(), color_image));
            break;
        }

        if is_block_item
            && !has_explicit_item_texture
            && let Some((image, source)) = generate_isometric_block_icon(
                stack.item_id,
                stack.damage,
                &mut self.block_model_resolver,
                &mut self.block_texture_images,
                &mut self.logged_stone_fallback,
                &mut self.logged_model_fallback,
            )
        {
            // If the model path collapsed all the way to a manual cube but a more accurate
            // block texture candidate exists, prefer that flat fallback over a fake cube.
            if source == IsometricIconSource::ManualCubeFallback
                && !has_explicit_item_texture
                && let Some((rel_path, color_image)) = first_candidate_image.take()
            {
                let texture_name =
                    format!("item_icon_{}_{}_{}", stack.item_id, stack.damage, rel_path);
                let handle =
                    ctx.load_texture(texture_name, color_image, egui::TextureOptions::NEAREST);
                let id = handle.id();
                self.loaded.insert(key, handle);
                return Some(id);
            }

            let texture_name = format!("item_icon_iso_{}_{}", stack.item_id, stack.damage);
            let handle = ctx.load_texture(texture_name, image, egui::TextureOptions::NEAREST);
            let id = handle.id();
            self.loaded.insert(key, handle);
            return Some(id);
        }

        if let Some((rel_path, color_image)) = first_candidate_image {
            let texture_name = format!("item_icon_{}_{}_{}", stack.item_id, stack.damage, rel_path);
            let handle = ctx.load_texture(texture_name, color_image, egui::TextureOptions::NEAREST);
            let id = handle.id();
            self.loaded.insert(key, handle);
            return Some(id);
        }

        if stack.damage != 0 {
            let fallback_key = (stack.item_id, 0);
            if let Some(handle) = self.loaded.get(&fallback_key) {
                return Some(handle.id());
            }
        }

        warn!(
            "Icon text fallback for id={} meta={} key={:?}",
            stack.item_id,
            stack.damage,
            item_registry_key(stack.item_id)
        );
        self.missing.insert(key);
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IsometricIconSource {
    BlockstateModel,
    ItemModel,
    ManualCubeFallback,
}

fn generate_isometric_block_icon(
    item_id: i32,
    damage: i16,
    resolver: &mut BlockModelResolver,
    texture_cache: &mut HashMap<String, Option<egui::ColorImage>>,
    logged_stone_fallback: &mut HashSet<(i32, i16)>,
    logged_model_fallback: &mut HashSet<(i32, i16)>,
) -> Option<(egui::ColorImage, IsometricIconSource)> {
    let block_id = u16::try_from(item_id).ok()?;
    if block_registry_key(block_id).is_none() {
        return None;
    }

    if let Some(image) = generate_custom_block_icon(block_id, texture_cache) {
        return Some((image, IsometricIconSource::BlockstateModel));
    }

    if let Some(mut quads) = resolver.icon_quads_for_meta(block_id, damage as u8) {
        quads.sort_by(|a, b| {
            quad_depth(a)
                .partial_cmp(&quad_depth(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
        let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
        let mut rendered_any = false;
        for quad in quads {
            let Some(tex) = load_model_texture(&quad.texture_path, texture_cache) else {
                continue;
            };
            rendered_any = true;
            let tint = quad
                .tint_index
                .and_then(|_| icon_tint_color(block_id, damage))
                .unwrap_or([255, 255, 255]);
            raster_iso_quad(&mut out, &mut depth, &quad, &tex, tint);
        }
        if rendered_any {
            return Some((out, IsometricIconSource::BlockstateModel));
        }
        if logged_model_fallback.insert((item_id, damage)) {
            warn!(
                "[isometric-debug] model texture fallback id={} meta={} key={:?}",
                item_id,
                damage,
                block_registry_key(block_id)
            );
        }
    }

    if let Some(quads) = resolver.block_item_icon_quads(block_id, damage as u8) {
        let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
        let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
        let mut rendered_any = false;
        for quad in quads {
            let Some(tex) = load_model_texture(&quad.texture_path, texture_cache) else {
                continue;
            };
            rendered_any = true;
            let tint = quad
                .tint_index
                .and_then(|_| icon_tint_color(block_id, damage))
                .unwrap_or([255, 255, 255]);
            raster_iso_quad(&mut out, &mut depth, &quad, &tex, tint);
        }
        if rendered_any {
            return Some((out, IsometricIconSource::ItemModel));
        }
    }

    // Guaranteed fallback: render a textured isometric cube so block items are never flat.
    let top_name = resolver
        .face_texture_name_for_meta(block_id, damage as u8, ModelFace::PosY)
        .filter(|name| !(block_id != 1 && name == "stone.png"))
        .or_else(|| fallback_block_face_texture(block_id, damage, BlockFace::Up))
        .unwrap_or_else(|| block_texture_name(block_id, BlockFace::Up).to_string());
    let east_name = resolver
        .face_texture_name_for_meta(block_id, damage as u8, ModelFace::PosX)
        .filter(|name| !(block_id != 1 && name == "stone.png"))
        .or_else(|| fallback_block_face_texture(block_id, damage, BlockFace::East))
        .unwrap_or_else(|| block_texture_name(block_id, BlockFace::East).to_string());
    let south_name = resolver
        .face_texture_name_for_meta(block_id, damage as u8, ModelFace::PosZ)
        .filter(|name| !(block_id != 1 && name == "stone.png"))
        .or_else(|| fallback_block_face_texture(block_id, damage, BlockFace::South))
        .unwrap_or_else(|| block_texture_name(block_id, BlockFace::South).to_string());
    let top = load_block_texture(&top_name, texture_cache)?;
    let east = load_block_texture(&east_name, texture_cache)?;
    let south = load_block_texture(&south_name, texture_cache)?;
    if block_id != 1
        && (top_name == "stone.png" || east_name == "stone.png" || south_name == "stone.png")
    {
        if logged_stone_fallback.insert((item_id, damage)) {
            warn!(
                "[isometric-debug] stone texture fallback id={} meta={} key={:?} top={} east={} south={}",
                item_id,
                damage,
                block_registry_key(block_id),
                top_name,
                east_name,
                south_name
            );
        }
    }

    let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
    let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
    let cube_faces = [
        (
            IconQuad {
                vertices: [
                    [0.0, 0.0, 1.0],
                    [1.0, 0.0, 1.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: format!("blocks/{south_name}"),
                tint_index: None,
            },
            south,
        ),
        (
            IconQuad {
                vertices: [
                    [1.0, 0.0, 1.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                ],
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: format!("blocks/{east_name}"),
                tint_index: None,
            },
            east,
        ),
        (
            IconQuad {
                vertices: [
                    [0.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [1.0, 1.0, 1.0],
                    [0.0, 1.0, 1.0],
                ],
                uv: [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
                texture_path: format!("blocks/{top_name}"),
                tint_index: None,
            },
            top,
        ),
    ];
    for (quad, tex) in &cube_faces {
        raster_iso_quad(&mut out, &mut depth, quad, tex, [255, 255, 255]);
    }
    Some((out, IsometricIconSource::ManualCubeFallback))
}

fn generate_custom_block_icon(
    block_id: u16,
    texture_cache: &mut HashMap<String, Option<egui::ColorImage>>,
) -> Option<egui::ColorImage> {
    match block_id {
        54 | 130 | 146 => generate_chest_icon(block_id, texture_cache),
        _ => None,
    }
}

fn generate_chest_icon(
    block_id: u16,
    texture_cache: &mut HashMap<String, Option<egui::ColorImage>>,
) -> Option<egui::ColorImage> {
    let texture_name = match block_id {
        130 => "entity/chest/ender.png",
        146 => "entity/chest/trapped.png",
        _ => "entity/chest/normal.png",
    };
    let tex = load_model_texture(texture_name, texture_cache)?;
    let mut out = egui::ColorImage::new([48, 48], vec![egui::Color32::TRANSPARENT; 48 * 48]);
    let mut depth = vec![f32::NEG_INFINITY; out.size[0] * out.size[1]];
    for quad in chest_icon_quads(texture_name) {
        raster_iso_quad(&mut out, &mut depth, &quad, &tex, [255, 255, 255]);
    }
    Some(out)
}

fn chest_icon_quads(texture_path: &str) -> Vec<IconQuad> {
    let mut out = Vec::new();
    push_icon_box_quads(
        &mut out,
        texture_path,
        (64.0, 64.0),
        (0.0, 19.0),
        [1.0, 6.0, 1.0],
        [14.0, 10.0, 14.0],
    );
    push_icon_box_quads(
        &mut out,
        texture_path,
        (64.0, 64.0),
        (0.0, 0.0),
        [1.0, 2.0, 1.0],
        [14.0, 5.0, 14.0],
    );
    push_icon_box_quads(
        &mut out,
        texture_path,
        (64.0, 64.0),
        (0.0, 0.0),
        [7.0, 5.0, 0.0],
        [2.0, 4.0, 1.0],
    );
    out
}

fn push_icon_box_quads(
    out: &mut Vec<IconQuad>,
    texture_path: &str,
    texture_size: (f32, f32),
    texture_offset: (f32, f32),
    box_origin: [f32; 3],
    box_size: [f32; 3],
) {
    let (u, v) = texture_offset;
    let (dx, dy, dz) = (box_size[0], box_size[1], box_size[2]);
    let (x1, y1, z1) = (box_origin[0], box_origin[1], box_origin[2]);
    let (x2, y2, z2) = (x1 + dx, y1 + dy, z1 + dz);
    let (tex_w, tex_h) = texture_size;
    let faces = [
        (
            [[x2, y1, z2], [x2, y1, z1], [x2, y2, z1], [x2, y2, z2]],
            [[u + dz + dx, v + dz], [u + dz + dx + dz, v + dz], [u + dz + dx + dz, v + dz + dy], [u + dz + dx, v + dz + dy]],
        ),
        (
            [[x1, y1, z1], [x1, y1, z2], [x1, y2, z2], [x1, y2, z1]],
            [[u, v + dz], [u + dz, v + dz], [u + dz, v + dz + dy], [u, v + dz + dy]],
        ),
        (
            [[x1, y2, z1], [x2, y2, z1], [x2, y2, z2], [x1, y2, z2]],
            [[u + dz, v], [u + dz + dx, v], [u + dz + dx, v + dz], [u + dz, v + dz]],
        ),
        (
            [[x1, y1, z2], [x2, y1, z2], [x2, y2, z2], [x1, y2, z2]],
            [
                [u + dz + dx + dz, v + dz],
                [u + dz + dx + dz + dx, v + dz],
                [u + dz + dx + dz + dx, v + dz + dy],
                [u + dz + dx + dz, v + dz + dy],
            ],
        ),
        (
            [[x2, y1, z1], [x1, y1, z1], [x1, y2, z1], [x2, y2, z1]],
            [[u + dz, v + dz], [u + dz + dx, v + dz], [u + dz + dx, v + dz + dy], [u + dz, v + dz + dy]],
        ),
    ];

    for (vertices, uv_px) in faces {
        let uv = uv_px.map(|[uu, vv]| [uu / tex_w, vv / tex_h]);
        out.push(icon_quad(vertices, uv, texture_path));
    }
}

fn icon_quad(vertices: [[f32; 3]; 4], uv: [[f32; 2]; 4], texture_path: &str) -> IconQuad {
    IconQuad {
        vertices: vertices.map(|[x, y, z]| [x / 16.0, y / 16.0, z / 16.0]),
        uv,
        texture_path: texture_path.to_string(),
        tint_index: None,
    }
}

fn load_block_texture(
    name: &str,
    cache: &mut HashMap<String, Option<egui::ColorImage>>,
) -> Option<egui::ColorImage> {
    load_model_texture(&format!("blocks/{name}"), cache)
}

fn fallback_block_face_texture(block_id: u16, damage: i16, face: BlockFace) -> Option<String> {
    let meta = damage as u8;
    let color = |m: u8| -> &'static str {
        match m & 0xF {
            0 => "white",
            1 => "orange",
            2 => "magenta",
            3 => "light_blue",
            4 => "yellow",
            5 => "lime",
            6 => "pink",
            7 => "gray",
            8 => "silver",
            9 => "cyan",
            10 => "purple",
            11 => "blue",
            12 => "brown",
            13 => "green",
            14 => "red",
            _ => "black",
        }
    };
    let wood = |m: u8| -> &'static str {
        match m & 0x7 {
            1 => "spruce",
            2 => "birch",
            3 => "jungle",
            4 => "acacia",
            5 => "big_oak",
            _ => "oak",
        }
    };
    match block_id {
        8 | 9 => Some("water_still.png".to_string()),
        10 | 11 => Some("lava_still.png".to_string()),
        23 => Some(match face {
            BlockFace::Up | BlockFace::Down => "furnace_top.png".to_string(),
            BlockFace::South => "dispenser_front_horizontal.png".to_string(),
            _ => "furnace_side.png".to_string(),
        }),
        26 => Some("bed_feet_top.png".to_string()),
        29 => Some(match face {
            BlockFace::Up | BlockFace::Down => "piston_top_sticky.png".to_string(),
            _ => "piston_side.png".to_string(),
        }),
        30 => Some("web.png".to_string()),
        31 => Some(
            if (meta & 0x3) == 2 {
                "fern.png"
            } else {
                "tallgrass.png"
            }
            .to_string(),
        ),
        32 => Some("deadbush.png".to_string()),
        33 => Some(match face {
            BlockFace::Up => "piston_top_normal.png".to_string(),
            BlockFace::Down => "piston_bottom.png".to_string(),
            _ => "piston_side.png".to_string(),
        }),
        34 | 36 => Some("piston_top_normal.png".to_string()),
        37 => Some("flower_dandelion.png".to_string()),
        38 => Some(match meta {
            1 => "flower_blue_orchid.png".to_string(),
            2 => "flower_allium.png".to_string(),
            3 => "flower_houstonia.png".to_string(),
            4 => "flower_tulip_red.png".to_string(),
            5 => "flower_tulip_orange.png".to_string(),
            6 => "flower_tulip_white.png".to_string(),
            7 => "flower_tulip_pink.png".to_string(),
            8 => "flower_oxeye_daisy.png".to_string(),
            _ => "flower_rose.png".to_string(),
        }),
        39 => Some("mushroom_brown.png".to_string()),
        40 => Some("mushroom_red.png".to_string()),
        43 | 44 => Some(match meta & 0x7 {
            1 => "sandstone_normal.png".to_string(),
            2 => "planks_oak.png".to_string(),
            3 => "cobblestone.png".to_string(),
            4 => "brick.png".to_string(),
            5 => "stonebrick.png".to_string(),
            6 => "nether_brick.png".to_string(),
            7 => "quartz_block_side.png".to_string(),
            _ => "stone.png".to_string(),
        }),
        50 | 75 | 76 => Some("torch_on.png".to_string()),
        51 => Some("fire_layer_0.png".to_string()),
        55 => Some("redstone_dust_line0.png".to_string()),
        59 => Some("wheat_stage_0.png".to_string()),
        63 | 68 => Some("planks_oak.png".to_string()),
        69 => Some("lever.png".to_string()),
        74 => Some("redstone_ore.png".to_string()),
        83 => Some("reeds.png".to_string()),
        90 => Some("portal.png".to_string()),
        92 => Some("cake_top.png".to_string()),
        93 | 94 => Some("repeater_off_south.png".to_string()),
        101 => Some("iron_bars.png".to_string()),
        102 => Some("glass.png".to_string()),
        104 => Some("pumpkin_stem_disconnected.png".to_string()),
        105 => Some("melon_stem_disconnected.png".to_string()),
        106 => Some("vine.png".to_string()),
        111 => Some("waterlily.png".to_string()),
        115 => Some("nether_wart_stage_0.png".to_string()),
        117 => Some("brewing_stand_base.png".to_string()),
        118 => Some("cauldron_top.png".to_string()),
        119 => Some("endframe_top.png".to_string()),
        127 => Some("cocoa_stage_0.png".to_string()),
        131 => Some("trip_wire_source.png".to_string()),
        132 => Some("trip_wire.png".to_string()),
        140 => Some("flower_pot.png".to_string()),
        141 => Some("carrots_stage_0.png".to_string()),
        142 => Some("potatoes_stage_0.png".to_string()),
        144 => Some("skeleton_skull.png".to_string()),
        145 => Some("anvil_top_damaged_0.png".to_string()),
        149 | 150 => Some("comparator_off.png".to_string()),
        154 => Some("hopper_top.png".to_string()),
        166 => Some("barrier.png".to_string()),
        175 => Some("double_plant_sunflower_bottom.png".to_string()),
        176 | 177 => Some("wool_colored_white.png".to_string()),
        178 => Some("daylight_detector_top.png".to_string()),
        181 | 182 => Some(match meta & 0x7 {
            1 => "red_sandstone_top.png".to_string(),
            _ => "red_sandstone_normal.png".to_string(),
        }),
        193 => Some("door_spruce_lower.png".to_string()),
        194 => Some("door_birch_lower.png".to_string()),
        195 => Some("door_jungle_lower.png".to_string()),
        196 => Some("door_acacia_lower.png".to_string()),
        197 => Some("door_dark_oak_lower.png".to_string()),
        35 => Some(format!("wool_colored_{}.png", color(meta))),
        95 => Some(format!("glass_{}.png", color(meta))),
        159 => Some(format!("hardened_clay_stained_{}.png", color(meta))),
        160 => Some(format!("glass_{}.png", color(meta))),
        171 => Some(format!("wool_colored_{}.png", color(meta))),
        5 => Some(format!("planks_{}.png", wood(meta))),
        6 => {
            let sap = match meta & 0x7 {
                1 => "sapling_spruce",
                2 => "sapling_birch",
                3 => "sapling_jungle",
                4 => "sapling_acacia",
                5 => "sapling_roofed_oak",
                _ => "sapling_oak",
            };
            Some(format!("{sap}.png"))
        }
        17 => Some(match face {
            BlockFace::Up | BlockFace::Down => match meta & 0x3 {
                1 => "log_spruce_top.png".to_string(),
                2 => "log_birch_top.png".to_string(),
                3 => "log_jungle_top.png".to_string(),
                _ => "log_oak_top.png".to_string(),
            },
            _ => match meta & 0x3 {
                1 => "log_spruce.png".to_string(),
                2 => "log_birch.png".to_string(),
                3 => "log_jungle.png".to_string(),
                _ => "log_oak.png".to_string(),
            },
        }),
        18 => Some(match meta & 0x3 {
            1 => "leaves_spruce.png".to_string(),
            2 => "leaves_birch.png".to_string(),
            3 => "leaves_jungle.png".to_string(),
            _ => "leaves_oak.png".to_string(),
        }),
        161 => Some(match meta & 0x1 {
            1 => "leaves_big_oak.png".to_string(),
            _ => "leaves_acacia.png".to_string(),
        }),
        162 => Some(match face {
            BlockFace::Up | BlockFace::Down => match meta & 0x1 {
                1 => "log_big_oak_top.png".to_string(),
                _ => "log_acacia_top.png".to_string(),
            },
            _ => match meta & 0x1 {
                1 => "log_big_oak.png".to_string(),
                _ => "log_acacia.png".to_string(),
            },
        }),
        _ => None,
    }
}

fn quad_depth(quad: &IconQuad) -> f32 {
    let mut depth = 0.0;
    for v in &quad.vertices {
        depth += v[0] + v[1] + v[2];
    }
    depth / 4.0
}

fn load_model_texture(
    texture_path: &str,
    cache: &mut HashMap<String, Option<egui::ColorImage>>,
) -> Option<egui::ColorImage> {
    if let Some(cached) = cache.get(texture_path) {
        return cached.clone();
    }
    let path = texturepack_textures_root().join(texture_path);
    let image = load_color_image(&path);
    cache.insert(texture_path.to_string(), image.clone());
    image
}

fn raster_iso_quad(
    dst: &mut egui::ColorImage,
    depth: &mut [f32],
    quad: &IconQuad,
    tex: &egui::ColorImage,
    tint: [u8; 3],
) {
    let mut pts = [[0.0f32; 2]; 4];
    let mut z = [0.0f32; 4];
    for (i, v) in quad.vertices.iter().enumerate() {
        let [sx, sy, sz] = project_iso(*v);
        pts[i] = [sx, sy];
        z[i] = sz;
    }
    let shade = face_shade(quad);
    raster_textured_triangle(
        dst, depth, tex, pts[0], pts[1], pts[2], z[0], z[1], z[2], quad.uv[0], quad.uv[1],
        quad.uv[2], shade, tint,
    );
    raster_textured_triangle(
        dst, depth, tex, pts[0], pts[2], pts[3], z[0], z[2], z[3], quad.uv[0], quad.uv[2],
        quad.uv[3], shade, tint,
    );
}

fn project_iso(v: [f32; 3]) -> [f32; 3] {
    let x = v[0] - 0.5;
    let y = v[1] - 0.5;
    let z = v[2] - 0.5;
    let sx = (x - z) * 24.0 + 24.0;
    let sy = ((x + z) * 12.0 - y * 24.0) + 26.0;
    let sz = x + y + z;
    [sx, sy, sz]
}

fn face_shade(quad: &IconQuad) -> f32 {
    let a = quad.vertices[0];
    let b = quad.vertices[1];
    let c = quad.vertices[2];
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if len <= f32::EPSILON {
        return 1.0;
    }
    let n = [n[0] / len, n[1] / len, n[2] / len];
    if n[1].abs() > 0.8 {
        return 1.0;
    }
    if n[0] > 0.35 {
        return 0.82;
    }
    if n[2] > 0.35 {
        return 0.66;
    }
    if n[0] < -0.35 || n[2] < -0.35 {
        return 0.58;
    }
    0.72
}

fn raster_textured_triangle(
    dst: &mut egui::ColorImage,
    depth: &mut [f32],
    tex: &egui::ColorImage,
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    z0: f32,
    z1: f32,
    z2: f32,
    uv0: [f32; 2],
    uv1: [f32; 2],
    uv2: [f32; 2],
    shade: f32,
    tint: [u8; 3],
) {
    let min_x = p0[0].min(p1[0]).min(p2[0]).floor().max(0.0) as i32;
    let max_x = p0[0]
        .max(p1[0])
        .max(p2[0])
        .ceil()
        .min((dst.size[0] - 1) as f32) as i32;
    let min_y = p0[1].min(p1[1]).min(p2[1]).floor().max(0.0) as i32;
    let max_y = p0[1]
        .max(p1[1])
        .max(p2[1])
        .ceil()
        .min((dst.size[1] - 1) as f32) as i32;
    let area = edge_fn(p0, p1, p2);
    if area.abs() < 1e-5 {
        return;
    }

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            let w0 = edge_fn(p1, p2, p) / area;
            let w1 = edge_fn(p2, p0, p) / area;
            let w2 = edge_fn(p0, p1, p) / area;
            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                continue;
            }
            let z = w0 * z0 + w1 * z1 + w2 * z2;
            let idx = y as usize * dst.size[0] + x as usize;
            if z <= depth[idx] {
                continue;
            }
            let u = w0 * uv0[0] + w1 * uv1[0] + w2 * uv2[0];
            let v = w0 * uv0[1] + w1 * uv1[1] + w2 * uv2[1];
            let tx = (u.clamp(0.0, 1.0) * (tex.size[0] as f32 - 1.0)).round() as usize;
            let ty = (v.clamp(0.0, 1.0) * (tex.size[1] as f32 - 1.0)).round() as usize;
            let mut c = tex.pixels[ty * tex.size[0] + tx];
            if c.a() == 0 {
                continue;
            }
            c = egui::Color32::from_rgba_unmultiplied(
                (f32::from(c.r()) * shade * (f32::from(tint[0]) / 255.0)).clamp(0.0, 255.0) as u8,
                (f32::from(c.g()) * shade * (f32::from(tint[1]) / 255.0)).clamp(0.0, 255.0) as u8,
                (f32::from(c.b()) * shade * (f32::from(tint[2]) / 255.0)).clamp(0.0, 255.0) as u8,
                c.a(),
            );
            dst.pixels[idx] = c;
            depth[idx] = z;
        }
    }
}

fn icon_tint_color(block_id: u16, damage: i16) -> Option<[u8; 3]> {
    // Deterministic inventory/debug tints approximating vanilla biome coloring.
    // This avoids grayscale foliage/grass in icon views.
    let meta = damage as u8;
    match block_id {
        // Grass family
        2 | 31 | 59 | 83 => Some([0x7f, 0xb2, 0x38]),
        // Vines / foliage family
        106 | 111 | 161 => Some([0x48, 0xb5, 0x18]),
        18 => Some(match meta & 0x3 {
            1 => [0x61, 0x99, 0x61], // spruce
            2 => [0x80, 0xA7, 0x55], // birch
            _ => [0x48, 0xB5, 0x18], // oak/jungle
        }),
        175 => match meta & 0x7 {
            2 | 3 => Some([0x7f, 0xb2, 0x38]), // double grass / fern
            _ => None,
        },
        // Water-like tint
        8 | 9 => Some([0x3f, 0x76, 0xe4]),
        _ => None,
    }
}

fn edge_fn(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (c[0] - a[0]) * (b[1] - a[1]) - (c[1] - a[1]) * (b[0] - a[0])
}

fn texturepack_textures_root() -> PathBuf {
    rs_utils::texturepack_textures_root()
}

fn load_color_image(path: &Path) -> Option<egui::ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let mut rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    // For animated texture sheets (e.g. frame stacks), use first frame only.
    // Vanilla atlas animation advances frames over time; debug icons should not stretch the full sheet.
    if rgba.height() > rgba.width() && rgba.height() % rgba.width() == 0 {
        let w = rgba.width();
        let frame = image::imageops::crop(&mut rgba, 0, 0, w, w).to_image();
        rgba = frame;
    }
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}
