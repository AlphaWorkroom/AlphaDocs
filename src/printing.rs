//! Physical print preparation, independent of the document and UI theme.
//! Platform drivers are called on workers by the UI; no PDF viewer is required.
#[cfg(windows)]
mod windows_backend;
use printpdf::{CurTransMat, Mm, Op, PdfDocument, PdfPage, PdfParseOptions, PdfSaveOptions};

#[derive(Clone, Debug)]
pub struct Printer {
    pub name: String,
    pub is_default: bool,
}
#[derive(Clone, Debug)]
pub struct DriverOption {
    pub label: String,
    pub ticket: Vec<u8>,
    pub landscape: Option<bool>,
    pub monochrome: bool,
}
#[derive(Clone, Debug)]
pub struct Media {
    pub option: DriverOption,
    pub size: [f32; 2],
}
#[derive(Clone, Debug)]
pub struct Feature {
    pub label: String,
    pub options: Vec<DriverOption>,
}
#[derive(Clone, Debug, Default)]
pub struct Capabilities {
    pub media: Vec<Media>,
    pub features: Vec<Feature>,
    pub max_copies: Option<u16>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Scale {
    Fit,
    Percent(f32),
}
#[derive(Clone, Debug)]
pub struct Settings {
    pub paper: [f32; 2],
    pub landscape: bool,
    pub scale: Scale,
    pub copies: u16,
    pub range: String,
    pub tickets: Vec<Vec<u8>>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            paper: [595.28, 841.89],
            landscape: false,
            scale: Scale::Fit,
            copies: 1,
            range: String::new(),
            tickets: Vec::new(),
        }
    }
}
impl Settings {
    pub fn output_size(&self) -> [f32; 2] {
        if self.landscape {
            [self.paper[1], self.paper[0]]
        } else {
            self.paper
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub origin: [f32; 2],
    pub size: [f32; 2],
}
impl Area {
    pub fn full(size: [f32; 2]) -> Self {
        Self {
            origin: [0.0; 2],
            size,
        }
    }
    pub fn valid_for(&self, paper: [f32; 2]) -> bool {
        self.origin.iter().all(|v| v.is_finite() && *v >= 0.0)
            && self.size.iter().all(|v| v.is_finite() && *v > 0.0)
            && (0..2).all(|i| self.origin[i] + self.size[i] <= paper[i] + 1.0)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Transform {
    pub scale: f32,
    /// Translation in PDF bottom-left point coordinates.
    pub offset: [f32; 2],
}
impl Transform {
    pub fn new(
        source: [f32; 2],
        paper: [f32; 2],
        area: Area,
        scale: Scale,
    ) -> Result<Self, String> {
        if !source
            .iter()
            .chain(&paper)
            .all(|v| v.is_finite() && *v > 0.0)
            || !area.valid_for(paper)
        {
            return Err("Invalid paper or printable area reported by the printer".into());
        }
        let scale = match scale {
            Scale::Fit => (area.size[0] / source[0]).min(area.size[1] / source[1]),
            Scale::Percent(value) if value.is_finite() && (10.0..=200.0).contains(&value) => {
                value / 100.0
            }
            _ => return Err("Print scale must be between 10% and 200%".into()),
        };
        let top = area.origin[1] + (area.size[1] - source[1] * scale) * 0.5;
        Ok(Self {
            scale,
            offset: [
                area.origin[0] + (area.size[0] - source[0] * scale) * 0.5,
                paper[1] - top - source[1] * scale,
            ],
        })
    }
}
/// User ranges are one-based. Keep natural document order and remove duplicates.
pub fn page_range(text: &str, total: usize) -> Result<Vec<usize>, String> {
    if total == 0 {
        return Err("The document has no pages".into());
    }
    if text.trim().is_empty() {
        return Ok((0..total).collect());
    }
    let mut included = vec![false; total];
    for part in text.split(',') {
        let bounds: Vec<_> = part.trim().split('-').collect();
        if bounds.is_empty() || bounds.len() > 2 {
            return Err("Use page numbers or ranges, for example 1-3, 5".into());
        }
        let parse = |s: &str| {
            s.trim()
                .parse::<usize>()
                .map_err(|_| "Use page numbers or ranges, for example 1-3, 5".to_string())
        };
        let start = parse(bounds[0])?;
        let end = if bounds.len() == 2 {
            parse(bounds[1])?
        } else {
            start
        };
        if start == 0 || start > end || end > total {
            return Err(format!("Page range must be between 1 and {total}"));
        }
        included[start - 1..end].fill(true);
    }
    Ok(included
        .into_iter()
        .enumerate()
        .filter_map(|(i, v)| v.then_some(i))
        .collect())
}
/// Impose measured document pages onto output sheets. Preview and spool use these exact bytes.
pub fn prepare_pdf(source: &[u8], settings: &Settings, area: Area) -> Result<Vec<u8>, String> {
    let mut pdf = PdfDocument::parse(source, &PdfParseOptions::default(), &mut Vec::new())
        .map_err(|e| format!("Could not prepare print pages: {e}"))?;
    let indices = page_range(&settings.range, pdf.pages.len())?;
    let paper = settings.output_size();
    let mut pages = Vec::with_capacity(indices.len());
    for index in indices {
        let page = &pdf.pages[index];
        let transform = Transform::new(
            [page.media_box.width.0, page.media_box.height.0],
            paper,
            area,
            settings.scale,
        )?;
        let corner = |x, y| printpdf::LinePoint {
            p: printpdf::Point {
                x: printpdf::Pt(x),
                y: printpdf::Pt(y),
            },
            bezier: false,
        };
        let left = area.origin[0];
        let right = left + area.size[0];
        let top = paper[1] - area.origin[1];
        let bottom = top - area.size[1];
        let mut ops = vec![
            Op::SaveGraphicsState,
            Op::DrawPolygon {
                polygon: printpdf::Polygon {
                    rings: vec![printpdf::PolygonRing {
                        points: vec![
                            corner(left, top),
                            corner(right, top),
                            corner(right, bottom),
                            corner(left, bottom),
                        ],
                    }],
                    mode: printpdf::PaintMode::Clip,
                    winding_order: printpdf::WindingOrder::NonZero,
                },
            },
            Op::SetTransformationMatrix {
                matrix: CurTransMat::Raw([
                    transform.scale,
                    0.0,
                    0.0,
                    transform.scale,
                    transform.offset[0],
                    transform.offset[1],
                ]),
            },
        ];
        // Interactive PDF links are not part of a paper printout.
        ops.extend(
            page.ops
                .iter()
                .filter(|op| !matches!(op, Op::LinkAnnotation { .. }))
                .cloned(),
        );
        ops.push(Op::RestoreGraphicsState);
        pages.push(PdfPage::new(
            Mm(paper[0] * 25.4 / 72.0),
            Mm(paper[1] * 25.4 / 72.0),
            ops,
        ));
    }
    pdf.pages = pages;
    Ok(pdf.save(&PdfSaveOptions::default(), &mut Vec::new()))
}

pub fn discover() -> Result<Vec<Printer>, String> {
    #[cfg(windows)]
    {
        windows_backend::discover()
    }
    #[cfg(not(windows))]
    {
        Err("Native printer discovery is not implemented on this platform yet. Save the prepared PDF to print with your system tools.".into())
    }
}
pub fn capabilities(printer: &str) -> Result<Capabilities, String> {
    #[cfg(windows)]
    {
        windows_backend::capabilities(printer)
    }
    #[cfg(not(windows))]
    {
        let _ = printer;
        Err("Native printer capabilities are not available on this platform yet".into())
    }
}
#[derive(Clone, Debug)]
pub struct Validated {
    pub ticket: Vec<u8>,
    pub area: Option<Area>,
    pub monochrome: Option<bool>,
}
pub fn validate(printer: &str, settings: &Settings) -> Result<Validated, String> {
    #[cfg(windows)]
    {
        windows_backend::validate(printer, settings)
    }
    #[cfg(not(windows))]
    {
        let _ = (printer, settings);
        Err("Native job submission is not implemented on this platform yet".into())
    }
}
pub fn submit(printer: &str, path: &std::path::Path, ticket: Vec<u8>) -> Result<(), String> {
    #[cfg(windows)]
    {
        windows_backend::submit(printer, path, ticket)
    }
    #[cfg(not(windows))]
    {
        let _ = (printer, path, ticket);
        Err("Native job submission is not implemented on this platform yet".into())
    }
}
pub fn render_preview(bytes: &[u8], page: usize) -> Result<Vec<u8>, String> {
    #[cfg(windows)]
    {
        windows_backend::render_preview(bytes, page)
    }
    #[cfg(not(windows))]
    {
        let _ = (bytes, page);
        Err("Print PDF preview rendering is not implemented on this platform yet".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_validate_and_deduplicate() {
        assert_eq!(page_range("2-4, 1, 3", 5).unwrap(), [0, 1, 2, 3]);
        for bad in ["0", "2-1", "6", "1,", "a", "1-2-3"] {
            assert!(page_range(bad, 5).is_err());
        }
        assert_eq!(page_range("", 2).unwrap(), [0, 1]);
    }
    #[test]
    fn physical_fit_and_scale_use_printable_area() {
        let area = Area {
            origin: [18.0, 36.0],
            size: [576.0, 720.0],
        };
        let t = Transform::new([612.0, 792.0], [612.0, 792.0], area, Scale::Fit).unwrap();
        assert!((t.scale - 720.0 / 792.0).abs() < 0.0001);
        let t = Transform::new([612.0, 792.0], [612.0, 792.0], area, Scale::Percent(50.0)).unwrap();
        assert_eq!(t.scale, 0.5);
        assert_eq!(t.offset, [153.0, 198.0]);
        assert!(
            Transform::new(
                [612.0, 792.0],
                [612.0, 792.0],
                area,
                Scale::Percent(f32::NAN)
            )
            .is_err()
        );
    }
    #[test]
    fn print_pdf_preserves_pages_and_changes_only_output_geometry() {
        let source = PdfDocument::new("Print test")
            .with_pages(vec![
                PdfPage::new(Mm(210.0), Mm(297.0), vec![]),
                PdfPage::new(Mm(210.0), Mm(297.0), vec![]),
            ])
            .save(&PdfSaveOptions::default(), &mut vec![]);
        let settings = Settings {
            paper: [612.0, 792.0],
            range: "2".into(),
            scale: Scale::Percent(50.0),
            ..Default::default()
        };
        let bytes = prepare_pdf(&source, &settings, Area::full(settings.output_size())).unwrap();
        let pdf = PdfDocument::parse(&bytes, &PdfParseOptions::default(), &mut vec![]).unwrap();
        assert_eq!(pdf.pages.len(), 1);
        assert!((pdf.pages[0].media_box.width.0 - 612.0).abs() < 0.01);
        assert!(pdf.pages[0].ops.iter().any(|op|matches!(op, Op::SetTransformationMatrix {matrix} if (matrix.as_array()[0]-0.5).abs()<0.001)));
    }
}
