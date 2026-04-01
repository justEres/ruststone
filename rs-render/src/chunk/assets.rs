use super::*;

impl FromWorld for ChunkRenderAssets {
    fn from_world(world: &mut World) -> Self {
        let settings = world
            .get_resource::<crate::debug::RenderDebugSettings>()
            .cloned()
            .unwrap_or_default();
        let (mut atlas_image, texture_mapping, biome_tints) = load_or_build_atlas();
        let mut sampler = ImageSamplerDescriptor::nearest();
        sampler.address_mode_u = ImageAddressMode::ClampToEdge;
        sampler.address_mode_v = ImageAddressMode::ClampToEdge;
        sampler.address_mode_w = ImageAddressMode::ClampToEdge;
        sampler.mipmap_filter = bevy::image::ImageFilterMode::Nearest;
        sampler.lod_min_clamp = 0.0;
        sampler.lod_max_clamp = 0.0;
        atlas_image.sampler = ImageSampler::Descriptor(sampler);
        let atlas_handle = {
            let mut images = world.resource_mut::<Assets<Image>>();
            images.add(atlas_image)
        };
        let skybox_texture = world.resource::<AssetServer>().load("skybox.ktx2");
        let mut materials = world.resource_mut::<Assets<ChunkAtlasMaterial>>();
        let use_shadowed_pbr = uses_shadowed_pbr_path(&settings);
        let cutout_alpha_mode = AlphaMode::Mask(0.5);
        let grass_side_origin = texture_mapping
            .texture_index_by_name("grass_side.png")
            .map(atlas_tile_origin)
            .unwrap_or([f32::NAN, f32::NAN]);
        let grass_overlay_origin = texture_mapping
            .texture_index_by_name("grass_side_overlay.png")
            .map(atlas_tile_origin)
            .unwrap_or([f32::NAN, f32::NAN]);
        let grass_overlay_info = Vec4::new(
            grass_side_origin[0],
            grass_side_origin[1],
            grass_overlay_origin[0],
            grass_overlay_origin[1],
        );

        let make_lighting = |pass_mode: f32| {
            let mut u = lighting_uniform_for_mode(&settings, None, pass_mode);
            u.grass_overlay_info = grass_overlay_info;
            u
        };

        let opaque_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: !use_shadowed_pbr,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(0.0),
            },
        });
        let transparent_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.8),
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: !use_shadowed_pbr,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(1.0),
            },
        });
        let cutout_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                alpha_mode: cutout_alpha_mode,
                cull_mode: None,
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: false,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(2.0),
            },
        });
        let cutout_culled_material = materials.add(ChunkAtlasMaterial {
            base: StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: None,
                metallic: 0.0,
                reflectance: 0.0,
                perceptual_roughness: 1.0,
                alpha_mode: cutout_alpha_mode,
                cull_mode: Some(bevy::render::render_resource::Face::Back),
                opaque_render_method: OpaqueRendererMethod::Forward,
                unlit: false,
                ..default()
            },
            extension: AtlasTextureExtension {
                atlas: atlas_handle.clone(),
                skybox: skybox_texture.clone(),
                lighting: make_lighting(2.0),
            },
        });

        Self {
            opaque_material,
            cutout_material,
            cutout_culled_material,
            transparent_material,
            atlas: atlas_handle,
            skybox_texture,
            texture_mapping,
            biome_tints,
            grass_overlay_info,
        }
    }
}

pub(super) fn assets_root() -> PathBuf {
    ruststone_assets_root()
}

fn texture_root_path() -> PathBuf {
    assets_root().join(TEXTURE_BASE)
}

fn load_or_build_atlas() -> (Image, Arc<AtlasBlockMapping>, Arc<BiomeTintResolver>) {
    let textures_root = texture_root_path();
    let extra_sources = extra_texture_sources();
    let mut texture_names = collect_texture_names(&textures_root);
    texture_names.extend(extra_sources.keys().cloned());
    texture_names.extend(PLAYER_HEAD_TEXTURES.iter().map(|s| (*s).to_string()));
    texture_names.sort();
    texture_names.dedup();
    if texture_names.is_empty() {
        texture_names.push("missing_texture.png".to_string());
    } else if !texture_names.iter().any(|name| name == "missing_texture.png") {
        texture_names.insert(0, "missing_texture.png".to_string());
    }
    if texture_names.len() > ATLAS_TILE_CAPACITY {
        warn!(
            "Block texture atlas overflow: {} textures but capacity is {}",
            texture_names.len(),
            ATLAS_TILE_CAPACITY
        );
        texture_names.truncate(ATLAS_TILE_CAPACITY);
    }

    let mut name_to_index = HashMap::with_capacity(texture_names.len());
    for (idx, name) in texture_names.iter().enumerate() {
        name_to_index.insert(name.clone(), idx as u16);
    }

    let mut tile_size = None;
    let mut atlas = None::<ImageBuffer<Rgba<u8>, Vec<u8>>>;

    for (idx, texture_name) in texture_names.iter().enumerate() {
        let img = load_texture_image(&textures_root, texture_name, &extra_sources)
            .unwrap_or_else(missing_texture_image);
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let size = tile_size.get_or_insert((w, h));
        let (tile_w, tile_h) = *size;
        let rgba = if w != tile_w || h != tile_h {
            imageops::resize(&rgba, tile_w, tile_h, imageops::Nearest)
        } else {
            rgba
        };
        let mut rgba = rgba;
        apply_default_foliage_tint(texture_name, &mut rgba);
        normalize_overlay_mask_texture(texture_name, &mut rgba);
        force_opaque_texture_alpha(texture_name, &mut rgba);

        let atlas_buf = atlas.get_or_insert_with(|| {
            ImageBuffer::from_pixel(
                tile_w * ATLAS_COLUMNS,
                tile_h * ATLAS_ROWS,
                Rgba([0, 0, 0, 0]),
            )
        });
        let col = (idx as u32) % ATLAS_COLUMNS;
        let row = (idx as u32) / ATLAS_COLUMNS;
        let x = col * tile_w;
        let y = row * tile_h;
        imageops::replace(atlas_buf, &rgba, x.into(), y.into());
    }

    let atlas = atlas.unwrap_or_else(|| {
        ImageBuffer::from_pixel(ATLAS_COLUMNS * 16, ATLAS_ROWS * 16, Rgba([0, 0, 0, 0]))
    });
    dump_atlas_debug_images(&atlas);
    let mut model_resolver = BlockModelResolver::new(default_model_roots());
    let mapping = Arc::new(build_block_texture_mapping(
        &name_to_index,
        Some(&mut model_resolver),
    ));
    let biome_tints = Arc::new(BiomeTintResolver::load(&texturepack_minecraft_root()));
    (
        bevy_image_from_rgba(DynamicImage::ImageRgba8(atlas)),
        mapping,
        biome_tints,
    )
}

const PLAYER_HEAD_TEXTURES: [&str; 6] = [
    "head_player_top.png",
    "head_player_bottom.png",
    "head_player_front.png",
    "head_player_back.png",
    "head_player_left.png",
    "head_player_right.png",
];

fn extra_texture_sources() -> HashMap<String, PathBuf> {
    let root = texturepack_minecraft_root().join("textures");
    [
        ("barrier_item.png", "items/barrier.png"),
        ("chest_normal.png", "entity/chest/normal.png"),
        ("chest_trapped.png", "entity/chest/trapped.png"),
        ("chest_ender.png", "entity/chest/ender.png"),
        ("sign_entity.png", "entity/sign.png"),
    ]
    .into_iter()
    .map(|(name, rel)| (name.to_string(), root.join(rel)))
    .collect()
}

fn load_texture_image(
    textures_root: &std::path::Path,
    texture_name: &str,
    extra_sources: &HashMap<String, PathBuf>,
) -> Option<DynamicImage> {
    if let Some(path) = extra_sources.get(texture_name)
        && let Ok(img) = image::open(path)
    {
        return Some(img);
    }

    let direct_path = textures_root.join(texture_name);
    if let Ok(img) = image::open(&direct_path) {
        return Some(img);
    }

    if let Some(img) = player_head_face_texture(texture_name) {
        return Some(img);
    }

    None
}

fn player_head_face_texture(texture_name: &str) -> Option<DynamicImage> {
    let (x, y) = match texture_name {
        "head_player_right.png" => (0, 8),
        "head_player_front.png" => (8, 8),
        "head_player_left.png" => (16, 8),
        "head_player_back.png" => (24, 8),
        "head_player_top.png" => (8, 0),
        "head_player_bottom.png" => (16, 0),
        _ => return None,
    };

    let steve = texturepack_minecraft_root().join("textures/entity/steve.png");
    let img = image::open(steve).ok()?.to_rgba8();
    if img.width() < x + 8 || img.height() < y + 8 {
        return None;
    }
    let cropped = imageops::crop_imm(&img, x, y, 8, 8).to_image();
    Some(DynamicImage::ImageRgba8(cropped))
}

fn collect_texture_names(textures_root: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(textures_root) else {
        return out;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
            && let Some(name) = path.file_name().and_then(|s| s.to_str())
        {
            out.push(name.to_string());
        }
    }
    out
}

fn bevy_image_from_rgba(img: DynamicImage) -> Image {
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let data = rgba.into_raw();
    let mut image = Image::new_fill(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.data = Some(data);
    image
}

fn dump_atlas_debug_images(atlas: &ImageBuffer<Rgba<u8>, Vec<u8>>) {
    let out_dir = assets_root().join("debug");
    if fs::create_dir_all(&out_dir).is_err() {
        return;
    }

    let atlas_path = out_dir.join("atlas_dump.png");
    let _ = DynamicImage::ImageRgba8(atlas.clone()).save(&atlas_path);

    let mut alpha_img =
        ImageBuffer::from_pixel(atlas.width(), atlas.height(), Rgba([0, 0, 0, 255]));
    for (src, dst) in atlas.pixels().zip(alpha_img.pixels_mut()) {
        let a = src.0[3];
        *dst = Rgba([a, a, a, 255]);
    }
    let alpha_path = out_dir.join("atlas_alpha.png");
    let _ = DynamicImage::ImageRgba8(alpha_img).save(&alpha_path);
}

fn missing_texture_image() -> DynamicImage {
    let mut img = ImageBuffer::from_pixel(16, 16, Rgba([255, 0, 255, 255]));
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        if (x + y) % 2 == 0 {
            *pixel = Rgba([0, 0, 0, 255]);
        }
    }
    DynamicImage::ImageRgba8(img)
}

pub fn apply_mesh_data(mesh: &mut Mesh, data: MeshData) {
    let MeshData {
        positions,
        normals,
        mut uvs,
        uvs_b,
        colors,
        indices,
        ..
    } = data;

    for (uv, tile_origin) in uvs.iter_mut().zip(uvs_b.iter()) {
        let col = (tile_origin[0] * ATLAS_COLUMNS as f32).round();
        let row = (tile_origin[1] * ATLAS_ROWS as f32).round();
        uv[0] += col * ATLAS_UV_PACK_SCALE;
        uv[1] += row * ATLAS_UV_PACK_SCALE;
    }

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs_b);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}

pub fn build_mesh_from_data(data: MeshData) -> (Mesh, Option<(Vec3, Vec3)>) {
    let bounds = data.bounds();
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    apply_mesh_data(&mut mesh, data);
    (mesh, bounds)
}
