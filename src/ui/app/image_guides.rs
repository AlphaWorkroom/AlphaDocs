//! Alignment uses measured layout geometry in logical screen pixels. Nothing
//! here changes the document or participates in printing/export.
use super::*;

const TOLERANCE: f32 = 7.0;

#[derive(Clone, Copy)]
struct AxisGuide {
    value: f32,
    span: [f32; 2],
}
#[derive(Default)]
pub(super) struct Targets {
    x: Vec<AxisGuide>,
    y: Vec<AxisGuide>,
    lines: Vec<f32>,
}
pub(super) struct Result {
    pub rect: Rect,
    guides: Vec<[Pos2; 2]>,
}
impl Result {
    pub fn paint(&self, ui: &Ui, page: Rect) {
        let painter = ui.painter().with_clip_rect(ui.clip_rect().intersect(page));
        for segment in &self.guides {
            painter.line_segment(*segment, Stroke::new(1.0, Color32::from_rgb(201, 66, 185)));
        }
    }
}
impl Targets {
    fn add(&mut self, rect: Rect) {
        for value in [rect.center().x, rect.left(), rect.right()] {
            self.x.push(AxisGuide {
                value,
                span: [rect.top(), rect.bottom()],
            });
        }
        for value in [rect.center().y, rect.top(), rect.bottom()] {
            self.y.push(AxisGuide {
                value,
                span: [rect.left(), rect.right()],
            });
        }
    }
    pub fn collect(
        layout: &Layout,
        block: usize,
        page: usize,
        column: Rect,
        paper: Rect,
        origin: Pos2,
        scale: f32,
    ) -> Self {
        let mut targets = Self::default();
        targets.add(paper);
        targets.add(column);
        let screen =
            |r: Rect| Rect::from_min_size(origin + r.min.to_vec2() * scale, r.size() * scale);
        let mut paragraphs: std::collections::BTreeMap<Address, Rect> = Default::default();
        for line in &layout.lines {
            if line.page != page {
                continue;
            }
            let rect = screen(line.rect);
            if rect.right() < column.left() || rect.left() > column.right() {
                continue;
            }
            targets.lines.push(rect.top());
            paragraphs
                .entry(line.address)
                .and_modify(|r| *r = r.union(rect))
                .or_insert(rect);
        }
        for rect in paragraphs.values() {
            targets.add(*rect);
        }
        for object in &layout.objects {
            match object {
                Object::Image {
                    block: other,
                    page: p,
                    rect,
                    ..
                } if *p == page && *other != block => targets.add(screen(*rect)),
                Object::Border { page: p, rect, .. } if *p == page => targets.add(screen(*rect)),
                _ => {}
            }
        }
        targets.lines.sort_by(f32::total_cmp);
        targets.lines.dedup_by(|a, b| (*a - *b).abs() < 0.1);
        targets
    }
    pub fn resolve(&self, proposed: Rect, bounds: Rect, placement: ImagePlacement) -> Result {
        let nearest = |values: [f32; 3], guides: &[AxisGuide]| {
            values
                .into_iter()
                .flat_map(|v| guides.iter().map(move |g| (g.value - v, *g)))
                .filter(|(delta, _)| delta.abs() <= TOLERANCE)
                .min_by(|(a, _), (b, _)| a.abs().total_cmp(&b.abs()))
        };
        let mut rect = proposed;
        if placement.snap_guides {
            if let Some((dx, _)) = nearest([rect.center().x, rect.left(), rect.right()], &self.x) {
                rect = rect.translate(vec2(dx, 0.0));
            }
            if placement.snap == ImageSnap::Freeform
                && let Some((dy, _)) =
                    nearest([rect.center().y, rect.top(), rect.bottom()], &self.y)
            {
                rect = rect.translate(vec2(0.0, dy));
            }
        }
        let line = self
            .lines
            .iter()
            .copied()
            .filter(|y| *y >= bounds.top() && *y + rect.height() <= bounds.bottom())
            .min_by(|a, b| (*a - rect.top()).abs().total_cmp(&(*b - rect.top()).abs()));
        // Leave a genuinely free gap between closely spaced lines, including
        // zoomed-out pages; a fixed 7px threshold would catch every position.
        let line_tolerance = line.map_or(TOLERANCE, |y| {
            self.lines
                .iter()
                .map(|other| (*other - y).abs())
                .filter(|gap| *gap > 0.1)
                .min_by(f32::total_cmp)
                .map_or(TOLERANCE, |gap| TOLERANCE.min(gap / 3.0))
        });
        if placement.snap != ImageSnap::Freeform
            && let Some(y) = line
            && (placement.snap == ImageSnap::TextLines || (y - rect.top()).abs() <= line_tolerance)
        {
            rect = rect.translate(vec2(0.0, y - rect.top()));
        }
        let x = rect.left().clamp(
            bounds.left(),
            (bounds.right() - rect.width()).max(bounds.left()),
        );
        let y = rect.top().clamp(
            bounds.top(),
            (bounds.bottom() - rect.height()).max(bounds.top()),
        );
        rect = Rect::from_min_size(pos2(x, y), rect.size());
        let mut result = Result {
            rect,
            guides: vec![],
        };
        if placement.guides {
            if let Some((_, g)) = nearest([rect.center().x, rect.left(), rect.right()], &self.x) {
                result.guides.push([
                    pos2(g.value, g.span[0].min(rect.top()) - 8.0),
                    pos2(g.value, g.span[1].max(rect.bottom()) + 8.0),
                ]);
            }
            if let Some((_, g)) = nearest([rect.center().y, rect.top(), rect.bottom()], &self.y) {
                result.guides.push([
                    pos2(g.span[0].min(rect.left()) - 8.0, g.value),
                    pos2(g.span[1].max(rect.right()) + 8.0, g.value),
                ]);
            }
            if placement.snap != ImageSnap::Freeform
                && let Some(y) = line
                && (y - rect.top()).abs() < 0.1
            {
                result
                    .guides
                    .push([pos2(bounds.left(), y), pos2(bounds.right(), y)]);
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bounds() -> Rect {
        Rect::from_min_max(pos2(0.0, 0.0), pos2(500.0, 700.0))
    }
    #[test]
    fn line_modes_keep_horizontal_freedom_and_near_mode_keeps_gaps() {
        let t = Targets {
            lines: vec![100.0, 130.0, 160.0],
            ..Default::default()
        };
        let p = ImagePlacement {
            snap: ImageSnap::TextLines,
            ..Default::default()
        };
        let rect = Rect::from_min_size(pos2(73.2, 144.0), vec2(80.0, 50.0));
        assert_eq!(t.resolve(rect, bounds(), p).rect.min, pos2(73.2, 130.0));
        let p = ImagePlacement {
            snap: ImageSnap::NearTextLines,
            ..p
        };
        assert_eq!(t.resolve(rect, bounds(), p).rect, rect);
        let near = rect.translate(vec2(0.0, -9.0));
        assert_eq!(t.resolve(near, bounds(), p).rect.min, pos2(73.2, 130.0));
        assert_eq!(
            t.resolve(near, bounds(), ImagePlacement::default()).rect,
            near
        );
        let compressed = Targets {
            lines: vec![100.0, 109.0, 118.0],
            ..Default::default()
        };
        let gap = Rect::from_min_size(pos2(73.2, 104.5), rect.size());
        assert_eq!(compressed.resolve(gap, bounds(), p).rect, gap);
    }
    #[test]
    fn guides_are_visual_until_magnetism_is_enabled_and_clamp_to_page() {
        let mut t = Targets::default();
        t.add(Rect::from_min_size(pos2(200.0, 100.0), vec2(100.0, 100.0)));
        let r = Rect::from_min_size(pos2(204.0, 150.0), vec2(100.0, 100.0));
        let preview = t.resolve(r, bounds(), ImagePlacement::default());
        assert_eq!(preview.rect, r);
        assert_eq!(preview.guides.len(), 2);
        let p = ImagePlacement {
            snap_guides: true,
            ..Default::default()
        };
        assert_eq!(t.resolve(r, bounds(), p).rect.left(), 200.0);
        let p = ImagePlacement { guides: false, ..p };
        assert!(t.resolve(r, bounds(), p).guides.is_empty());
        let outside = r.translate(vec2(300.0, 800.0));
        assert_eq!(
            t.resolve(outside, bounds(), p).rect.right_bottom(),
            bounds().right_bottom()
        );
    }
}
