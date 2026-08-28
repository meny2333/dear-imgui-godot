use std::collections::HashMap;

use godot::classes::image::Format;
use godot::classes::{FileAccess, Image, ImageTexture, ProjectSettings, Texture2D};
use godot::prelude::*;

use imgui::{Context, FontConfig, FontGlyphRanges, FontSource, TextureId};

const DEFAULT_FONT_SIZE: f32 = 13.0;
const FONT_PATH_SETTING: &str = "imgui/font_path";
const FONT_SIZE_SETTING: &str = "imgui/font_size";

pub struct TextureRegistry {
    map: HashMap<usize, Gd<Texture2D>>,
    next: usize,
}

impl TextureRegistry {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            next: 1,
        }
    }

    pub fn register(&mut self, tex: Gd<Texture2D>) -> usize {
        let id = self.next;
        self.next += 1;
        self.map.insert(id, tex);
        id
    }

    pub fn lookup(&self, id: usize) -> Option<Rid> {
        self.map.get(&id).map(|t| t.get_rid())
    }

    pub fn remove(&mut self, id: usize) {
        self.map.remove(&id);
    }
}

fn configured_font(scale: f32) -> Option<(Vec<u8>, f32)> {
    let settings = ProjectSettings::singleton();
    let path = settings
        .get_setting(FONT_PATH_SETTING)
        .try_to::<GString>()
        .ok()?
        .to_string();
    if path.is_empty() {
        return None;
    }

    let size = settings
        .get_setting(FONT_SIZE_SETTING)
        .try_to::<f64>()
        .ok()
        .map(|value| value as f32)
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(DEFAULT_FONT_SIZE);

    let data = FileAccess::get_file_as_bytes(path.as_str());
    if data.is_empty() {
        godot_warn!("dear-imgui-godot: unable to load configured font: {path}");
        return None;
    }

    Some((data.to_vec(), size * scale))
}

/// Build the font atlas at the given UI scale and register its texture, returning
/// the new texture id. A configured TTF/OTF font uses the common simplified
/// Chinese glyph range so CJK text does not overflow the atlas on mobile. When
/// rebuilding at runtime, pass the previous id as old_id so its texture can be
/// released.
pub fn build_font_atlas(
    ctx: &mut Context,
    textures: &mut TextureRegistry,
    scale: f32,
    old_id: usize,
) -> usize {
    let atlas = ctx.fonts();
    atlas.clear();
    if let Some((data, size_pixels)) = configured_font(scale) {
        atlas.add_font(&[FontSource::TtfData {
            data: &data,
            size_pixels,
            config: Some(FontConfig {
                size_pixels,
                oversample_h: 1,
                oversample_v: 1,
                pixel_snap_h: true,
                glyph_ranges: FontGlyphRanges::chinese_simplified_common(),
                ..Default::default()
            }),
        }]);
    } else {
        atlas.add_font(&[FontSource::DefaultFontData {
            config: Some(FontConfig {
                size_pixels: DEFAULT_FONT_SIZE * scale,
                oversample_h: 1,
                oversample_v: 1,
                pixel_snap_h: true,
                ..Default::default()
            }),
        }]);
    }

    let (width, height, data) = {
        let tex = atlas.build_rgba32_texture();
        (
            tex.width as i32,
            tex.height as i32,
            PackedByteArray::from(tex.data),
        )
    };

    let image = Image::create_from_data(width, height, false, Format::RGBA8, &data)
        .expect("imgui font atlas image");
    let texture = ImageTexture::create_from_image(&image).expect("imgui font atlas texture");

    let id = textures.register(texture.upcast::<Texture2D>());
    atlas.tex_id = TextureId::from(id);

    if old_id != 0 {
        textures.remove(old_id);
    }
    id
}
