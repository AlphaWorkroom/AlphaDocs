use super::*;

fn metadata(image: &EmbeddedImage) -> String {
    format!(
        "alphadocs:objectId=\"{}\" alphadocs:comments=\"{}\" alphadocs:reactions=\"{}\"",
        esc(&image.id),
        esc(&serde_json::to_string(&image.comments).expect("Comments serialize")),
        esc(&serde_json::to_string(&image.reactions).expect("Reactions serialize"))
    )
}
pub(super) fn write(writer: &mut Writer, image: &EmbeddedImage) -> Result<String, String> {
    let object = image
        .editable
        .as_ref()
        .ok_or("Missing editable object source")?;
    crate::editable_objects::validate(object)?;
    match &object.source {
        ObjectSource::Equation {
            latex,
            size,
            colour,
        } => {
            let math = crate::editable_objects::equation_omml(latex)?;
            // Word requires explicit run formatting inside math; paragraph
            // defaults alone leave equations at its default 12 pt.
            let style = TextStyle {
                font: "Cambria Math".into(),
                size: *size,
                color: *colour,
                ..Default::default()
            };
            let run_properties = parse(&rpr(&style))?.children.remove(0);
            let mut tree = parse(&math)?;
            fn format_math(node: &mut Node, properties: &Node) {
                if node.qualified_name == "m:r" {
                    let index = usize::from(
                        node.children
                            .first()
                            .is_some_and(|n| n.qualified_name == "m:rPr"),
                    );
                    node.children.insert(index, properties.clone());
                }
                if node.name == "oMathPara" {
                    let properties = parse("<m:oMathParaPr><m:jc m:val=\"left\"/></m:oMathParaPr>")
                        .expect("Fixed XML")
                        .children
                        .remove(0);
                    node.children.retain(|n| n.name != "oMathParaPr");
                    node.children.insert(0, properties);
                }
                for child in &mut node.children {
                    format_math(child, properties);
                }
            }
            format_math(&mut tree, &run_properties);
            let root = tree.children.first().ok_or("Missing generated equation")?;
            let mut math = node_xml(root, true);
            if root.name == "oMath" {
                math = format!(
                    "<m:oMathPara xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:oMathParaPr><m:jc m:val=\"left\"/></m:oMathParaPr>{math}</m:oMathPara>"
                );
            }
            Ok(format!(
                "<w:p {}><w:pPr><w:spacing w:before=\"{}\"/><w:ind w:left=\"{}\"/><w:jc w:val=\"left\"/>{}</w:pPr>{math}</w:p>",
                metadata(image),
                twips(image.placement.offset[1].max(0.0)),
                twips(image.placement.offset[0].max(0.0)),
                rpr(&style)
            ))
        }
        ObjectSource::Drawing {
            kind,
            text,
            fill,
            border,
        } => {
            writer.counter += 1;
            let id = writer.counter;
            let (w, h) = image.visible_size();
            let cx = (w * 12700.0).round() as i64;
            let cy = (h * 12700.0).round() as i64;
            let x = (image.placement.offset[0] * 12700.0).round() as i64;
            let y = (image.placement.offset[1] * 12700.0).round() as i64;
            let geometry = match kind {
                ShapeKind::Ellipse => "ellipse",
                ShapeKind::Line | ShapeKind::Arrow => "line",
                _ => "rect",
            };
            let fill = fill
                .map(|c| format!("<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>", rgb(c)))
                .unwrap_or_else(|| "<a:noFill/>".into());
            let line = if border.width == 0.0 {
                "<a:ln><a:noFill/></a:ln>".into()
            } else {
                format!(
                    "<a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>{}</a:ln>",
                    (border.width * 12700.0).round() as i64,
                    rgb(border.colour),
                    if *kind == ShapeKind::Arrow {
                        "<a:tailEnd type=\"triangle\"/>"
                    } else {
                        ""
                    }
                )
            };
            let line_shape = matches!(kind, ShapeKind::Line | ShapeKind::Arrow);
            let anchor_y = if line_shape { y + cy / 2 } else { y };
            let shape_height = if line_shape { 0 } else { cy };
            let paragraph_properties = if line_shape {
                format!(
                    "<w:pPr><w:spacing w:line=\"{}\" w:lineRule=\"exact\"/></w:pPr>",
                    twips(h)
                )
            } else {
                String::new()
            };
            let inset_x = if *kind == ShapeKind::Ellipse {
                (w * 0.15 + 8.0) * 12700.0
            } else {
                101600.0
            } as i64;
            let inset_y = if *kind == ShapeKind::Ellipse {
                (h * 0.15 + 8.0) * 12700.0
            } else {
                101600.0
            } as i64;
            let text =
                if matches!(kind, ShapeKind::Line | ShapeKind::Arrow) && text.chars.is_empty() {
                    String::new()
                } else {
                    format!(
                        "<wps:txbx><w:txbxContent>{}</w:txbxContent></wps:txbx>",
                        writer.paragraph(text)
                    )
                };
            let wrapping = match image.placement.flow {
                ImageFlow::Wrap => "<wp:wrapSquare wrapText=\"bothSides\"/>",
                ImageFlow::Break => "<wp:wrapTopAndBottom/>",
                _ => "<wp:wrapNone/>",
            };
            Ok(format!(
                "<w:p>{paragraph_properties}<w:r><w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\" simplePos=\"0\" relativeHeight=\"0\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"column\"><wp:posOffset>{x}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"paragraph\"><wp:posOffset>{anchor_y}</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{shape_height}\"/>{wrapping}<wp:docPr id=\"{id}\" name=\"{}\" alphadocs:lineBoxHeight=\"{h}\" {}/><a:graphic><a:graphicData uri=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:wsp xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><wps:cNvSpPr txBox=\"{}\"/><wps:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{shape_height}\"/></a:xfrm><a:prstGeom prst=\"{geometry}\"><a:avLst/></a:prstGeom>{fill}{line}</wps:spPr>{text}<wps:bodyPr lIns=\"{inset_x}\" tIns=\"{inset_y}\" rIns=\"{inset_x}\" bIns=\"{inset_y}\"><a:spAutoFit/></wps:bodyPr></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>",
                u8::from(image.placement.flow == ImageFlow::Behind),
                esc(kind.label()),
                metadata(image),
                u8::from(*kind == ShapeKind::TextBox)
            ))
        }
    }
}

fn node_xml(node: &Node, root: bool) -> String {
    let name = &node.qualified_name;
    let mut out = format!("<{name}");
    if root {
        out.push_str(" xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"");
    }
    for (key, value) in &node.qualified_attrs {
        if !(root && matches!(key.as_str(), "xmlns:m" | "xmlns:w")) {
            out.push_str(&format!(" {key}=\"{}\"", esc(value)));
        }
    }
    out.push('>');
    out.push_str(&esc(&node.text));
    for child in &node.children {
        out.push_str(&node_xml(child, false));
    }
    out.push_str(&format!("</{name}>"));
    out
}
pub(super) fn restore_metadata(image: &mut EmbeddedImage, properties: Option<&Node>) {
    if let Some(properties) = properties {
        if let Some(id) = properties.attr("objectId") {
            image.id = id.into();
        }
        image.comments = properties
            .attr("comments")
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        image.reactions = properties
            .attr("reactions")
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
    }
}
pub(super) fn read_math(
    node: &Node,
    p: &Paragraph,
    ctx: &eframe::egui::Context,
) -> Result<EmbeddedImage, String> {
    let math = node.descendant("oMath").ok_or("Missing Office equation")?;
    let latex = crate::editable_objects::latex_from_omml(&node_xml(math, true))?;
    let mut style = p.style.clone();
    let mut properties = Vec::new();
    math.descendants("rPr", &mut properties);
    if let Some(properties) = properties.into_iter().find(|n| n.qualified_name == "w:rPr") {
        apply_style(&mut style, properties);
    }
    let mut image = crate::editable_objects::prepare(
        ctx,
        ObjectSource::Equation {
            latex,
            size: style.size.clamp(6.0, 100.0),
            colour: style.color,
        },
        0.0,
        0.0,
    )?;
    restore_metadata(&mut image, Some(node));
    Ok(image)
}
pub(super) fn read_drawing(
    drawing: &Node,
    anchor_paragraph: &Node,
    rels: &HashMap<String, String>,
    styles: &Styles,
    numbering: &HashMap<String, ListKind>,
    ctx: &eframe::egui::Context,
) -> Result<EmbeddedImage, String> {
    let shape = drawing
        .descendant("wsp")
        .ok_or("Unsupported drawing type")?;
    let props = shape.child("spPr").ok_or("Drawing has no geometry")?;
    let kind = match props.child("prstGeom").and_then(|n| n.attr("prst")) {
        Some("rect") => {
            if shape.child("cNvSpPr").and_then(|n| n.attr("txBox")) == Some("1") {
                ShapeKind::TextBox
            } else {
                ShapeKind::Rectangle
            }
        }
        Some("ellipse") => ShapeKind::Ellipse,
        Some("line") => {
            if props.child("ln").and_then(|n| n.child("tailEnd")).is_some() {
                ShapeKind::Arrow
            } else {
                ShapeKind::Line
            }
        }
        _ => return Err("Unsupported drawing geometry; text retained separately".into()),
    };
    let mut text = Paragraph::default();
    if let Some(content) = shape.descendant("txbxContent") {
        let mut first = true;
        for p in content.children.iter().filter(|n| n.name == "p") {
            let more = paragraph(p, rels, styles, numbering);
            if first {
                text = more;
                first = false;
            } else {
                text.chars.push(StyledChar {
                    ch: '\n',
                    style: text.style.clone(),
                });
                text.chars.extend(more.chars);
            }
        }
    }
    let fill = props
        .child("solidFill")
        .and_then(|n| n.child("srgbClr"))
        .and_then(Node::val)
        .and_then(unrgb);
    let line = props.child("ln");
    let border = CellBorder {
        colour: line
            .and_then(|n| n.child("solidFill"))
            .and_then(|n| n.child("srgbClr"))
            .and_then(Node::val)
            .and_then(unrgb)
            .unwrap_or([95; 3]),
        width: if line.is_some_and(|n| n.child("noFill").is_some()) {
            0.0
        } else {
            line.and_then(|n| n.attr("w"))
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(12700.0)
                / 12700.0
        },
    };
    let extent = drawing
        .descendant("extent")
        .ok_or("Drawing has no dimensions")?;
    let width = extent
        .attr("cx")
        .and_then(|s| s.parse::<f32>().ok())
        .ok_or("Invalid drawing width")?
        / 12700.0;
    let standard_height = extent
        .attr("cy")
        .and_then(|s| s.parse::<f32>().ok())
        .ok_or("Invalid drawing height")?
        / 12700.0;
    let horizontal_line =
        matches!(kind, ShapeKind::Line | ShapeKind::Arrow) && standard_height <= 0.01;
    let height = if horizontal_line {
        line_box_height(anchor_paragraph)
            .or_else(|| {
                drawing
                    .descendant("docPr")
                    .and_then(|n| n.attr("lineBoxHeight"))
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|h| h.is_finite() && (12.0..=1500.0).contains(h))
            })
            .unwrap_or(12.0)
    } else {
        standard_height
    };
    let mut image = crate::editable_objects::prepare(
        ctx,
        ObjectSource::Drawing {
            kind,
            text: Box::new(text),
            fill,
            border,
        },
        width,
        height,
    )?;
    image.placement = import_placement(drawing, drawing.descendant("docPr"));
    if horizontal_line {
        image.placement.offset[1] -= height / 2.0;
    }
    restore_metadata(&mut image, drawing.descendant("docPr"));
    Ok(image)
}

pub(super) fn line_box_height(paragraph: &Node) -> Option<f32> {
    let spacing = paragraph.child("pPr")?.child("spacing")?;
    if spacing.attr("lineRule") != Some("exact") {
        return None;
    }
    let height = spacing.attr("line")?.parse::<f32>().ok()? / 20.0;
    (height.is_finite() && (12.0..=1500.0).contains(&height)).then_some(height)
}
