use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
#[derive(Clone, Copy)]
pub enum Icon {
    Document,
    Copy,
    Delete,
    Download,
    PageSetup,
    Share,
    Home,
    Undo,
    Redo,
    Print,
    Proofing,
    LineSpacing,
    Search,
    FindReplace,
    Save,
    Link,
    Image,
    Highlight,
    TextColor,
    Table,
    AlignLeft,
    AlignCenter,
    AlignRight,
    Justify,
    Bullet,
    Number,
    Check,
    Resolve,
    Indent,
    Outdent,
    Clear,
    History,
    Theme,
    Outline,
    Plus,
    More,
    Folder,
    Star,
    Sort,
    Comment,
    Reaction,
    Wrap,
    Paintbrush,
    Close,
    Edit,
    Suggest,
    View,
    ChevronUp,
    ChevronDown,
    Citations,
    Dictionary,
    Microphone,
    Translate,
    Accessibility,
    Bell,
    Compare,
}
pub fn draw(ui: &Ui, rect: Rect, icon: Icon, color: Color32) {
    let p = ui.painter();
    let r = Rect::from_center_size(rect.center(), vec2(18.0, 18.0));
    let o = r.min;
    let xy = |x: f32, y: f32| o + vec2(x, y);
    let s = Stroke::new(1.5, color);
    let line = |a: Pos2, b: Pos2| {
        p.line_segment([a, b], s);
    };
    match icon {
        Icon::Citations => {
            for x in [2.0, 10.0] {
                p.rect_stroke(
                    Rect::from_min_max(xy(x, 4.0), xy(x + 5.0, 10.0)),
                    1.0,
                    s,
                    egui::StrokeKind::Inside,
                );
                line(xy(x + 5.0, 10.0), xy(x + 3.0, 14.0));
                line(xy(x + 3.0, 14.0), xy(x, 14.0));
            }
        }
        Icon::Dictionary => {
            p.rect_stroke(
                Rect::from_min_max(xy(3.0, 1.0), xy(15.0, 17.0)),
                1.0,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(6.0, 1.0), xy(6.0, 17.0));
            line(xy(8.0, 11.0), xy(10.5, 5.0));
            line(xy(10.5, 5.0), xy(13.0, 11.0));
            line(xy(9.0, 9.0), xy(12.0, 9.0));
        }
        Icon::Microphone => {
            p.rect_stroke(
                Rect::from_min_max(xy(6.0, 1.0), xy(12.0, 12.0)),
                3.0,
                s,
                egui::StrokeKind::Inside,
            );
            p.add(egui::Shape::line(
                vec![
                    xy(3.0, 8.0),
                    xy(3.0, 11.0),
                    xy(5.0, 14.0),
                    xy(9.0, 15.0),
                    xy(13.0, 14.0),
                    xy(15.0, 11.0),
                    xy(15.0, 8.0),
                ],
                s,
            ));
            line(xy(9.0, 15.0), xy(9.0, 18.0));
        }
        Icon::Bell => {
            p.add(egui::Shape::closed_line(
                vec![
                    xy(3.0, 13.0),
                    xy(5.0, 10.0),
                    xy(5.0, 6.0),
                    xy(7.0, 3.0),
                    xy(11.0, 3.0),
                    xy(13.0, 6.0),
                    xy(13.0, 10.0),
                    xy(15.0, 13.0),
                ],
                s,
            ));
            line(xy(7.0, 16.0), xy(11.0, 16.0));
            line(xy(9.0, 1.0), xy(9.0, 3.0));
        }
        Icon::Accessibility => {
            p.circle_stroke(xy(9.0, 2.5), 1.8, s);
            line(xy(2.0, 7.0), xy(16.0, 7.0));
            line(xy(9.0, 7.0), xy(9.0, 11.0));
            line(xy(9.0, 11.0), xy(5.0, 17.0));
            line(xy(9.0, 11.0), xy(13.0, 17.0));
        }
        Icon::Translate => {
            line(xy(1.0, 4.0), xy(11.0, 4.0));
            line(xy(6.0, 1.0), xy(6.0, 4.0));
            line(xy(9.0, 4.0), xy(7.0, 8.0));
            line(xy(7.0, 8.0), xy(2.0, 12.0));
            line(xy(3.0, 6.0), xy(9.0, 11.0));
            line(xy(10.0, 17.0), xy(14.0, 7.0));
            line(xy(14.0, 7.0), xy(18.0, 17.0));
            line(xy(12.0, 14.0), xy(16.0, 14.0));
        }
        Icon::Compare => {
            for x in [1.0, 10.0] {
                p.rect_stroke(
                    Rect::from_min_max(xy(x, 2.0), xy(x + 7.0, 16.0)),
                    0.5,
                    s,
                    egui::StrokeKind::Inside,
                );
                line(xy(x + 2.0, 6.0), xy(x + 5.0, 6.0));
                line(xy(x + 2.0, 10.0), xy(x + 5.0, 10.0));
            }
        }
        Icon::Copy => {
            p.rect_stroke(
                Rect::from_min_max(xy(5.0, 2.0), xy(15.0, 14.0)),
                0.5,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(2.0, 5.0), xy(2.0, 17.0));
            line(xy(2.0, 17.0), xy(12.0, 17.0));
        }
        Icon::Delete => {
            line(xy(3.0, 4.0), xy(15.0, 4.0));
            line(xy(6.0, 1.5), xy(12.0, 1.5));
            p.rect_stroke(
                Rect::from_min_max(xy(5.0, 4.0), xy(13.0, 16.0)),
                0.5,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(7.5, 7.0), xy(7.5, 13.0));
            line(xy(10.5, 7.0), xy(10.5, 13.0));
        }
        Icon::Download => {
            line(xy(9.0, 1.0), xy(9.0, 12.0));
            line(xy(5.0, 8.0), xy(9.0, 12.0));
            line(xy(9.0, 12.0), xy(13.0, 8.0));
            line(xy(3.0, 13.0), xy(3.0, 16.0));
            line(xy(3.0, 16.0), xy(15.0, 16.0));
            line(xy(15.0, 16.0), xy(15.0, 13.0));
        }
        Icon::PageSetup => {
            p.rect_stroke(
                Rect::from_min_max(xy(3.0, 1.0), xy(15.0, 17.0)),
                0.5,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(6.0, 4.0), xy(12.0, 4.0));
            line(xy(6.0, 14.0), xy(12.0, 14.0));
        }
        Icon::Share => {
            p.circle_stroke(xy(6.0, 5.0), 2.5, s);
            p.add(egui::Shape::line(
                vec![
                    xy(1.0, 15.0),
                    xy(1.0, 13.0),
                    xy(3.0, 10.0),
                    xy(9.0, 10.0),
                    xy(11.0, 13.0),
                    xy(11.0, 15.0),
                ],
                s,
            ));
            line(xy(14.0, 5.0), xy(14.0, 11.0));
            line(xy(11.0, 8.0), xy(17.0, 8.0));
        }
        Icon::ChevronUp | Icon::ChevronDown => {
            let y = if matches!(icon, Icon::ChevronUp) {
                -1.0
            } else {
                1.0
            };
            line(xy(5.0, 9.0 - y * 2.0), xy(9.0, 9.0 + y * 2.0));
            line(xy(9.0, 9.0 + y * 2.0), xy(13.0, 9.0 - y * 2.0));
        }
        Icon::Edit | Icon::Suggest => {
            if matches!(icon, Icon::Suggest) {
                p.rect_stroke(
                    Rect::from_min_max(xy(1.0, 2.0), xy(17.0, 15.0)),
                    1.0,
                    s,
                    egui::StrokeKind::Inside,
                );
                line(xy(1.0, 15.0), xy(1.0, 18.0));
                line(xy(1.0, 18.0), xy(5.0, 15.0));
            }
            // A crisp hollow pencil: shaft, graphite tip and separate eraser cap.
            p.add(egui::Shape::convex_polygon(
                vec![xy(4.0, 11.5), xy(12.0, 3.5), xy(14.5, 6.0), xy(6.5, 14.0)],
                Color32::TRANSPARENT,
                Stroke::new(1.4, color),
            ));
            p.add(egui::Shape::convex_polygon(
                vec![xy(3.3, 12.2), xy(2.5, 15.5), xy(5.8, 14.7)],
                color,
                Stroke::NONE,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![xy(13.2, 2.3), xy(14.3, 1.2), xy(16.8, 3.7), xy(15.7, 4.8)],
                color,
                Stroke::NONE,
            ));
        }
        Icon::View => {
            p.add(egui::Shape::closed_line(
                vec![
                    xy(0.0, 9.0),
                    xy(3.0, 5.0),
                    xy(7.0, 3.0),
                    xy(11.0, 3.0),
                    xy(15.0, 5.0),
                    xy(18.0, 9.0),
                    xy(15.0, 13.0),
                    xy(11.0, 15.0),
                    xy(7.0, 15.0),
                    xy(3.0, 13.0),
                ],
                s,
            ));
            p.circle_stroke(xy(9.0, 9.0), 3.0, s);
        }
        Icon::Resolve => {
            line(xy(2.0, 9.0), xy(7.0, 14.0));
            line(xy(7.0, 14.0), xy(16.0, 3.0));
        }
        Icon::Document | Icon::Save => {
            p.rect_stroke(
                Rect::from_min_max(xy(3.0, 1.0), xy(15.0, 17.0)),
                1.0,
                s,
                egui::StrokeKind::Inside,
            );
            for y in [6.0, 9.0, 12.0] {
                line(xy(6.0, y), xy(12.0, y));
            }
        }
        Icon::Home => {
            line(xy(1.0, 8.0), xy(9.0, 1.0));
            line(xy(9.0, 1.0), xy(17.0, 8.0));
            line(xy(4.0, 6.0), xy(4.0, 16.0));
            line(xy(4.0, 16.0), xy(14.0, 16.0));
            line(xy(14.0, 16.0), xy(14.0, 6.0));
        }
        Icon::Undo | Icon::Redo => {
            let flip = matches!(icon, Icon::Redo);
            let pt = |x: f32, y: f32| xy(if flip { 18.0 - x } else { x }, y);
            line(pt(3.0, 5.0), pt(13.0, 5.0));
            line(pt(13.0, 5.0), pt(15.0, 8.0));
            line(pt(15.0, 8.0), pt(15.0, 12.0));
            line(pt(15.0, 12.0), pt(6.0, 12.0));
            line(pt(3.0, 5.0), pt(7.0, 1.0));
            line(pt(3.0, 5.0), pt(7.0, 9.0));
        }
        Icon::Proofing => {
            line(xy(1.0, 13.0), xy(5.0, 2.0));
            line(xy(5.0, 2.0), xy(9.0, 13.0));
            line(xy(3.0, 9.0), xy(7.0, 9.0));
            line(xy(9.0, 13.0), xy(12.0, 16.0));
            line(xy(12.0, 16.0), xy(17.0, 8.0));
        }
        Icon::LineSpacing => {
            for y in [3.0, 9.0, 15.0] {
                line(xy(8.0, y), xy(17.0, y));
            }
            line(xy(3.0, 2.0), xy(3.0, 16.0));
            line(xy(0.0, 5.0), xy(3.0, 2.0));
            line(xy(6.0, 5.0), xy(3.0, 2.0));
            line(xy(0.0, 13.0), xy(3.0, 16.0));
            line(xy(6.0, 13.0), xy(3.0, 16.0));
        }
        Icon::Print => {
            p.rect_stroke(
                Rect::from_min_max(xy(1.0, 6.0), xy(17.0, 13.0)),
                2.0,
                s,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(xy(4.0, 0.0), xy(14.0, 6.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_min_max(xy(4.0, 10.0), xy(14.0, 18.0)),
                0.0,
                ui.visuals().panel_fill,
            );
            p.rect_stroke(
                Rect::from_min_max(xy(4.0, 10.0), xy(14.0, 18.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Search => {
            p.circle_stroke(xy(7.0, 7.0), 5.0, s);
            line(xy(11.0, 11.0), xy(16.0, 16.0));
        }
        Icon::FindReplace => {
            p.circle_stroke(xy(6.0, 6.0), 4.5, s);
            line(xy(9.0, 9.0), xy(12.0, 12.0));
            line(xy(3.0, 15.0), xy(16.0, 15.0));
            line(xy(13.0, 12.0), xy(16.0, 15.0));
            line(xy(13.0, 18.0), xy(16.0, 15.0));
        }
        Icon::Link => {
            p.rect_stroke(
                Rect::from_min_max(xy(0.0, 6.0), xy(10.0, 12.0)),
                3.0,
                s,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                Rect::from_min_max(xy(8.0, 6.0), xy(18.0, 12.0)),
                3.0,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(6.0, 9.0), xy(12.0, 9.0));
        }
        Icon::TextColor => {
            line(xy(4.0, 12.0), xy(9.0, 1.0));
            line(xy(9.0, 1.0), xy(14.0, 12.0));
            line(xy(6.0, 8.0), xy(12.0, 8.0));
            p.rect_filled(
                Rect::from_min_max(xy(1.0, 15.0), xy(17.0, 18.0)),
                0.0,
                color,
            );
        }
        Icon::Highlight => {
            p.add(egui::Shape::convex_polygon(
                vec![xy(4.0, 11.0), xy(12.0, 1.0), xy(16.0, 5.0), xy(8.0, 15.0)],
                color,
                Stroke::NONE,
            ));
            line(xy(4.0, 12.0), xy(2.0, 15.0));
            line(xy(2.0, 15.0), xy(6.0, 15.0));
        }
        Icon::Image => {
            p.rect_stroke(r, 1.0, s, egui::StrokeKind::Inside);
            p.circle_filled(xy(5.0, 5.0), 1.5, color);
            line(xy(2.0, 15.0), xy(7.0, 9.0));
            line(xy(7.0, 9.0), xy(11.0, 13.0));
            line(xy(11.0, 13.0), xy(14.0, 7.0));
            line(xy(14.0, 7.0), xy(17.0, 14.0));
        }
        Icon::Table => {
            p.rect_stroke(r, 0.0, s, egui::StrokeKind::Inside);
            for n in [6.0, 12.0] {
                line(xy(n, 0.0), xy(n, 18.0));
                line(xy(0.0, n), xy(18.0, n));
            }
        }
        Icon::AlignLeft | Icon::AlignCenter | Icon::AlignRight | Icon::Justify => {
            for (i, y) in [3.0, 7.0, 11.0, 15.0].iter().enumerate() {
                let width = if i % 2 == 0 || matches!(icon, Icon::Justify) {
                    16.0
                } else {
                    10.0
                };
                let x = match icon {
                    Icon::AlignRight => 17.0 - width,
                    Icon::AlignCenter => 9.0 - width * 0.5,
                    _ => 1.0,
                };
                line(xy(x, *y), xy(x + width, *y));
            }
        }
        Icon::Bullet | Icon::Number | Icon::Check => {
            for (i, y) in [4.0, 9.0, 14.0].iter().enumerate() {
                if matches!(icon, Icon::Bullet) {
                    p.circle_filled(xy(2.0, *y), 1.4, color);
                } else if matches!(icon, Icon::Check) {
                    line(xy(0.0, *y), xy(2.0, *y + 2.0));
                    line(xy(2.0, *y + 2.0), xy(5.0, *y - 2.0));
                } else {
                    p.text(
                        xy(1.0, *y),
                        egui::Align2::CENTER_CENTER,
                        (i + 1).to_string(),
                        egui::FontId::proportional(7.0),
                        color,
                    );
                }
                line(xy(7.0, *y), xy(17.0, *y));
            }
        }
        Icon::Indent | Icon::Outdent => {
            for y in [3.0, 7.0, 11.0, 15.0] {
                line(xy(9.0, y), xy(17.0, y));
            }
            let x = if matches!(icon, Icon::Indent) {
                6.0
            } else {
                1.0
            };
            let other = if x == 6.0 { 1.0 } else { 6.0 };
            line(xy(other, 5.0), xy(x, 9.0));
            line(xy(x, 9.0), xy(other, 13.0));
        }
        Icon::Clear => {
            line(xy(5.0, 2.0), xy(17.0, 2.0));
            line(xy(11.0, 2.0), xy(8.0, 15.0));
            line(xy(1.0, 1.0), xy(17.0, 17.0));
        }
        Icon::History => {
            p.circle_stroke(xy(9.0, 9.0), 7.5, s);
            line(xy(9.0, 4.0), xy(9.0, 9.0));
            line(xy(9.0, 9.0), xy(13.0, 11.0));
        }
        Icon::Theme => {
            p.circle_stroke(xy(9.0, 9.0), 5.0, s);
            for (a, b) in [
                (xy(9.0, 0.0), xy(9.0, 2.0)),
                (xy(9.0, 16.0), xy(9.0, 18.0)),
                (xy(0.0, 9.0), xy(2.0, 9.0)),
                (xy(16.0, 9.0), xy(18.0, 9.0)),
            ] {
                line(a, b);
            }
        }
        Icon::Outline => {
            for y in [3.0, 9.0, 15.0] {
                line(xy(1.0, y), xy(17.0, y));
            }
        }
        Icon::Plus => {
            line(xy(9.0, 1.0), xy(9.0, 17.0));
            line(xy(1.0, 9.0), xy(17.0, 9.0));
        }
        Icon::More => {
            for y in [3.0, 9.0, 15.0] {
                p.circle_filled(xy(9.0, y), 1.5, color);
            }
        }
        Icon::Folder => {
            p.rect_stroke(
                Rect::from_min_max(xy(1.0, 5.0), xy(17.0, 16.0)),
                1.0,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(1.0, 5.0), xy(1.0, 2.0));
            line(xy(1.0, 2.0), xy(8.0, 2.0));
            line(xy(8.0, 2.0), xy(10.0, 5.0));
        }
        Icon::Star => {
            let pts = (0..10)
                .map(|i| {
                    let a = i as f32 * std::f32::consts::PI / 5.0 - std::f32::consts::FRAC_PI_2;
                    let radius = if i % 2 == 0 { 8.5 } else { 4.0 };
                    xy(9.0 + a.cos() * radius, 9.0 + a.sin() * radius)
                })
                .collect();
            p.add(egui::Shape::closed_line(pts, s));
        }
        Icon::Close => {
            line(xy(4.0, 4.0), xy(14.0, 14.0));
            line(xy(14.0, 4.0), xy(4.0, 14.0));
        }
        Icon::Sort => {
            for (i, y) in [3.0, 8.0, 13.0].into_iter().enumerate() {
                line(xy(1.0, y), xy(11.0 - i as f32 * 3.0, y));
            }
            line(xy(15.0, 2.0), xy(15.0, 16.0));
            line(xy(12.0, 13.0), xy(15.0, 16.0));
            line(xy(15.0, 16.0), xy(18.0, 13.0));
        }
        Icon::Comment => {
            p.rect_stroke(
                Rect::from_min_max(xy(1.0, 2.0), xy(17.0, 14.0)),
                1.0,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(3.0, 14.0), xy(3.0, 18.0));
            line(xy(3.0, 18.0), xy(7.0, 14.0));
            line(xy(5.0, 8.0), xy(13.0, 8.0));
            line(xy(9.0, 4.0), xy(9.0, 12.0));
        }
        Icon::Reaction => {
            p.circle_stroke(xy(8.0, 10.0), 7.0, s);
            p.circle_filled(xy(5.0, 8.0), 1.0, color);
            p.circle_filled(xy(11.0, 8.0), 1.0, color);
            line(xy(4.0, 12.0), xy(6.0, 14.0));
            line(xy(6.0, 14.0), xy(10.0, 14.0));
            line(xy(10.0, 14.0), xy(12.0, 12.0));
            line(xy(15.0, 0.0), xy(15.0, 6.0));
            line(xy(12.0, 3.0), xy(18.0, 3.0));
        }
        Icon::Wrap => {
            p.rect_stroke(
                Rect::from_min_max(xy(1.0, 6.0), xy(7.0, 12.0)),
                0.0,
                s,
                egui::StrokeKind::Inside,
            );
            for y in [2.0, 16.0] {
                line(xy(1.0, y), xy(17.0, y));
            }
            for y in [7.0, 11.0] {
                line(xy(10.0, y), xy(17.0, y));
            }
        }
        Icon::Paintbrush => {
            p.rect_stroke(
                Rect::from_min_max(xy(1.0, 1.0), xy(17.0, 6.0)),
                1.0,
                s,
                egui::StrokeKind::Inside,
            );
            line(xy(3.0, 6.0), xy(3.0, 9.0));
            line(xy(3.0, 9.0), xy(9.0, 9.0));
            line(xy(9.0, 9.0), xy(9.0, 12.0));
            p.rect_stroke(
                Rect::from_min_max(xy(7.0, 12.0), xy(11.0, 18.0)),
                1.0,
                s,
                egui::StrokeKind::Inside,
            );
        }
    }
}
pub fn button(ui: &mut Ui, icon: Icon, label: &str, selected: bool, enabled: bool) -> Response {
    let (id, rect) = ui.allocate_space(vec2(30.0, 30.0));
    paint_button(ui, rect, id, icon, label, selected, enabled)
}
pub fn button_with_id(
    ui: &mut Ui,
    id: egui::Id,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let (_, rect) = ui.allocate_space(vec2(30.0, 30.0));
    paint_button(ui, rect, id, icon, label, selected, enabled)
}
fn paint_button(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    icon: Icon,
    label: &str,
    selected: bool,
    enabled: bool,
) -> Response {
    let response = ui.interact(
        rect,
        id,
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    if selected || response.hovered() && enabled {
        ui.painter().rect_filled(
            rect,
            5.0,
            if selected {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().widgets.hovered.weak_bg_fill
            },
        );
    }
    let color = if enabled && matches!(icon, Icon::Resolve) {
        ui.visuals().selection.stroke.color
    } else if enabled {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color().gamma_multiply(0.5)
    };
    draw(ui, rect, icon, color);
    response.on_hover_text(super::platform::shortcut_label(ui.ctx(), label))
}
pub fn brand(ui: &mut Ui, rect: Rect, _color: Color32) {
    let rect = Rect::from_center_size(
        rect.center(),
        vec2(
            rect.width().min(rect.height()),
            rect.width().min(rect.height()),
        ),
    );
    let id = egui::Id::new("alphadocs_brand_texture");
    let texture = ui
        .ctx()
        .data(|data| data.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let image = image::load_from_memory(include_bytes!("../../assets/alphadocs-icon.png"))
                .expect("bundled AlphaDocs logo is valid PNG")
                .to_rgba8();
            let texture = ui.ctx().load_texture(
                "alphadocs_brand",
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx()
                .data_mut(|data| data.insert_temp(id, texture.clone()));
            texture
        });
    ui.painter().image(
        texture.id(),
        rect,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

pub fn workroom_brand(ui: &mut Ui, rect: Rect, tint: Color32) {
    let id = egui::Id::new("alpha_workroom_brand_texture");
    let texture = ui
        .ctx()
        .data(|d| d.get_temp::<egui::TextureHandle>(id))
        .unwrap_or_else(|| {
            let mut image =
                image::load_from_memory(include_bytes!("../../assets/alpha-workroom-brand.png"))
                    .expect("bundled Alpha Workroom artwork is valid PNG")
                    .to_rgba8();
            // The supplied black artwork is an alpha mask. Tint its white RGB
            // at paint time so theme changes preserve every antialiased edge.
            for pixel in image.pixels_mut() {
                pixel.0[..3].fill(255);
            }
            let texture = ui.ctx().load_texture(
                "alpha_workroom_brand",
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            );
            ui.ctx().data_mut(|d| d.insert_temp(id, texture.clone()));
            texture
        });
    let size = texture.size_vec2();
    let ratio = (rect.width() / size.x).min(rect.height() / size.y);
    let target = Rect::from_center_size(rect.center(), size * ratio);
    ui.painter().image(
        texture.id(),
        target,
        Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
        tint,
    );
}
