//! Vector output for the deliberately small generated drawing vocabulary.
//! Reuses resvg's native font/outline resolution; no HTML renderer is needed.
use super::*;
use resvg::{tiny_skia, usvg};
pub(super) fn render(
    object: &EditableObject,
    rect: eframe::egui::Rect,
    h: f32,
) -> Result<Vec<Op>, String> {
    crate::editable_objects::validate(object)?;
    let mut options = usvg::Options::default();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    for bytes in crate::editable_objects::source_fonts(&object.source).into_values() {
        options.fontdb_mut().load_font_data(bytes);
    }
    let tree = usvg::Tree::from_str(&object.svg, &options).map_err(|e| e.to_string())?;
    let mut ops = vec![Op::SaveGraphicsState];
    let scale = eframe::egui::vec2(
        rect.width() / tree.size().width(),
        rect.height() / tree.size().height(),
    );
    let mut budget = 200_000;
    group(tree.root(), rect, h, scale, &mut ops, &mut budget)?;
    ops.push(Op::RestoreGraphicsState);
    Ok(ops)
}
fn group(
    group: &usvg::Group,
    rect: eframe::egui::Rect,
    h: f32,
    scale: eframe::egui::Vec2,
    ops: &mut Vec<Op>,
    budget: &mut usize,
) -> Result<(), String> {
    if group.clip_path().is_some() || group.mask().is_some() || group.opacity().get() != 1.0 {
        return Err("Unsupported generated vector compositing".into());
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => self::group(g, rect, h, scale, ops, budget)?,
            usvg::Node::Text(text) => self::group(text.flattened(), rect, h, scale, ops, budget)?,
            usvg::Node::Image(_) => {
                return Err("Images are not accepted in editable vector objects".into());
            }
            usvg::Node::Path(path) => {
                let transform = path.abs_transform();
                let convert = |mut p: tiny_skia::Point, bezier| {
                    transform.map_point(&mut p);
                    LinePoint {
                        p: point(rect.left() + p.x * scale.x, h - rect.top() - p.y * scale.y),
                        bezier,
                    }
                };
                let mut rings = vec![];
                let mut current = vec![];
                let mut previous = tiny_skia::Point::from_xy(0.0, 0.0);
                let mut closed = false;
                for segment in path.data().segments() {
                    if *budget == 0 {
                        return Err("Generated vector is too complex".into());
                    }
                    *budget -= 1;
                    match segment {
                        tiny_skia::PathSegment::MoveTo(p) => {
                            if !current.is_empty() {
                                rings.push((std::mem::take(&mut current), closed));
                            }
                            closed = false;
                            current.push(convert(p, false));
                            previous = p;
                        }
                        tiny_skia::PathSegment::LineTo(p) => {
                            current.push(convert(p, false));
                            previous = p;
                        }
                        tiny_skia::PathSegment::QuadTo(control, end) => {
                            let a = tiny_skia::Point::from_xy(
                                previous.x + (control.x - previous.x) * 2.0 / 3.0,
                                previous.y + (control.y - previous.y) * 2.0 / 3.0,
                            );
                            let b = tiny_skia::Point::from_xy(
                                end.x + (control.x - end.x) * 2.0 / 3.0,
                                end.y + (control.y - end.y) * 2.0 / 3.0,
                            );
                            current.extend([
                                convert(a, true),
                                convert(b, true),
                                convert(end, false),
                            ]);
                            previous = end;
                        }
                        tiny_skia::PathSegment::CubicTo(a, b, end) => {
                            current.extend([
                                convert(a, true),
                                convert(b, true),
                                convert(end, false),
                            ]);
                            previous = end;
                        }
                        tiny_skia::PathSegment::Close => closed = true,
                    }
                }
                if !current.is_empty() {
                    rings.push((current, closed));
                }
                if let Some(fill) = path.fill() {
                    let usvg::Paint::Color(c) = fill.paint() else {
                        return Err("Unsupported generated vector paint".into());
                    };
                    if fill.opacity().get() != 1.0 {
                        return Err("Unsupported generated vector opacity".into());
                    }
                    ops.push(Op::SetFillColor {
                        col: rgb([c.red, c.green, c.blue]),
                    });
                    ops.push(Op::DrawPolygon {
                        polygon: Polygon {
                            rings: rings
                                .iter()
                                .map(|(points, _)| PolygonRing {
                                    points: points.clone(),
                                })
                                .collect(),
                            mode: PaintMode::Fill,
                            winding_order: if fill.rule() == usvg::FillRule::EvenOdd {
                                WindingOrder::EvenOdd
                            } else {
                                WindingOrder::NonZero
                            },
                        },
                    });
                }
                if let Some(stroke) = path.stroke() {
                    let usvg::Paint::Color(c) = stroke.paint() else {
                        return Err("Unsupported generated vector stroke".into());
                    };
                    if stroke.opacity().get() != 1.0 {
                        return Err("Unsupported generated vector opacity".into());
                    }
                    let effective = (transform.sx * transform.sy - transform.kx * transform.ky)
                        .abs()
                        .sqrt();
                    ops.push(Op::SetOutlineColor {
                        col: rgb([c.red, c.green, c.blue]),
                    });
                    ops.push(Op::SetOutlineThickness {
                        pt: Pt(stroke.width().get() * effective * (scale.x * scale.y).sqrt()),
                    });
                    for (points, closed) in rings {
                        ops.push(Op::DrawLine {
                            line: Line {
                                points,
                                is_closed: closed,
                            },
                        });
                    }
                }
            }
        }
    }
    Ok(())
}
