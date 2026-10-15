//! Reversible image view edits. Original embedded bytes are never replaced.
use crate::model::{EmbeddedImage, ImageEffects};

/// sRGB adjustments are shared by the screen texture and flattened exports.
pub fn apply_effects(pixels: &mut image::RgbaImage, effects: ImageEffects) {
    if effects == ImageEffects::default() {
        return;
    }
    for pixel in pixels.pixels_mut() {
        let gray = 0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
        for channel in &mut pixel.0[..3] {
            let value = if effects.grayscale {
                gray
            } else {
                *channel as f32
            };
            *channel =
                ((value - 127.5) * (1.0 + effects.contrast) + 127.5 + effects.brightness * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8;
        }
        pixel[3] = (pixel[3] as f32 * effects.opacity).round() as u8;
    }
}

pub fn alpha_bounds(data: &[u8]) -> Result<Option<[f32; 4]>, String> {
    let decoded = image::load_from_memory(data)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for (x, y, pixel) in decoded.enumerate_pixels() {
        if pixel[3] > 0 {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
    }
    if left >= right || top >= bottom {
        return Ok(None);
    }
    Ok(Some([
        left as f32 / width as f32,
        top as f32 / height as f32,
        right as f32 / width as f32,
        bottom as f32 / height as f32,
    ]))
}
pub fn flattened_png(image: &EmbeddedImage) -> Result<Vec<u8>, String> {
    let decoded = image::load_from_memory(&image.data).map_err(|e| e.to_string())?;
    let (width, height) = (decoded.width(), decoded.height());
    let [left, top, right, bottom] = image.visible_crop();
    let x = ((left * width as f32).round() as u32).min(width - 1);
    let y = ((top * height as f32).round() as u32).min(height - 1);
    let end_x = ((right * width as f32).round() as u32).clamp(x + 1, width);
    let end_y = ((bottom * height as f32).round() as u32).clamp(y + 1, height);
    let mut pixels = decoded.crop_imm(x, y, end_x - x, end_y - y).to_rgba8();
    apply_effects(&mut pixels, image.effects);
    if image.padding.enabled && image.padding.fraction > 0.0 {
        let p = image.padding_points();
        let px = (pixels.width() as f32 * p / (image.width * (right - left))).round() as u32;
        let py = (pixels.height() as f32 * p / (image.height * (bottom - top))).round() as u32;
        let mut padded = image::RgbaImage::new(pixels.width() + 2 * px, pixels.height() + 2 * py);
        image::imageops::overlay(&mut padded, &pixels, px as i64, py as i64);
        pixels = padded;
    }
    let cropped = image::DynamicImage::ImageRgba8(pixels);
    let mut bytes = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(bytes.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compat, engine::Editor, model::*, persistence};
    use std::io::Read;

    fn source() -> EmbeddedImage {
        let pixels = image::RgbaImage::from_fn(10, 10, |x, y| {
            if (2..6).contains(&x) && (1..7).contains(&y) {
                image::Rgba([50, 120, 200, 255])
            } else {
                image::Rgba([0, 0, 0, 0])
            }
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(pixels)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        EmbeddedImage {
            name: "Original <image>.png".into(),
            data: bytes.into_inner().into(),
            width: 100.0,
            height: 100.0,
            alt_text: "Blue shape & transparent edges".into(),
            title: "A \"shape\"".into(),
            ..Default::default()
        }
    }
    #[test]
    fn trimming_is_reversible_and_native_docx_retain_source_and_edit_metadata() {
        let original = source();
        let mut doc = Document::new("Crop round trip");
        doc.blocks = vec![
            Block::Image(original.clone()),
            Block::Paragraph(Paragraph::default()),
        ];
        let mut editor = Editor::new(doc);
        editor.trim_image_transparency(0, true).unwrap();
        let Block::Image(cropped) = &editor.doc.blocks[0] else {
            unreachable!()
        };
        assert_eq!(cropped.visible_crop(), [0.2, 0.1, 0.6, 0.7]);
        assert!((cropped.visible_size().0 - 40.0).abs() < 0.001);
        assert!((cropped.visible_size().1 - 60.0).abs() < 0.001);
        assert_eq!(cropped.data, original.data);
        let flattened = image::load_from_memory(&flattened_png(cropped).unwrap()).unwrap();
        assert_eq!((flattened.width(), flattened.height()), (4, 6));
        let cropped = cropped.clone();
        let native = std::env::temp_dir().join(format!("alphadocs-crop-{}.adocs", editor.doc.id));
        let docx = native.with_extension("docx");
        persistence::save_document(&editor.doc, &native).unwrap();
        let reopened = persistence::load_document(&native).unwrap();
        assert_eq!(reopened.version, DOCUMENT_VERSION);
        assert_eq!(reopened.blocks, editor.doc.blocks);
        compat::docx::export(&editor.doc, &docx).unwrap();
        let (reopened, warnings) = compat::docx::import(&docx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let restored = reopened
            .blocks
            .iter()
            .find_map(|b| {
                if let Block::Image(i) = b {
                    Some(i)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(*restored, cropped);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&docx).unwrap()).unwrap();
        let mut media = Vec::new();
        zip.by_name("word/media/image1.png")
            .unwrap()
            .read_to_end(&mut media)
            .unwrap();
        assert_eq!(media, original.data.as_slice());
        let mut xml = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("<a:srcRect l=\"20000\" t=\"10000\" r=\"40000\" b=\"30000\"/>"));
        drop(zip);
        editor.trim_image_transparency(0, false).unwrap();
        let Block::Image(untrimmed) = &editor.doc.blocks[0] else {
            unreachable!()
        };
        assert_eq!(untrimmed.visible_crop(), [0.0, 0.0, 1.0, 1.0]);
        assert_eq!(untrimmed.data, original.data);
        editor.undo();
        assert_eq!(editor.doc.blocks[0], Block::Image(cropped));
        std::fs::remove_file(native).unwrap();
        std::fs::remove_file(docx).unwrap();
    }
    #[test]
    fn adjustments_are_reversible_and_shared_with_exports_and_interchange() {
        let original = source();
        let mut editor = Editor::new(Document::new("Image effects"));
        editor.doc.blocks.insert(0, Block::Image(original.clone()));
        let effects = ImageEffects {
            opacity: 0.5,
            brightness: 0.1,
            contrast: -0.25,
            grayscale: true,
        };
        editor.set_image_effects(0, effects);
        let Block::Image(adjusted) = &editor.doc.blocks[0] else {
            unreachable!()
        };
        assert_eq!(adjusted.data, original.data);
        let mut screen = image::load_from_memory(&original.data).unwrap().to_rgba8();
        apply_effects(&mut screen, effects);
        let flattened = image::load_from_memory(&flattened_png(adjusted).unwrap())
            .unwrap()
            .to_rgba8();
        assert_eq!(screen, flattened);
        assert_eq!(screen.get_pixel(2, 1)[3], 128);
        assert_eq!(screen.get_pixel(2, 1)[0], screen.get_pixel(2, 1)[1]);
        assert_eq!(screen.get_pixel(0, 0)[3], 0);
        let native =
            std::env::temp_dir().join(format!("alphadocs-effects-{}.adocs", editor.doc.id));
        persistence::save_document(&editor.doc, &native).unwrap();
        assert_eq!(persistence::load_document(&native).unwrap(), editor.doc);
        let path = native.with_extension("docx");
        compat::docx::export(&editor.doc, &path).unwrap();
        let (reopened, warnings) = compat::docx::import(&path).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(reopened.blocks, editor.doc.blocks);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut xml = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut xml)
            .unwrap();
        assert!(xml.contains("<a:alphaModFix amt=\"50000\"/>"));
        assert!(xml.contains("<a:grayscl/>"));
        drop(zip);
        let changed =
            path.with_file_name(format!("alphadocs-effects-external-{}.docx", editor.doc.id));
        let mut original_zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&changed).unwrap());
        for index in 0..original_zip.len() {
            let mut part = original_zip.by_index(index).unwrap();
            let name = part.name().to_string();
            let mut bytes = Vec::new();
            part.read_to_end(&mut bytes).unwrap();
            if name == "word/document.xml" {
                bytes = String::from_utf8(bytes)
                    .unwrap()
                    .replace("bright=\"10000\"", "bright=\"20000\"")
                    .replace("cx=\"1270000\"", "cx=\"2540000\"")
                    .into_bytes();
            }
            writer
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut writer, &bytes).unwrap();
        }
        writer.finish().unwrap();
        let (external, warnings) = compat::docx::import(&changed).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(
            matches!(&external.blocks[0], Block::Image(i) if i.effects.brightness == 0.2 && i.width == 200.0 && i.data == original.data)
        );
        std::fs::remove_file(changed).unwrap();
        editor.undo();
        assert_eq!(editor.doc.blocks[0], Block::Image(original));
        editor.redo();
        assert!(matches!(&editor.doc.blocks[0], Block::Image(i) if i.effects == effects));
        std::fs::remove_file(native).unwrap();
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn transparent_padding_keeps_original_media_and_roundtrips_with_standard_docx_preview() {
        let original = source();
        let mut doc = Document::new("Padding regression");
        let mut im = original.clone();
        im.trim_transparency = true;
        im.trim_bounds = Some([0.2, 0.1, 0.6, 0.7]);
        im.padding = ImagePadding {
            enabled: true,
            fraction: 0.25,
        };
        let (visible_w, visible_h) = im.visible_size();
        assert!((visible_w - 60.0).abs() < 0.001 && (visible_h - 80.0).abs() < 0.001);
        let (w, h) = im.source_size_for_visible(120.0, 160.0);
        assert!((w - 200.0).abs() < 0.001 && (h - 200.0).abs() < 0.001);
        let png = flattened_png(&im).unwrap();
        let pixels = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(pixels.dimensions(), (6, 8));
        assert_eq!(pixels.get_pixel(0, 0)[3], 0);
        assert_eq!(pixels.get_pixel(1, 1)[3], 255);
        doc.blocks = vec![
            Block::Image(im.clone()),
            Block::Paragraph(Paragraph::default()),
        ];
        let path = std::env::temp_dir().join(format!("padding-{}.adocs", doc.id));
        persistence::save_document(&doc, &path).unwrap();
        assert_eq!(
            persistence::load_document(&path).unwrap().blocks,
            doc.blocks
        );
        compat::docx::export(&doc, &path.with_extension("docx")).unwrap();
        let (imported, warnings) = compat::docx::import(&path.with_extension("docx")).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        let restored = imported
            .blocks
            .iter()
            .find_map(|b| {
                if let Block::Image(i) = b {
                    Some(i)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(restored, &im);
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(path.with_extension("docx")).unwrap())
                .unwrap();
        let mut source = Vec::new();
        zip.by_name("word/media/image1-original.png")
            .unwrap()
            .read_to_end(&mut source)
            .unwrap();
        assert_eq!(source, original.data.as_slice());
        let mut display = Vec::new();
        zip.by_name("word/media/image2.png")
            .unwrap()
            .read_to_end(&mut display)
            .unwrap();
        assert_eq!(display, png);
        drop(zip);
        let mut e = Editor::new(doc);
        e.transaction(|e| {
            if let Block::Image(i) = &mut e.doc.blocks[0] {
                i.padding.enabled = false;
            }
        });
        assert!(
            matches!(&e.doc.blocks[0],Block::Image(i) if !i.padding.enabled && i.data==original.data)
        );
        e.undo();
        assert_eq!(e.doc.blocks[0], Block::Image(im));
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("docx")).unwrap();
    }
    #[test]
    fn fully_transparent_images_report_no_visible_bounds() {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(3, 3))
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        assert_eq!(alpha_bounds(bytes.get_ref()).unwrap(), None);
    }
}
