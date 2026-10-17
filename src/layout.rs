use crate::{fonts, model::*};
use eframe::egui::{
    self, Color32, Context, FontFamily, FontId, Galley, Pos2, Rect, Stroke, Vec2, pos2, vec2,
};
use std::{collections::HashMap, sync::Arc};

/// Physical checkbox size shared by the canvas and PDF. Leave a gap between
/// adjacent lines even when paragraphs have no extra spacing.
pub fn checklist_mark_size(line_height: f32) -> f32 {
    (line_height * 0.7).min(15.0)
}

#[derive(Clone)]
pub struct Line {
    pub address: Address,
    pub start: usize,
    pub end: usize,
    pub page: usize,
    pub rect: Rect,
    pub row: Arc<egui::epaint::text::Row>,
    pub marker: Option<String>,
}
impl Line {
    pub fn x(&self, offset: usize) -> f32 {
        let row = &self.row;
        let i = offset.saturating_sub(self.start);
        row.glyphs.get(i).map_or(row.size.x, |g| g.pos.x)
    }
    pub fn hit(&self, x: f32) -> usize {
        let row = &self.row;
        for (i, g) in row.glyphs.iter().enumerate() {
            if x < g.pos.x + g.advance_width * 0.5 {
                return self.start + i;
            }
        }
        self.end
    }
}
#[derive(Clone)]
pub enum Object {
    Image {
        block: usize,
        page: usize,
        rect: Rect,
        /// Unclipped column/block origin, used to commit pointer placement.
        anchor: Pos2,
    },
    Border {
        page: usize,
        rect: Rect,
        address: Address,
    },
    Rule {
        page: usize,
        rect: Rect,
    },
}
pub struct Layout {
    pub lines: Vec<Line>,
    pub running: Vec<Line>,
    pub objects: Vec<Object>,
    pub pages: usize,
    pub size: Vec2,
}
#[derive(Default)]
pub struct MeasurementCache {
    document: String,
    dpi: f32,
    generation: u64,
    paragraphs: HashMap<(Address, u32), (Paragraph, Arc<Galley>, u64)>,
}
impl MeasurementCache {
    pub fn invalidate(&mut self) {
        self.paragraphs.clear();
    }
    fn begin(&mut self, ctx: &Context, doc: &Document) {
        if self.document != doc.id || self.dpi != ctx.pixels_per_point() {
            self.invalidate();
            self.document = doc.id.clone();
            self.dpi = ctx.pixels_per_point();
        }
        self.generation = self.generation.wrapping_add(1);
    }
    fn shape(
        &mut self,
        ctx: &Context,
        address: Address,
        paragraph: &Paragraph,
        width: f32,
    ) -> Arc<Galley> {
        let key = (address, width.to_bits());
        if let Some((old, galley, generation)) = self.paragraphs.get_mut(&key)
            && old == paragraph
        {
            *generation = self.generation;
            return galley.clone();
        }
        let galley = shape(ctx, paragraph, width);
        self.paragraphs
            .insert(key, (paragraph.clone(), galley.clone(), self.generation));
        galley
    }
}

pub fn color(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
pub fn text_format(ctx: &Context, s: &TextStyle, spacing: f32) -> egui::TextFormat {
    let family = FontFamily::Name(fonts::key(s).into());
    let line_height =
        ctx.fonts_mut(|f| f.row_height(&FontId::new(s.size, family.clone()))) * spacing;
    egui::TextFormat {
        font_id: FontId::new(s.rendered_size(), family),
        valign: match s.baseline {
            Baseline::Normal => egui::Align::Center,
            Baseline::Superscript => egui::Align::Min,
            Baseline::Subscript => egui::Align::Max,
        },
        color: color(s.color),
        background: s.highlight.map(color).unwrap_or(Color32::TRANSPARENT),
        underline: if s.underline || s.link.is_some() {
            Stroke::new(0.55, color(s.color))
        } else {
            Stroke::NONE
        },
        strikethrough: if s.strike {
            Stroke::new(0.55, color(s.color))
        } else {
            Stroke::NONE
        },
        line_height: Some(line_height),
        ..Default::default()
    }
}
pub fn shape(ctx: &Context, p: &Paragraph, width: f32) -> Arc<Galley> {
    shape_range(ctx, p, 0, p.chars.len(), width)
}
#[cfg(test)]
#[test]
fn bounded_shaping_matches_full_paragraph_first_rows_at_different_sizes() {
    let ctx = Context::default();
    fonts::install(&ctx);
    let mut output = ctx.run_ui(Default::default(), |_| {
        for size in [1.0, 11.0, 32.0] {
            let mut p = Paragraph::from_text(
                &"Unicode café e\u{301} text 👩‍🔬 with proper measured wrapping. ".repeat(50),
            );
            p.first_indent = 8.0;
            for c in &mut p.chars {
                c.style.size = size;
            }
            for width in [24.0, 48.0, 112.0, 451.0] {
                let full = shape(&ctx, &p, width);
                let bounded = shape_first_row(&ctx, &p, 0, width);
                let glyphs = |g: &Galley| {
                    g.rows[0]
                        .glyphs
                        .iter()
                        .map(|glyph| (glyph.chr, glyph.pos, glyph.advance_width))
                        .collect::<Vec<_>>()
                };
                assert_eq!(glyphs(&full), glyphs(&bounded));
                assert_eq!(full.rows[0].size, bounded.rows[0].size);
            }
        }
    });
    output.textures_delta.clear();
}
fn shape_range(ctx: &Context, p: &Paragraph, start: usize, end: usize, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width.max(24.0);
    job.keep_trailing_whitespace = true;
    job.justify = p.alignment == Alignment::Justified;
    let mut i = start;
    while i < end {
        let style = &p.chars[i].style;
        let mut run_end = i + 1;
        while run_end < end && p.chars[run_end].style == *style {
            run_end += 1;
        }
        let text: String = p.chars[i..run_end].iter().map(|c| c.ch).collect();
        job.append(
            &text,
            if i == 0 { p.first_indent.max(0.0) } else { 0.0 },
            text_format(ctx, style, p.line_spacing),
        );
        i = run_end;
    }
    if start == end {
        job.append("", 0.0, text_format(ctx, &p.style, p.line_spacing));
    }
    ctx.fonts_mut(|fonts| fonts.layout_job(job))
}
/// Batch input only to bound shaping work. A second measured row proves that
/// the first wrap is internal to the batch; character counts never choose it.
fn shape_first_row(ctx: &Context, p: &Paragraph, start: usize, width: f32) -> Arc<Galley> {
    let mut end = (start + 512).min(p.chars.len());
    loop {
        while end < p.chars.len() && !p.chars[end - 1].ch.is_whitespace() {
            end += 1;
        }
        let g = shape_range(ctx, p, start, end, width);
        if g.rows.len() > 1 || end == p.chars.len() {
            return g;
        }
        end = (start + (end - start).max(1) * 2).min(p.chars.len());
    }
}
/// Rasterize visible lines at the actual display size for crisp text at every zoom.
/// Canonical measured rows still determine page flow and exports.
pub fn display_line(
    ctx: &Context,
    doc: &Document,
    line: &Line,
    scale: f32,
    background: [u8; 3],
) -> Arc<Galley> {
    let p = doc.paragraph(line.address).expect("Measured paragraph");
    let mut job = egui::text::LayoutJob {
        keep_trailing_whitespace: true,
        ..Default::default()
    };
    let mut start = line.start;
    while start < line.end {
        let style = &p.chars[start].style;
        let mut end = start + 1;
        while end < line.end && p.chars[end].style == *style {
            end += 1;
        }
        let mut display = style.clone();
        display.size *= scale;
        display.color = contrast_color(display.color, display.highlight.unwrap_or(background));
        let text: String = p.chars[start..end].iter().map(|c| c.ch).collect();
        job.append(
            &text,
            if start == 0 {
                p.first_indent.max(0.0) * scale
            } else {
                0.0
            },
            text_format(ctx, &display, p.line_spacing),
        );
        start = end;
    }
    if line.start == line.end {
        let mut style = p.style.clone();
        style.size *= scale;
        job.append("", 0.0, text_format(ctx, &style, p.line_spacing));
    }
    let mut display = ctx.fonts_mut(|fonts| fonts.layout_job(job));
    if p.alignment == Alignment::Justified
        && line.end < p.chars.len()
        && p.chars[line.end].ch != '\n'
    {
        justify_display(
            &mut display,
            line.rect.width() * scale,
            ctx.pixels_per_point(),
        );
    }
    display
}
pub fn display_x(galley: &Galley, start: usize, offset: usize) -> f32 {
    let row = &galley.rows[0];
    row.glyphs
        .get(offset.saturating_sub(start))
        .map_or(row.size.x, |g| g.pos.x)
}
pub fn display_hit(galley: &Galley, start: usize, end: usize, x: f32) -> usize {
    galley.rows[0]
        .glyphs
        .iter()
        .position(|g| x < g.pos.x + g.advance_width * 0.5)
        .map_or(end, |i| start + i)
}
fn justify_display(galley: &mut Arc<Galley>, width: f32, pixels_per_point: f32) {
    let galley = Arc::make_mut(galley);
    let placed = &mut galley.rows[0];
    let row = Arc::make_mut(&mut placed.row);
    let spaces = row.glyphs.iter().filter(|g| g.chr == ' ').count();
    let extra = width - row.size.x;
    if spaces == 0 || extra <= 0.0 {
        return;
    }
    let mut count = 0;
    let shifts: Vec<_> = row
        .glyphs
        .iter()
        .map(|g| {
            let shift = (extra * count as f32 / spaces as f32 * pixels_per_point).round()
                / pixels_per_point;
            if g.chr == ' ' {
                count += 1;
            }
            (g.pos.x, shift)
        })
        .collect();
    let mut glyph_vertices = vec![false; row.visuals.mesh.vertices.len()];
    for (i, glyph) in row.glyphs.iter_mut().enumerate() {
        let shift = shifts[i].1;
        if !glyph.uv_rect.is_nothing() {
            let begin = glyph.first_vertex as usize;
            if let Some(vertices) = row.visuals.mesh.vertices.get_mut(begin..begin + 4) {
                for (j, vertex) in vertices.iter_mut().enumerate() {
                    vertex.pos.x += shift;
                    glyph_vertices[begin + j] = true;
                }
            }
        }
        glyph.pos.x += shift;
        if let Some(next) = shifts.get(i + 1) {
            glyph.advance_width += next.1 - shift;
        }
    }
    // Decorations span runs, rather than individual glyph quads. Expand their
    // endpoints with the same word-space distribution as the text.
    for (i, vertex) in row.visuals.mesh.vertices.iter_mut().enumerate() {
        if !glyph_vertices[i] {
            let index = shifts
                .partition_point(|(x, _)| *x <= vertex.pos.x)
                .saturating_sub(1);
            let shift = if vertex.pos.x >= row.size.x - 0.01 {
                extra
            } else {
                shifts[index].1
            };
            vertex.pos.x += (shift * pixels_per_point).round() / pixels_per_point;
        }
    }
    row.size.x = width;
    row.visuals.mesh_bounds = row
        .visuals
        .mesh
        .vertices
        .iter()
        .fold(Rect::NOTHING, |r, v| {
            r.union(Rect::from_min_max(v.pos, v.pos))
        });
    galley.rect.max.x = width;
    galley.mesh_bounds = row.visuals.mesh_bounds;
}
fn contrast_color(foreground: [u8; 3], background: [u8; 3]) -> [u8; 3] {
    fn luminance(c: [u8; 3]) -> f32 {
        c.into_iter()
            .zip([0.2126, 0.7152, 0.0722])
            .map(|(c, w)| {
                let s = c as f32 / 255.0;
                w * if s <= 0.04045 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            })
            .sum()
    }
    let bg = luminance(background);
    if bg < 0.1 && (foreground == [31; 3] || foreground == [0; 3]) {
        return [223, 227, 235];
    }
    let mut c = foreground;
    let target = if bg > 0.4 { 0.0 } else { 255.0 };
    for step in 0..=20 {
        let fg = luminance(c);
        if (fg.max(bg) + 0.05) / (fg.min(bg) + 0.05) >= 4.5 {
            break;
        }
        let amount = step as f32 / 20.0;
        c = foreground.map(|v| (v as f32 * (1.0 - amount) + target * amount).round() as u8);
    }
    c
}
struct Flow {
    page: usize,
    column: usize,
    columns: usize,
    y: f32,
    top: f32,
    bottom: f32,
}
struct FloatRegion {
    page: usize,
    column: usize,
    rect: Rect,
    side: WrapSide,
}

/// Shape real text rows into the remaining intervals beside anchored images.
/// Offsets remain paragraph character indices; soft breaks consume their newline.
fn floating_paragraph(
    ctx: &Context,
    p: &Paragraph,
    paragraph: (Address, Option<String>),
    flow: &mut Flow,
    floats: &[FloatRegion],
    column: (f32, f32, f32),
    out: &mut Layout,
) {
    let (address, marker) = paragraph;
    let (left, width, stride) = column;
    flow.y += p.before;
    let mut start = 0;
    loop {
        let full = shape_first_row(ctx, p, start, width);
        let height = full.rows[0].size.y.max(p.style.size);
        flow.ensure(height);
        if !floats
            .iter()
            .any(|f| f.page == flow.page && f.column == flow.column && f.rect.bottom() > flow.y)
        {
            // Once clear of the float, consume the already-shaped remainder in
            // one pass rather than reshaping it once for every subsequent line.
            let full = shape_range(ctx, p, start, p.chars.len(), width);
            for row in &full.rows {
                let height = row.size.y.max(p.style.size);
                flow.ensure(height);
                let x = left + flow.column as f32 * stride;
                let shift = match p.alignment {
                    Alignment::Center => (width - row.size.x) * 0.5,
                    Alignment::Right => width - row.size.x,
                    _ => 0.0,
                };
                let end = start + row.glyphs.len();
                out.lines.push(Line {
                    address,
                    start,
                    end,
                    page: flow.page,
                    rect: Rect::from_min_size(
                        pos2(x + shift, flow.y),
                        vec2(row.size.x.max(2.0), height),
                    ),
                    row: row.row.clone(),
                    marker: if start == 0 { marker.clone() } else { None },
                });
                start = end + usize::from(row.ends_with_newline);
                flow.y += height;
            }
            break;
        }
        let x = left + flow.column as f32 * stride;
        let mut intervals = vec![(x, x + width)];
        let mut blocked_until = flow.y;
        for f in floats.iter().filter(|f| {
            f.page == flow.page
                && f.column == flow.column
                && f.rect.top() < flow.y + height
                && f.rect.bottom() > flow.y
        }) {
            blocked_until = blocked_until.max(f.rect.bottom());
            let mut next = vec![];
            for (a, b) in intervals {
                if f.rect.right() <= a || f.rect.left() >= b {
                    next.push((a, b));
                    continue;
                }
                let l = (a, b.min(f.rect.left()));
                let r = (a.max(f.rect.right()), b);
                match f.side {
                    WrapSide::Both => {
                        next.push(l);
                        next.push(r);
                    }
                    WrapSide::Left => next.push(l),
                    WrapSide::Right => next.push(r),
                    WrapSide::Largest => next.push(if l.1 - l.0 >= r.1 - r.0 { l } else { r }),
                }
            }
            intervals = next;
        }
        intervals.retain(|(a, b)| b - a >= 24.0);
        if intervals.is_empty() {
            flow.y = blocked_until;
            continue;
        }
        let mut row_height = height;
        let mut ends_with_newline = false;
        for (a, b) in intervals {
            let g = shape_first_row(ctx, p, start, b - a);
            let row = &g.rows[0];
            let end = start + row.glyphs.len();
            row_height = row_height.max(row.size.y);
            let shift = match p.alignment {
                Alignment::Center => (b - a - row.size.x) * 0.5,
                Alignment::Right => b - a - row.size.x,
                _ => 0.0,
            };
            out.lines.push(Line {
                address,
                start,
                end,
                page: flow.page,
                rect: Rect::from_min_size(
                    pos2(a + shift, flow.y),
                    vec2(row.size.x.max(2.0), row_height),
                ),
                row: row.row.clone(),
                marker: if start == 0 { marker.clone() } else { None },
            });
            start = end + usize::from(row.ends_with_newline);
            ends_with_newline = row.ends_with_newline;
            if start >= p.chars.len() || row.ends_with_newline {
                break;
            }
        }
        flow.y += row_height;
        if start >= p.chars.len() && !ends_with_newline {
            break;
        }
    }
    flow.y += p.after;
}
impl Flow {
    fn ensure(&mut self, height: f32) {
        if self.y + height > self.bottom + 0.01 && self.y > self.top + 0.01 {
            self.next_region();
        }
    }
    fn new_page(&mut self) {
        self.page += 1;
        self.column = 0;
        self.y = self.top;
    }
    fn next_region(&mut self) {
        self.column += 1;
        if self.column >= self.columns {
            self.new_page();
        } else {
            self.y = self.top;
        }
    }
}
impl Layout {
    /// Numbers follow measured body lines, in reading order, never a character estimate.
    /// Omit a number when its measured left gutter would overlap the line's text.
    pub fn numbered_lines<'a>(&'a self, doc: &'a Document) -> Vec<(&'a Line, usize)> {
        if doc.page.pageless || doc.page.line_numbers == LineNumbering::Off {
            return vec![];
        }
        let mut number = 0;
        let mut page = 0;
        self.lines
            .iter()
            .filter(|line| {
                line.rect.left() >= 18.0
                    && matches!(
                        doc.blocks.get(line.address.block),
                        Some(Block::Paragraph(_))
                    )
            })
            .map(|line| {
                if doc.page.line_numbers == LineNumbering::EachPage && line.page != page {
                    number = 0;
                }
                page = line.page;
                number += 1;
                (line, number)
            })
            .collect()
    }
    pub fn measure(ctx: &Context, doc: &Document) -> Self {
        Self::measure_cached(ctx, doc, &mut MeasurementCache::default())
    }
    /// Reuse geometry for unchanged paragraphs. Call `invalidate` when replacing
    /// font definitions; cached rows are measured data and are never painted.
    pub fn measure_cached(ctx: &Context, doc: &Document, cache: &mut MeasurementCache) -> Self {
        cache.begin(ctx, doc);
        let (w, paper_height) = doc.page.size();
        let h = if doc.page.pageless {
            f32::INFINITY
        } else {
            paper_height
        };
        let [top, bottom, left, right] = doc.page.margins;
        let full_width = w - left - right;
        let columns = if doc.page.pageless {
            1
        } else {
            doc.page.columns.max(1) as usize
        };
        let content = (full_width - doc.page.column_gap * (columns - 1) as f32) / columns as f32;
        let header = cache.shape(
            ctx,
            Address {
                block: usize::MAX,
                row: 0,
                cell: 0,
            },
            &doc.page.header,
            full_width,
        );
        let footer = cache.shape(
            ctx,
            Address {
                block: usize::MAX,
                row: 1,
                cell: 0,
            },
            &doc.page.footer,
            full_width,
        );
        let header_height = if doc.page.pageless || doc.page.header.chars.is_empty() {
            0.0
        } else {
            header.rect.height()
        };
        let footer_height = if doc.page.pageless || doc.page.footer.chars.is_empty() {
            0.0
        } else {
            footer.rect.height()
        };
        let top = top.max(if header_height > 0.0 {
            18.0 + header_height + 12.0
        } else {
            0.0
        });
        let bottom = bottom.max(if footer_height > 0.0 {
            18.0 + footer_height + 12.0
        } else {
            0.0
        });
        let mut flow = Flow {
            page: 0,
            column: 0,
            columns,
            y: top,
            top,
            bottom: h - bottom,
        };
        let mut out = Self {
            lines: vec![],
            running: vec![],
            objects: vec![],
            pages: 1,
            size: vec2(w, h),
        };
        let mut number = 0;
        let mut floats: Vec<FloatRegion> = vec![];
        for (block, b) in doc.blocks.iter().enumerate() {
            match b {
                Block::Paragraph(p) => {
                    // Adjacent paragraph spacing collapses to the larger value, as
                    // in Word/Docs. Shared text storage remains untouched.
                    let mut spaced = p.clone();
                    if !p.page_break_before
                        && let Some(Block::Paragraph(previous)) =
                            block.checked_sub(1).and_then(|i| doc.blocks.get(i))
                    {
                        spaced.before = (p.before - previous.after).max(0.0);
                    }
                    let p = &spaced;
                    if !doc.page.pageless
                        && p.page_break_before
                        && (flow.y > top + 0.01 || flow.column > 0)
                    {
                        flow.new_page();
                    }
                    let marker = match p.list {
                        ListKind::Bullet => Some("•".into()),
                        ListKind::Number => {
                            number += 1;
                            Some(format!("{number}."))
                        }
                        ListKind::Check => Some(if p.checked { "☑" } else { "☐" }.into()),
                        ListKind::None => {
                            number = 0;
                            None
                        }
                    };
                    let marker_width = if marker.is_some() { 18.0 } else { 0.0 };
                    let indent = p
                        .indent
                        .clamp(0.0, (content - marker_width - 24.0).max(0.0));
                    let width = (content - indent - marker_width).max(24.0);
                    if floats.iter().any(|f| {
                        f.page == flow.page && f.column == flow.column && f.rect.bottom() > flow.y
                    }) {
                        floating_paragraph(
                            ctx,
                            p,
                            (
                                Address {
                                    block,
                                    row: 0,
                                    cell: 0,
                                },
                                marker,
                            ),
                            &mut flow,
                            &floats,
                            (
                                left + indent + marker_width,
                                width,
                                content + doc.page.column_gap,
                            ),
                            &mut out,
                        );
                        continue;
                    }
                    let g = cache.shape(
                        ctx,
                        Address {
                            block,
                            row: 0,
                            cell: 0,
                        },
                        p,
                        width,
                    );
                    if p.keep_together {
                        flow.ensure(p.before + g.rect.height() + p.after);
                    }
                    if p.widow_control && g.rows.len() > 1 {
                        flow.ensure(
                            p.before + g.rows.iter().take(2).map(|r| r.size.y).sum::<f32>(),
                        );
                    }
                    if p.keep_next
                        && let Some(Block::Paragraph(next)) = doc.blocks.get(block + 1)
                    {
                        let ng = cache.shape(
                            ctx,
                            Address {
                                block: block + 1,
                                row: 0,
                                cell: 0,
                            },
                            next,
                            content - next.indent,
                        );
                        flow.ensure(
                            g.rect.height()
                                + p.before
                                + p.after.max(next.before)
                                + ng.rows.first().map_or(20.0, |r| r.size.y),
                        );
                    }
                    flow.ensure(p.before + g.rows.first().map_or(15.0, |r| r.size.y));
                    flow.y += p.before;
                    let mut start = 0;
                    for (idx, row) in g.rows.iter().enumerate() {
                        let height = row.size.y.max(p.style.size);
                        if p.widow_control && g.rows.len() - idx == 2 {
                            flow.ensure(
                                g.rows[idx..]
                                    .iter()
                                    .map(|r| r.size.y.max(p.style.size))
                                    .sum(),
                            );
                        }
                        flow.ensure(height);
                        let x = left
                            + flow.column as f32 * (content + doc.page.column_gap)
                            + indent
                            + marker_width;
                        let shift = match p.alignment {
                            Alignment::Center => (width - row.size.x) * 0.5,
                            Alignment::Right => width - row.size.x,
                            _ => 0.0,
                        };
                        let end = start + row.glyphs.len();
                        out.lines.push(Line {
                            address: Address {
                                block,
                                row: 0,
                                cell: 0,
                            },
                            start,
                            end,
                            page: flow.page,
                            rect: Rect::from_min_size(
                                pos2(x + shift, flow.y),
                                vec2(row.size.x.max(2.0), height),
                            ),
                            row: g.rows[idx].row.clone(),
                            marker: if idx == 0 { marker.clone() } else { None },
                        });
                        flow.y += height;
                        start = end + usize::from(row.ends_with_newline);
                    }
                    flow.y += p.after;
                }
                Block::PageBreak => {
                    if !doc.page.pageless {
                        flow.new_page();
                    }
                }
                Block::Rule => {
                    flow.ensure(12.0);
                    out.objects.push(Object::Rule {
                        page: flow.page,
                        rect: Rect::from_min_size(
                            pos2(
                                left + flow.column as f32 * (content + doc.page.column_gap),
                                flow.y + 5.0,
                            ),
                            vec2(content, 1.0),
                        ),
                    });
                    flow.y += 12.0;
                }
                Block::Image(image) => {
                    let (width, height) = image.visible_size();
                    let ratio = (content / width).min(1.0).min((flow.bottom - top) / height);
                    let size = vec2(width * ratio, height * ratio);
                    let placement = image.placement;
                    let before = if placement.flow == ImageFlow::Break {
                        placement.margins[0]
                    } else {
                        0.0
                    };
                    flow.ensure(size.y + placement.offset[1].max(0.0) + before);
                    let column_left = left + flow.column as f32 * (content + doc.page.column_gap);
                    let rect = Rect::from_min_size(
                        pos2(
                            column_left
                                + placement.offset[0].clamp(0.0, (content - size.x).max(0.0)),
                            (flow.y + placement.offset[1] + before)
                                .clamp(top, (flow.bottom - size.y).max(top)),
                        ),
                        size,
                    );
                    out.objects.push(Object::Image {
                        block,
                        page: flow.page,
                        rect,
                        anchor: pos2(column_left, flow.y + before),
                    });
                    match placement.flow {
                        ImageFlow::Break => {
                            flow.y = rect.bottom() + placement.margins[1];
                        }
                        ImageFlow::Wrap => {
                            let [top, bottom, left, right] = placement.margins;
                            floats.push(FloatRegion {
                                page: flow.page,
                                column: flow.column,
                                rect: Rect::from_min_max(
                                    rect.min - vec2(left, top),
                                    rect.max + vec2(right, bottom),
                                ),
                                side: placement.wrap_side,
                            });
                        }
                        ImageFlow::Behind | ImageFlow::Front => {}
                    }
                }
                Block::Table(rows) => {
                    if let Some(bottom) = floats
                        .iter()
                        .filter(|f| {
                            f.page == flow.page
                                && f.column == flow.column
                                && f.rect.bottom() > flow.y
                        })
                        .map(|f| f.rect.bottom())
                        .max_by(f32::total_cmp)
                    {
                        flow.y = bottom;
                    }
                    for (ri, cells) in rows.iter().enumerate() {
                        if cells.is_empty() {
                            continue;
                        }
                        let cw = content / cells.len() as f32;
                        let padding: Vec<_> = cells
                            .iter()
                            .map(|p| {
                                let marker = if p.list == ListKind::None { 0.0 } else { 18.0 };
                                marker + p.indent.clamp(0.0, (cw - 12.0 - marker - 24.0).max(0.0))
                            })
                            .collect();
                        let galleys: Vec<_> = cells
                            .iter()
                            .enumerate()
                            .map(|(ci, p)| {
                                cache.shape(
                                    ctx,
                                    Address {
                                        block,
                                        row: ri,
                                        cell: ci,
                                    },
                                    p,
                                    cw - 12.0 - padding[ci],
                                )
                            })
                            .collect();
                        let whole_height = cells
                            .iter()
                            .zip(&galleys)
                            .map(|(p, g)| p.before + g.rect.height() + p.after)
                            .fold(0.0, f32::max)
                            + 12.0;
                        flow.ensure(whole_height.min(flow.bottom - flow.top));
                        let mut indices = vec![0; cells.len()];
                        let mut offsets = vec![0; cells.len()];
                        // Cells flow independently: paragraph spacing or different fonts in
                        // one cell must not overlap later lines or synchronize unrelated rows.
                        loop {
                            let first_y = flow.y;
                            let page = flow.page;
                            let available = (flow.bottom - first_y - 12.0).max(1.0);
                            let mut used_height = 14.0f32;
                            for (ci, g) in galleys.iter().enumerate() {
                                if indices[ci] >= g.rows.len() {
                                    continue;
                                }
                                let p = &cells[ci];
                                let first_height = g.rows[indices[ci]].size.y.max(p.style.size);
                                let mut used = if indices[ci] == 0 {
                                    p.before.min((available - first_height).max(0.0))
                                } else {
                                    0.0
                                };
                                let mut taken = 0;
                                while let Some(row) = g.rows.get(indices[ci]) {
                                    let height = row.size.y.max(p.style.size);
                                    if used + height > available + 0.01 && taken > 0 {
                                        break;
                                    }
                                    let width = (cw - 12.0 - padding[ci]).max(24.0);
                                    let shift = match p.alignment {
                                        Alignment::Center => (width - row.size.x) * 0.5,
                                        Alignment::Right => width - row.size.x,
                                        _ => 0.0,
                                    };
                                    let start = offsets[ci];
                                    let end = start + row.glyphs.len();
                                    let marker = if indices[ci] == 0 {
                                        match p.list {
                                            ListKind::Bullet => Some("•".into()),
                                            ListKind::Number => Some("1.".into()),
                                            ListKind::Check => {
                                                Some(if p.checked { "☑" } else { "☐" }.into())
                                            }
                                            ListKind::None => None,
                                        }
                                    } else {
                                        None
                                    };
                                    out.lines.push(Line {
                                        address: Address {
                                            block,
                                            row: ri,
                                            cell: ci,
                                        },
                                        start,
                                        end,
                                        page,
                                        rect: Rect::from_min_size(
                                            pos2(
                                                left + flow.column as f32
                                                    * (content + doc.page.column_gap)
                                                    + ci as f32 * cw
                                                    + 6.0
                                                    + padding[ci]
                                                    + shift,
                                                first_y + 6.0 + used,
                                            ),
                                            vec2(row.size.x.max(2.0), height),
                                        ),
                                        row: g.rows[indices[ci]].row.clone(),
                                        marker,
                                    });
                                    offsets[ci] = end + usize::from(row.ends_with_newline);
                                    indices[ci] += 1;
                                    taken += 1;
                                    used += height;
                                }
                                if indices[ci] >= g.rows.len() {
                                    used += p.after.min((available - used).max(0.0));
                                }
                                used_height = used_height.max(used);
                            }
                            flow.y = first_y + used_height + 12.0;
                            for ci in 0..cells.len() {
                                out.objects.push(Object::Border {
                                    page,
                                    address: Address {
                                        block,
                                        row: ri,
                                        cell: ci,
                                    },
                                    rect: Rect::from_min_size(
                                        pos2(
                                            left + flow.column as f32
                                                * (content + doc.page.column_gap)
                                                + ci as f32 * cw,
                                            first_y,
                                        ),
                                        vec2(cw, flow.y - first_y),
                                    ),
                                });
                            }
                            if indices
                                .iter()
                                .zip(&galleys)
                                .all(|(index, g)| *index >= g.rows.len())
                            {
                                break;
                            }
                            flow.next_region();
                        }
                    }
                    flow.y += 8.0;
                }
            }
        }
        out.pages = flow.page + 1;
        if doc.page.pageless {
            out.size.y = flow.y.max(
                out.lines
                    .iter()
                    .map(|l| l.rect.bottom())
                    .fold(0.0, f32::max),
            ) + bottom;
            out.size.y = out.size.y.max(300.0);
            for object in &out.objects {
                let rect = match object {
                    Object::Image { rect, .. }
                    | Object::Border { rect, .. }
                    | Object::Rule { rect, .. } => rect,
                };
                out.size.y = out.size.y.max(rect.bottom() + bottom);
            }
        }
        for page in 0..if doc.page.pageless { 0 } else { out.pages } {
            for (row, p, g, y) in [
                (0, &doc.page.header, &header, 18.0),
                (1, &doc.page.footer, &footer, h - 18.0 - footer_height),
            ] {
                if p.chars.is_empty() {
                    continue;
                }
                let mut start = 0;
                for (index, r) in g.rows.iter().enumerate() {
                    let shift = match p.alignment {
                        Alignment::Center => (full_width - r.size.x) * 0.5,
                        Alignment::Right => full_width - r.size.x,
                        _ => 0.0,
                    };
                    let end = start + r.glyphs.len();
                    out.running.push(Line {
                        address: Address {
                            block: usize::MAX,
                            row,
                            cell: 0,
                        },
                        start,
                        end,
                        page,
                        rect: Rect::from_min_size(pos2(left + shift, y + r.pos.y), r.size),
                        row: g.rows[index].row.clone(),
                        marker: None,
                    });
                    start = end + usize::from(r.ends_with_newline);
                }
            }
        }
        cache
            .paragraphs
            .retain(|_, (_, _, generation)| *generation == cache.generation);
        out
    }
    pub fn caret_line(&self, pos: Position) -> Option<&Line> {
        self.lines
            .iter()
            .rfind(|l| l.address == pos.address && pos.offset >= l.start && pos.offset <= l.end)
            .or_else(|| self.lines.iter().find(|l| l.address == pos.address))
    }
    pub fn hit(&self, page: usize, point: Pos2) -> Option<Position> {
        let l = self.hit_line(page, point)?;
        Some(Position {
            address: l.address,
            offset: l.hit(point.x - l.rect.left()),
        })
    }
    pub fn hit_line(&self, page: usize, point: Pos2) -> Option<&Line> {
        let cell = self.objects.iter().find_map(|object| match object {
            Object::Border {
                page: p,
                rect,
                address,
            } if *p == page && rect.contains(point) => Some(*address),
            _ => None,
        });
        self.lines
            .iter()
            .filter(|l| l.page == page && cell.is_none_or(|address| l.address == address))
            .min_by(|a, b| {
                let distance = |l: &Line| {
                    let dy = if point.y < l.rect.top() {
                        l.rect.top() - point.y
                    } else if point.y > l.rect.bottom() {
                        point.y - l.rect.bottom()
                    } else {
                        0.0
                    };
                    let dx = if point.x < l.rect.left() {
                        l.rect.left() - point.x
                    } else if point.x > l.rect.right() {
                        point.x - l.rect.right()
                    } else {
                        0.0
                    };
                    dy * 1000.0 + dx
                };
                distance(a).total_cmp(&distance(b))
            })
    }
    pub fn vertical(&self, pos: Position, down: bool) -> Position {
        let Some(l) = self.caret_line(pos) else {
            return pos;
        };
        let index = self
            .lines
            .iter()
            .position(|n| std::ptr::eq(n, l))
            .unwrap_or(0);
        let in_table = self
            .objects
            .iter()
            .any(|o| matches!(o,Object::Border{address,..} if *address==pos.address));
        let allowed = |n: &Line| {
            !in_table || n.address.block != pos.address.block || n.address.cell == pos.address.cell
        };
        let next = if down {
            (index + 1..self.lines.len()).find(|i| allowed(&self.lines[*i]))
        } else {
            (0..index).rev().find(|i| allowed(&self.lines[*i]))
        };
        if let Some(i) = next {
            let n = &self.lines[i];
            Position {
                address: n.address,
                offset: n.hit(l.rect.left() + l.x(pos.offset) - n.rect.left()),
            }
        } else {
            pos
        }
    }
    pub fn boundary(&self, pos: Position, end: bool) -> Position {
        if let Some(l) = self.caret_line(pos) {
            Position {
                address: pos.address,
                offset: if end { l.end } else { l.start },
            }
        } else {
            pos
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjacent_paragraph_spacing_uses_maximum_without_mutating_metadata() {
        let mut doc = Document::new("Spacing");
        let mut a = Paragraph::from_text("First");
        a.after = 12.0;
        let mut b = Paragraph::from_text("Second");
        b.before = 18.0;
        doc.blocks = vec![Block::Paragraph(a), Block::Paragraph(b)];
        let layout = measured(&doc);
        assert!((layout.lines[1].rect.top() - layout.lines[0].rect.bottom() - 18.0).abs() < 0.01);
        assert_eq!(
            doc.paragraph(Address {
                block: 1,
                ..Default::default()
            })
            .unwrap()
            .before,
            18.0
        );
    }
    #[test]
    fn display_text_retains_native_kerning_and_pixel_positions_at_each_zoom() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        let mut doc = Document::new("Raster positions");
        doc.blocks = vec![Block::Paragraph(Paragraph::from_text(
            "AV office Café — narrow words",
        ))];
        let layout = Layout::measure(&ctx, &doc);
        let line = &layout.lines[0];
        for zoom in [1.0, 1.25, 1.75] {
            let scale = 4.0 / 3.0 * zoom;
            let display = display_line(&ctx, &doc, line, scale, [255; 3]);
            let mut p = doc.paragraph(line.address).unwrap().clone();
            p.style.size *= scale;
            for c in &mut p.chars {
                c.style.size *= scale;
            }
            let native = shape(&ctx, &p, f32::INFINITY);
            let actual: Vec<_> = display.rows[0]
                .visuals
                .mesh
                .vertices
                .iter()
                .map(|v| v.pos)
                .collect();
            let expected: Vec<_> = native.rows[0]
                .visuals
                .mesh
                .vertices
                .iter()
                .map(|v| v.pos)
                .collect();
            assert_eq!(
                actual, expected,
                "Display must not fractionally reposition rasterized glyphs"
            );
            for (i, glyph) in display.rows[0].glyphs.iter().enumerate() {
                assert_eq!(display_x(&display, line.start, i), glyph.pos.x);
                if glyph.advance_width > 0.0 {
                    assert_eq!(
                        display_hit(
                            &display,
                            line.start,
                            line.end,
                            glyph.pos.x + glyph.advance_width * 0.25
                        ),
                        i
                    );
                }
            }
        }
    }
    #[test]
    fn justified_display_expands_spaces_without_distorting_word_kerning() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        let mut doc = Document::new("Justified raster");
        let mut p = Paragraph::from_text(&"Alpha office narrow letters ".repeat(8));
        p.alignment = Alignment::Justified;
        doc.blocks = vec![Block::Paragraph(p)];
        let layout = Layout::measure(&ctx, &doc);
        let line = &layout.lines[0];
        let scale = 4.0 / 3.0;
        let display = display_line(&ctx, &doc, line, scale, [255; 3]);
        assert!((display.rows[0].size.x - line.rect.width() * scale).abs() < 0.01);
        let mut natural_doc = doc.clone();
        natural_doc.paragraph_mut(line.address).unwrap().alignment = Alignment::Left;
        let natural = display_line(&ctx, &natural_doc, line, scale, [255; 3]);
        for (a, b) in display.rows[0]
            .glyphs
            .windows(2)
            .zip(natural.rows[0].glyphs.windows(2))
        {
            if b[0].chr != ' ' {
                assert!(((a[1].pos.x - a[0].pos.x) - (b[1].pos.x - b[0].pos.x)).abs() < 0.001);
            }
        }
    }
    #[test]
    fn table_cells_respect_independent_spacing_lists_and_pagination() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut first = Paragraph::from_text("Bullet cell");
        first.list = ListKind::Bullet;
        first.after = 0.0;
        let mut second = Paragraph::from_text(&"A long independently flowing cell. ".repeat(100));
        second.before = 24.0;
        second.after = 16.0;
        let mut doc = Document::new("Cell layout");
        doc.blocks = vec![Block::Table(vec![vec![first, second]])];
        let mut output = ctx.run_ui(Default::default(), |_| {
            let layout = Layout::measure(&ctx, &doc);
            let first = layout.lines.iter().find(|l| l.address.cell == 0).unwrap();
            let second = layout.lines.iter().find(|l| l.address.cell == 1).unwrap();
            assert_eq!(first.marker.as_deref(), Some("•"));
            assert!((second.rect.top() - first.rect.top() - 24.0).abs() < 0.01);
            assert!(layout.pages > 1);
            for line in &layout.lines {
                assert!(line.rect.bottom() <= doc.page.size().1 - doc.page.margins[1] + 0.1);
            }
            for lines in layout.lines.windows(2) {
                if lines[0].address == lines[1].address && lines[0].page == lines[1].page {
                    assert!(lines[1].rect.top() >= lines[0].rect.bottom() - 0.01);
                }
            }
        });
        output.textures_delta.clear();
    }
    #[test]
    fn empty_table_cells_hit_their_full_rectangle() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut d = Document::new("Hit table");
        d.blocks = vec![Block::Table(vec![vec![Paragraph::default(); 3]; 2])];
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
            let layout = Layout::measure(&ctx, &d);
            for object in &layout.objects {
                if let Object::Border {
                    page,
                    rect,
                    address,
                } = object
                {
                    for point in [rect.center(), rect.right_bottom() - vec2(2.0, 2.0)] {
                        assert_eq!(layout.hit(*page, point).unwrap().address, *address);
                    }
                }
            }
        });
        output.textures_delta.clear();
    }
    #[test]
    fn columns_and_running_text_use_measured_regions() {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut d = Document::new("Columns");
        d.page.columns = 2;
        d.page.header = Paragraph::from_text("Repeated header");
        d.page.footer = Paragraph::from_text("Repeated footer");
        d.blocks = (0..80)
            .map(|_| {
                Block::Paragraph(Paragraph::from_text(
                    "Measured text flows down the left column and then into the right column.",
                ))
            })
            .collect();
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
            let layout = Layout::measure(&ctx, &d);
            assert!(layout.pages > 1);
            let first: Vec<_> = layout.lines.iter().filter(|l| l.page == 0).collect();
            assert!(first.iter().any(|l| l.rect.left() > d.page.size().0 / 2.0));
            assert_eq!(layout.running.len(), layout.pages * 2);
            for line in &layout.running {
                assert!(line.rect.bottom() < d.page.size().1);
            }
        });
        output.textures_delta.clear();
    }
    fn measured(doc: &Document) -> Layout {
        let ctx = Context::default();
        fonts::install(&ctx);
        let mut result = None;
        let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
            result = Some(Layout::measure(&ctx, doc));
        });
        output.textures_delta.clear();
        result.unwrap()
    }
    #[test]
    fn pagination_uses_font_measurements() {
        let mut narrow = Document::new("measure");
        narrow.blocks = vec![Block::Paragraph(Paragraph::from_text(&"iiii ".repeat(600)))];
        let mut wide = narrow.clone();
        if let Block::Paragraph(p) = &mut wide.blocks[0] {
            *p = Paragraph::from_text(&"WWWW ".repeat(600));
        }
        let a = measured(&narrow);
        let b = measured(&wide);
        assert!(b.pages > a.pages);
        assert!(b.lines.len() > a.lines.len());
        for l in b.lines {
            assert!(l.rect.bottom() <= wide.page.size().1 - wide.page.margins[1] + 0.1);
        }
    }

    #[test]
    fn every_paper_preset_measures_with_both_orientations() {
        for paper in Paper::STANDARD {
            for landscape in [false, true] {
                let mut doc = Document::new("Paper measurement");
                doc.page.paper = paper;
                doc.page.landscape = landscape;
                doc.page.margins = [30.0; 4];
                doc.blocks = vec![Block::Paragraph(Paragraph::from_text(
                    &"Measured typography follows the selected physical page size. ".repeat(90),
                ))];
                let layout = measured(&doc);
                let (width, height) = doc.page.size();
                assert_eq!(layout.size.x, width, "{paper:?}, landscape={landscape}");
                assert_eq!(layout.size.y, height, "{paper:?}, landscape={landscape}");
                assert!(layout.pages >= 1);
                assert!(!layout.lines.is_empty());
                assert!(
                    layout.lines.iter().all(|line| {
                        line.rect.left() >= doc.page.margins[2] - 0.1
                            && line.rect.right() <= width - doc.page.margins[3] + 1.0
                            && line.rect.bottom() <= height - doc.page.margins[1] + 1.0
                    }),
                    "{paper:?}, landscape={landscape}, page=({width}, {height}), margins={:?}, bad geometry: {:?}",
                    doc.page.margins,
                    layout
                        .lines
                        .iter()
                        .find(|line| {
                            line.rect.left() < doc.page.margins[2] - 0.1
                                || line.rect.right() > width - doc.page.margins[3] + 0.1
                                || line.rect.bottom() > height - doc.page.margins[1] + 0.1
                        })
                        .map(|line| (line.page, line.rect))
                );
            }
        }
    }

    #[test]
    fn pageless_geometry_is_finite_and_wraps_around_floats() {
        let mut doc = Document::new("Pageless geometry");
        doc.page.pageless = true;
        doc.page.paper = Paper::Custom {
            width: 420.0,
            height: 600.0,
        };
        doc.page.landscape = true;
        doc.page.margins = [36.0; 4];
        doc.page.header = Paragraph::from_text("Hidden in pageless mode");
        doc.page.footer = Paragraph::from_text("Also hidden");
        let image = EmbeddedImage {
            name: "Float".into(),
            width: 140.0,
            height: 110.0,
            placement: ImagePlacement {
                flow: ImageFlow::Wrap,
                margins: [4.0; 4],
                ..Default::default()
            },
            ..Default::default()
        };
        doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text("Before the floating image.")),
            Block::Image(image),
            Block::Paragraph(Paragraph::from_text(
                &"Text wraps beside the image, then continues beneath it. ".repeat(12),
            )),
        ];
        let layout = measured(&doc);
        let image_rect = layout
            .objects
            .iter()
            .find_map(|object| match object {
                Object::Image { rect, .. } => Some(*rect),
                _ => None,
            })
            .unwrap();
        assert_eq!(layout.pages, 1);
        assert!(layout.size.x.is_finite() && layout.size.y.is_finite());
        assert!(layout.size.y > image_rect.bottom());
        assert!(layout.running.is_empty());
        assert!(
            layout
                .lines
                .iter()
                .all(|line| line.rect.right() <= layout.size.x)
        );
        assert!(
            layout
                .lines
                .iter()
                .any(|line| { line.address.block == 2 && line.rect.top() < image_rect.bottom() })
        );
        assert!(
            layout
                .lines
                .iter()
                .any(|line| { line.address.block == 2 && line.rect.top() >= image_rect.bottom() })
        );
    }

    #[test]
    fn measured_line_numbers_skip_tables_and_restart_each_page() {
        let mut doc = Document::new("Line numbering");
        doc.page.line_numbers = LineNumbering::EachPage;
        doc.page.header = Paragraph::from_text("Header line is not numbered");
        doc.blocks = vec![
            Block::Paragraph(Paragraph::from_text(&"body line ".repeat(400))),
            Block::Table(vec![vec![Paragraph::from_text("table line")]]),
            Block::Paragraph(Paragraph::from_text("after table")),
        ];
        let layout = measured(&doc);
        let numbered = layout.numbered_lines(&doc);
        assert!(layout.pages > 1);
        assert!(!layout.running.is_empty());
        assert!(
            numbered.iter().all(|(line, _)| {
                line.address.block != 1 && line.address.block < doc.blocks.len()
            })
        );
        let mut prior_page = None;
        let mut expected = 0;
        for (line, number) in numbered {
            if prior_page != Some(line.page) {
                expected = 0;
                prior_page = Some(line.page);
            }
            expected += 1;
            assert_eq!(number, expected);
        }

        doc.page.margins[2] = 0.0;
        let layout = measured(&doc);
        assert!(layout.numbered_lines(&doc).is_empty());
    }
    #[test]
    fn explicit_break_and_table_flow() {
        let mut doc = Document::new("test");
        doc.blocks.push(Block::PageBreak);
        doc.blocks.push(Block::Table(vec![
            vec![
                Paragraph::from_text("Table cell");
                3
            ];
            45
        ]));
        let result = measured(&doc);
        assert!(result.pages >= 3);
        assert!(
            result
                .lines
                .iter()
                .filter(|l| l.address.block == 2)
                .all(|l| l.page >= 1)
        );
    }
}
