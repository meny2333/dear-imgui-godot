use godot::classes::RenderingServer;
use godot::prelude::*;

use imgui::{DrawCmd, DrawCmdParams, DrawData};

use crate::fonts::TextureRegistry;

/// Slack, in pixels, when testing whether a command's geometry fits inside its clip
/// rect. Covers the sub-pixel anti-aliasing fringe so an edge-touching widget is not
/// treated as overflowing (which would needlessly send it through CPU clipping).
const CLIP_FIT_EPS: f32 = 1.0;

pub struct CanvasRenderer {
    canvas: Rid,
    item: Rid,
    viewport: Rid,
    layer: i32,
}

impl CanvasRenderer {
    pub fn new() -> Self {
        Self {
            canvas: Rid::Invalid,
            item: Rid::Invalid,
            viewport: Rid::Invalid,
            layer: crate::backend::DEFAULT_RENDER_LAYER,
        }
    }

    pub fn init(&mut self, viewport: Rid) {
        let mut rs = RenderingServer::singleton();
        self.viewport = viewport;
        self.canvas = rs.canvas_create();
        self.item = rs.canvas_item_create();
        self.layer = crate::backend::desired_render_layer();
        rs.viewport_attach_canvas(viewport, self.canvas);
        rs.viewport_set_canvas_stacking(viewport, self.canvas, self.layer, 0);
        rs.canvas_item_set_parent(self.item, self.canvas);
    }

    /// Restack the canvas onto `layer` when it differs from the layer in use.
    pub fn sync_layer(&mut self, layer: i32) {
        if layer == self.layer || self.canvas == Rid::Invalid {
            return;
        }
        self.layer = layer;
        RenderingServer::singleton().viewport_set_canvas_stacking(self.viewport, self.canvas, layer, 0);
    }

    pub fn render(&mut self, draw_data: &DrawData, textures: &TextureRegistry) {
        let mut rs = RenderingServer::singleton();

        // Everything is drawn onto a SINGLE canvas item. Godot's GL Compatibility 2D
        // batcher blacks out the viewport once a couple dozen sibling canvas items exist
        // (reached quickly with per-command items once several windows or stacked popups
        // are on screen), so per-command items are not an option. Clipping, which Godot
        // only supports per canvas item, is therefore done on the CPU: a command whose
        // geometry already fits its clip rect is added as-is, and one that spills past it
        // has its triangles clipped to the rect before being added. Adding the commands in
        // order keeps the painter's order intact.
        let item = self.item;
        rs.canvas_item_clear(item);

        for draw_list in draw_data.draw_lists() {
            let vtx = draw_list.vtx_buffer();
            let idx = draw_list.idx_buffer();

            let mut pv: Vec<Vector2> = Vec::with_capacity(vtx.len());
            let mut uv: Vec<Vector2> = Vec::with_capacity(vtx.len());
            let mut cv: Vec<Color> = Vec::with_capacity(vtx.len());
            for v in vtx {
                pv.push(Vector2::new(v.pos[0], v.pos[1]));
                uv.push(Vector2::new(v.uv[0], v.uv[1]));
                cv.push(Color::from_rgba(
                    v.col[0] as f32 / 255.0,
                    v.col[1] as f32 / 255.0,
                    v.col[2] as f32 / 255.0,
                    v.col[3] as f32 / 255.0,
                ));
            }

            for cmd in draw_list.commands() {
                let DrawCmd::Elements { count, cmd_params } = cmd else {
                    continue;
                };
                if count == 0 {
                    continue;
                }
                let DrawCmdParams {
                    clip_rect,
                    texture_id,
                    vtx_offset,
                    idx_offset,
                } = cmd_params;

                // `idx` values are 16-bit and relative to `vtx_offset`, so slice the vertex
                // arrays to that window and keep the indices 0-based. Passing absolute indices
                // instead would exceed 65,535 once a draw list grows past 64k vertices.
                let base = vtx_offset;
                let end = (base + u16::MAX as usize + 1).min(pv.len());

                // Build the index list and the geometry bounds in one pass.
                let mut iv: Vec<i32> = Vec::with_capacity(count);
                let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
                let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
                for k in 0..count {
                    let li = idx[idx_offset + k] as usize;
                    iv.push(li as i32);
                    let p = pv[base + li];
                    min_x = min_x.min(p.x);
                    min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x);
                    max_y = max_y.max(p.y);
                }

                let tex = textures.lookup(texture_id.id()).unwrap_or(Rid::Invalid);

                let fits = min_x >= clip_rect[0] - CLIP_FIT_EPS
                    && min_y >= clip_rect[1] - CLIP_FIT_EPS
                    && max_x <= clip_rect[2] + CLIP_FIT_EPS
                    && max_y <= clip_rect[3] + CLIP_FIT_EPS;

                if fits {
                    let points = PackedVector2Array::from(&pv[base..end]);
                    let uvs = PackedVector2Array::from(&uv[base..end]);
                    let colors = PackedColorArray::from(&cv[base..end]);
                    let indices = PackedInt32Array::from(iv.as_slice());
                    rs.canvas_item_add_triangle_array_ex(item, &indices, &points, &colors)
                        .uvs(&uvs)
                        .texture(tex)
                        .done();
                    continue;
                }

                // Overflows the clip rect: clip each triangle to it on the CPU.
                let mut cp: Vec<Vector2> = Vec::new();
                let mut cu: Vec<Vector2> = Vec::new();
                let mut cc: Vec<Color> = Vec::new();
                let mut ci: Vec<i32> = Vec::new();
                for t in 0..count / 3 {
                    let tri = [0usize, 1, 2].map(|j| {
                        let li = idx[idx_offset + t * 3 + j] as usize + base;
                        ClipVert { pos: pv[li], uv: uv[li], col: cv[li] }
                    });
                    clip_triangle(tri, clip_rect, &mut cp, &mut cu, &mut cc, &mut ci);
                }
                if ci.is_empty() {
                    continue;
                }
                let points = PackedVector2Array::from(cp.as_slice());
                let uvs = PackedVector2Array::from(cu.as_slice());
                let colors = PackedColorArray::from(cc.as_slice());
                let indices = PackedInt32Array::from(ci.as_slice());
                rs.canvas_item_add_triangle_array_ex(item, &indices, &points, &colors)
                    .uvs(&uvs)
                    .texture(tex)
                    .done();
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ClipVert {
    pos: Vector2,
    uv: Vector2,
    col: Color,
}

fn lerp_vert(a: ClipVert, b: ClipVert, t: f32) -> ClipVert {
    let l = |x: f32, y: f32| x + (y - x) * t;
    ClipVert {
        pos: a.pos + (b.pos - a.pos) * t,
        uv: a.uv + (b.uv - a.uv) * t,
        col: Color::from_rgba(
            l(a.col.r, b.col.r),
            l(a.col.g, b.col.g),
            l(a.col.b, b.col.b),
            l(a.col.a, b.col.a),
        ),
    }
}

/// Clip a triangle to an axis-aligned rect (`[x0, y0, x1, y1]`) with Sutherland-Hodgman,
/// interpolating uv and color at the cuts, and append the resulting fan to the output.
fn clip_triangle(
    tri: [ClipVert; 3],
    rect: [f32; 4],
    out_pos: &mut Vec<Vector2>,
    out_uv: &mut Vec<Vector2>,
    out_col: &mut Vec<Color>,
    out_idx: &mut Vec<i32>,
) {
    // (keep-greater, use-x, bound) for the left, right, top and bottom edges.
    let edges = [
        (true, true, rect[0]),
        (false, true, rect[2]),
        (true, false, rect[1]),
        (false, false, rect[3]),
    ];
    let mut poly: Vec<ClipVert> = tri.to_vec();
    for (keep_greater, use_x, bound) in edges {
        if poly.len() < 3 {
            return;
        }
        let coord = |v: &ClipVert| if use_x { v.pos.x } else { v.pos.y };
        let inside = |v: &ClipVert| {
            if keep_greater {
                coord(v) >= bound
            } else {
                coord(v) <= bound
            }
        };
        let mut next: Vec<ClipVert> = Vec::with_capacity(poly.len() + 1);
        for i in 0..poly.len() {
            let a = poly[i];
            let b = poly[(i + 1) % poly.len()];
            let a_in = inside(&a);
            if a_in {
                next.push(a);
            }
            if a_in != inside(&b) {
                let t = (bound - coord(&a)) / (coord(&b) - coord(&a));
                next.push(lerp_vert(a, b, t));
            }
        }
        poly = next;
    }
    if poly.len() < 3 {
        return;
    }
    let b = out_pos.len() as i32;
    for v in &poly {
        out_pos.push(v.pos);
        out_uv.push(v.uv);
        out_col.push(v.col);
    }
    for i in 1..poly.len() as i32 - 1 {
        out_idx.push(b);
        out_idx.push(b + i);
        out_idx.push(b + i + 1);
    }
}

impl Drop for CanvasRenderer {
    fn drop(&mut self) {
        let mut rs = RenderingServer::singleton();
        if self.item != Rid::Invalid {
            rs.free_rid(self.item);
        }
        if self.canvas != Rid::Invalid {
            rs.free_rid(self.canvas);
        }
    }
}
