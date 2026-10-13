use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, Stroke, Vec2};
use tt_core::vcam::scene::{LayerId, Scene, Transform};
use crate::ui::theme::Palette;

const HANDLE: f32 = 8.0;
const ROTATE_OFFSET: f32 = 26.0;
const SNAP_PX: f32 = 8.0;
const MIN_SIZE: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Grip {
    Move,
    /// Which edges move: -1 left/top, 1 right/bottom, 0 neither.
    Resize(i8, i8),
    Rotate,
}

struct Drag {
    layer: LayerId,
    grip: Grip,
    from: (f32, f32),
    start: Transform,
}

#[derive(Default)]
pub struct CanvasState {
    drag: Option<Drag>,
    guides: Vec<Guide>,
}

#[derive(Clone, Copy)]
enum Guide {
    Vertical(f32),
    Horizontal(f32),
}

/// Scene pixels ↔ screen points for a scene letterboxed into a rect.
#[derive(Clone, Copy)]
pub struct View {
    origin: Pos2,
    scale: f32,
}

impl View {
    pub fn fit(rect: Rect, scene: &Scene) -> (Self, Rect) {
        let size = Vec2::new(scene.width.max(1) as f32, scene.height.max(1) as f32);
        let scale = (rect.width() / size.x).min(rect.height() / size.y).max(0.01);
        let shown = Rect::from_center_size(rect.center(), size * scale);
        (Self { origin: shown.min, scale }, shown)
    }

    pub fn to_screen(self, p: (f32, f32)) -> Pos2 {
        self.origin + Vec2::new(p.0, p.1) * self.scale
    }

    pub fn to_scene(self, p: Pos2) -> (f32, f32) {
        let v = (p - self.origin) / self.scale;
        (v.x, v.y)
    }
}

fn handles(t: &Transform) -> Vec<(Grip, (f32, f32))> {
    let (hw, hh) = (t.w / 2.0, t.h / 2.0);
    let mut out = Vec::new();
    for sy in [-1i8, 0, 1] {
        for sx in [-1i8, 0, 1] {
            if sx != 0 || sy != 0 {
                out.push((Grip::Resize(sx, sy), t.from_local((f32::from(sx) * hw, f32::from(sy) * hh))));
            }
        }
    }
    out
}

fn rotate_handle(t: &Transform, view: &View) -> (f32, f32) {
    t.from_local((0.0, -t.h / 2.0 - ROTATE_OFFSET / view.scale))
}

fn grip_at(t: &Transform, view: &View, pointer: Pos2) -> Option<Grip> {
    let near = |p: (f32, f32)| view.to_screen(p).distance(pointer) <= HANDLE;
    if near(rotate_handle(t, view)) {
        return Some(Grip::Rotate);
    }
    handles(t).into_iter().find(|(_, p)| near(*p)).map(|(grip, _)| grip)
}

fn cursor_for(grip: Grip, rotation: f32) -> CursorIcon {
    let (sx, sy) = match grip {
        Grip::Move => return CursorIcon::Move,
        Grip::Rotate => return CursorIcon::Grab,
        Grip::Resize(sx, sy) => (f32::from(sx), f32::from(sy)),
    };
    // Pick the resize cursor closest to the handle's on-screen direction.
    let angle = (sy.atan2(sx).to_degrees() + rotation).rem_euclid(180.0);
    match angle {
        a if !(22.5..157.5).contains(&a) => CursorIcon::ResizeHorizontal,
        a if a < 67.5 => CursorIcon::ResizeNwSe,
        a if a < 112.5 => CursorIcon::ResizeVertical,
        _ => CursorIcon::ResizeNeSw,
    }
}

pub struct CanvasOutput {
    pub view: View,
    pub rect: Rect,
    pub response: egui::Response,
}

/// Draws the preview with selection handles and applies pointer edits to `scene`.
pub fn show(
    ui: &mut egui::Ui,
    state: &mut CanvasState,
    scene: &mut Scene,
    selected: &mut Option<LayerId>,
    preview: Option<&egui::TextureHandle>,
) -> CanvasOutput {
    let available = ui.available_rect_before_wrap();
    let (view, rect) = View::fit(available.shrink(4.0), scene);
    let response = ui.allocate_rect(available, egui::Sense::click_and_drag());
    let palette = Palette::of(ui.visuals());
    let painter = ui.painter_at(available);
    painter.rect_filled(rect, 0, Color32::BLACK);
    match preview {
        Some(texture) => {
            painter.image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, "Starting preview…", egui::FontId::proportional(14.0), palette.text_weak);
        }
    }

    let pointer = response.hover_pos();
    let modifiers = ui.input(|i| i.modifiers);
    let selected_transform = selected.and_then(|id| scene.layer(id)).filter(|l| !l.locked).map(|l| l.transform);

    if let Some(pointer) = pointer.filter(|_| state.drag.is_none()) {
        let grip = selected_transform.and_then(|t| grip_at(&t, &view, pointer));
        let hovered = scene.hit(view.to_scene(pointer));
        match (grip, selected_transform) {
            (Some(grip), Some(t)) => ui.ctx().set_cursor_icon(cursor_for(grip, t.rotation)),
            _ if hovered.is_some() => ui.ctx().set_cursor_icon(CursorIcon::Move),
            _ => {}
        }
        if let Some(layer) = hovered.and_then(|id| scene.layer(id)).filter(|_| hovered != *selected) {
            outline(&painter, &view, &layer.transform, Stroke::new(1.0, palette.accent.gamma_multiply(0.6)));
        }
    }

    // egui reports a drag only once the pointer has moved a little; grab what was under the press itself.
    if response.drag_started()
        && let Some(press) = ui.input(|i| i.pointer.press_origin())
    {
        let grip = selected_transform.and_then(|t| grip_at(&t, &view, press));
        let target = match grip {
            Some(grip) => selected.map(|id| (id, grip)),
            None => scene.hit(view.to_scene(press)).map(|id| (id, Grip::Move)),
        };
        *selected = target.map(|(id, _)| id);
        state.drag = target.and_then(|(id, grip)| {
            let start = scene.layer(id)?.transform;
            Some(Drag { layer: id, grip, from: view.to_scene(press), start })
        });
    }

    state.guides.clear();
    if let (Some(drag), Some(pointer)) = (&state.drag, response.interact_pointer_pos())
        && response.dragged()
    {
        let to = view.to_scene(pointer);
        let snap = !modifiers.ctrl;
        let others: Vec<Transform> = scene.layers.iter().filter(|l| l.id != drag.layer && l.visible).map(|l| l.transform).collect();
        let canvas = (scene.width as f32, scene.height as f32);
        let threshold = SNAP_PX / view.scale;
        let updated = match drag.grip {
            Grip::Move => {
                let mut t = drag.start;
                t.x += to.0 - drag.from.0;
                t.y += to.1 - drag.from.1;
                if snap {
                    state.guides = snap_move(&mut t, &others, canvas, threshold);
                }
                t
            }
            Grip::Resize(sx, sy) => resize(&drag.start, (sx, sy), to, !modifiers.shift, modifiers.alt),
            Grip::Rotate => {
                let (cx, cy) = drag.start.center();
                let angle = |p: (f32, f32)| (p.1 - cy).atan2(p.0 - cx).to_degrees();
                let mut rotation = drag.start.rotation + angle(to) - angle(drag.from);
                if modifiers.shift {
                    rotation = (rotation / 15.0).round() * 15.0;
                }
                Transform { rotation: normalize_degrees(rotation), ..drag.start }
            }
        };
        if let Some(layer) = scene.layer_mut(drag.layer) {
            layer.transform = updated;
        }
    }
    if response.drag_stopped() {
        state.drag = None;
    }
    if response.clicked()
        && let Some(press) = response.interact_pointer_pos()
    {
        let on_grip = selected_transform.is_some_and(|t| grip_at(&t, &view, press).is_some());
        if !on_grip {
            *selected = scene.hit(view.to_scene(press));
        }
    }
    if response.clicked() || response.drag_started() {
        response.request_focus();
    }

    for guide in &state.guides {
        let stroke = Stroke::new(1.0, Color32::from_rgb(236, 72, 153));
        match *guide {
            Guide::Vertical(x) => {
                painter.line_segment([view.to_screen((x, 0.0)), view.to_screen((x, scene.height as f32))], stroke);
            }
            Guide::Horizontal(y) => {
                painter.line_segment([view.to_screen((0.0, y)), view.to_screen((scene.width as f32, y))], stroke);
            }
        }
    }
    if let Some(layer) = selected.and_then(|id| scene.layer(id)) {
        let t = layer.transform;
        let stroke = Stroke::new(1.5, palette.accent);
        outline(&painter, &view, &t, stroke);
        if !layer.locked {
            let top = view.to_screen(t.from_local((0.0, -t.h / 2.0)));
            let knob = view.to_screen(rotate_handle(&t, &view));
            painter.line_segment([top, knob], stroke);
            painter.circle(knob, HANDLE / 2.0 + 1.0, palette.panel, stroke);
            for (_, p) in handles(&t) {
                let r = Rect::from_center_size(view.to_screen(p), Vec2::splat(HANDLE));
                painter.rect(r, 1, Color32::WHITE, stroke, egui::StrokeKind::Middle);
            }
        }
    }
    CanvasOutput { view, rect, response }
}

fn outline(painter: &egui::Painter, view: &View, t: &Transform, stroke: Stroke) {
    let points: Vec<Pos2> = t.corners().iter().map(|c| view.to_screen(*c)).collect();
    painter.add(egui::Shape::closed_line(points, stroke));
}

pub fn normalize_degrees(angle: f32) -> f32 {
    let a = angle.rem_euclid(360.0);
    if a > 180.0 { a - 360.0 } else { a }
}

/// Moves `grip`'s edges to `to`. Corner grips keep the aspect ratio when `keep_aspect` (on unless Shift is
/// held, as in OBS); `from_center` (Alt) mirrors the change on the opposite side.
fn resize(start: &Transform, grip: (i8, i8), to: (f32, f32), keep_aspect: bool, from_center: bool) -> Transform {
    let (sx, sy) = (f32::from(grip.0), f32::from(grip.1));
    let local = start.to_local(to);
    let (hw, hh) = (start.w / 2.0, start.h / 2.0);
    // The fixed side, in the start box's local coordinates.
    let anchor = if from_center { (0.0, 0.0) } else { (-sx * hw, -sy * hh) };
    let span = |s: f32, l: f32, a: f32, current: f32| {
        if s == 0.0 {
            current
        } else if from_center {
            (2.0 * s * l).max(MIN_SIZE)
        } else {
            (s * (l - a)).max(MIN_SIZE)
        }
    };
    let mut w = span(sx, local.0, anchor.0, start.w);
    let mut h = span(sy, local.1, anchor.1, start.h);
    if keep_aspect && sx != 0.0 && sy != 0.0 && start.w > 0.0 && start.h > 0.0 {
        let factor = (w / start.w).max(h / start.h);
        w = start.w * factor;
        h = start.h * factor;
    }
    let center_local = if from_center {
        (0.0, 0.0)
    } else {
        (if sx == 0.0 { 0.0 } else { anchor.0 + sx * w / 2.0 }, if sy == 0.0 { 0.0 } else { anchor.1 + sy * h / 2.0 })
    };
    let (cx, cy) = start.from_local(center_local);
    Transform { x: cx - w / 2.0, y: cy - h / 2.0, w, h, ..*start }
}

/// Snaps the moved box's edges and centre to the canvas and other layers; returns the guides to draw.
fn snap_move(t: &mut Transform, others: &[Transform], canvas: (f32, f32), threshold: f32) -> Vec<Guide> {
    let (x0, y0, x1, y1) = t.bounds();
    let mine_x = [x0, (x0 + x1) / 2.0, x1];
    let mine_y = [y0, (y0 + y1) / 2.0, y1];
    let mut targets_x = vec![0.0, canvas.0 / 2.0, canvas.0];
    let mut targets_y = vec![0.0, canvas.1 / 2.0, canvas.1];
    for other in others {
        let (a0, b0, a1, b1) = other.bounds();
        targets_x.extend([a0, (a0 + a1) / 2.0, a1]);
        targets_y.extend([b0, (b0 + b1) / 2.0, b1]);
    }
    let best = |mine: &[f32; 3], targets: &[f32]| {
        mine.iter()
            .flat_map(|m| targets.iter().map(move |t| (t - m, *t)))
            .filter(|(d, _)| d.abs() <= threshold)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
    };
    let mut guides = Vec::new();
    if let Some((dx, line)) = best(&mine_x, &targets_x) {
        t.x += dx;
        guides.push(Guide::Vertical(line));
    }
    if let Some((dy, line)) = best(&mine_y, &targets_y) {
        t.y += dy;
        guides.push(Guide::Horizontal(line));
    }
    guides
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn corner_resize_keeps_aspect_and_the_opposite_corner() {
        let start = Transform { x: 0.0, y: 0.0, w: 200.0, h: 100.0, ..Default::default() };
        let t = resize(&start, (1, 1), (300.0, 120.0), true, false);
        assert!(close(t.w, 300.0) && close(t.h, 150.0));
        assert!(close(t.x, 0.0) && close(t.y, 0.0));
        let free = resize(&start, (1, 1), (300.0, 120.0), false, false);
        assert!(close(free.w, 300.0) && close(free.h, 120.0));
    }

    #[test]
    fn edge_resize_from_the_left_moves_x() {
        let start = Transform { x: 100.0, y: 0.0, w: 200.0, h: 100.0, ..Default::default() };
        let t = resize(&start, (-1, 0), (50.0, 50.0), true, false);
        assert!(close(t.x, 50.0) && close(t.w, 250.0) && close(t.h, 100.0));
    }

    #[test]
    fn rotated_resize_keeps_the_anchor_corner_in_place() {
        let start = Transform { x: 0.0, y: 0.0, w: 100.0, h: 50.0, rotation: 30.0, ..Default::default() };
        let anchor = start.corners()[0];
        let t = resize(&start, (1, 1), start.from_local((80.0, 40.0)), false, false);
        let moved = t.corners()[0];
        assert!(close(anchor.0, moved.0) && close(anchor.1, moved.1), "{anchor:?} vs {moved:?}");
    }

    #[test]
    fn alt_resizes_around_the_centre() {
        let start = Transform { x: 0.0, y: 0.0, w: 100.0, h: 100.0, ..Default::default() };
        let t = resize(&start, (1, 0), (150.0, 50.0), true, true);
        assert!(close(t.w, 200.0) && close(t.center().0, 50.0));
    }

    #[test]
    fn moving_snaps_to_canvas_centre() {
        let mut t = Transform { x: 437.0, y: 10.0, w: 400.0, h: 100.0, ..Default::default() };
        let guides = snap_move(&mut t, &[], (1280.0, 720.0), 8.0);
        assert!(close(t.x, 440.0));
        assert_eq!(guides.len(), 1);
    }

    #[test]
    fn view_round_trips() {
        let scene = Scene { width: 1280, height: 720, ..Default::default() };
        let (view, shown) = View::fit(Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(640.0, 640.0)), &scene);
        assert!(close(shown.width(), 640.0) && close(shown.height(), 360.0));
        let p = view.to_scene(view.to_screen((100.0, 200.0)));
        assert!(close(p.0, 100.0) && close(p.1, 200.0));
    }

    use tt_core::vcam::scene::{Layer, Source};

    /// Feeds pointer events through real egui frames and returns the layer's transform afterwards.
    fn drag(scene: &mut Scene, selected: &mut Option<LayerId>, from: Pos2, to: Pos2, modifiers: egui::Modifiers) {
        let ctx = egui::Context::default();
        let mut state = CanvasState::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1288.0, 728.0));
        let steps: Vec<Vec<egui::Event>> = vec![
            vec![egui::Event::PointerMoved(from)],
            vec![egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers }],
            vec![egui::Event::PointerMoved(from + (to - from) * 0.5)],
            vec![egui::Event::PointerMoved(to)],
            vec![egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers }],
            vec![],
        ];
        for (i, events) in steps.into_iter().enumerate() {
            let input = egui::RawInput { screen_rect: Some(screen), events, time: Some(i as f64 * 0.1), ..Default::default() };
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
                    show(ui, &mut state, scene, selected, None);
                });
            });
            output.textures_delta.clear();
        }
    }

    fn one_layer() -> (Scene, LayerId) {
        let mut scene = Scene { width: 1280, height: 720, ..Default::default() };
        let layer = Layer::new("box", Source::TestPattern, Transform { x: 100.0, y: 100.0, w: 200.0, h: 100.0, ..Default::default() });
        let id = layer.id;
        scene.layers.push(layer);
        (scene, id)
    }

    #[test]
    fn dragging_a_layer_selects_and_moves_it() {
        let (mut scene, id) = one_layer();
        let mut selected = None;
        // The canvas fills the screen less a 4px margin at scale 1.
        drag(&mut scene, &mut selected, Pos2::new(204.0, 154.0), Pos2::new(304.0, 204.0), egui::Modifiers::NONE);
        assert_eq!(selected, Some(id));
        let t = scene.layer(id).unwrap().transform;
        assert!(close(t.x, 200.0) && close(t.y, 150.0), "{t:?}");
        assert!(close(t.w, 200.0));
    }

    #[test]
    fn dragging_a_corner_handle_resizes() {
        let (mut scene, id) = one_layer();
        let mut selected = Some(id);
        // Bottom-right handle sits at (300, 200) in the scene.
        drag(&mut scene, &mut selected, Pos2::new(304.0, 204.0), Pos2::new(404.0, 234.0), egui::Modifiers::NONE);
        let t = scene.layer(id).unwrap().transform;
        assert!(close(t.x, 100.0) && close(t.y, 100.0), "{t:?}");
        assert!(close(t.w, 300.0) && close(t.h, 150.0), "{t:?}");
    }

    #[test]
    fn clicking_empty_canvas_clears_the_selection() {
        let (mut scene, id) = one_layer();
        let mut selected = Some(id);
        drag(&mut scene, &mut selected, Pos2::new(900.0, 600.0), Pos2::new(900.0, 600.0), egui::Modifiers::NONE);
        assert_eq!(selected, None);
    }
}
