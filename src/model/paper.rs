use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub enum Paper {
    #[default]
    A4,
    Letter,
    Tabloid,
    Legal,
    Statement,
    Executive,
    Folio,
    A3,
    A5,
    B4,
    B5,
    Custom {
        width: f32,
        height: f32,
    },
}
impl Paper {
    pub const STANDARD: [Self; 11] = [
        Self::Letter,
        Self::Tabloid,
        Self::Legal,
        Self::Statement,
        Self::Executive,
        Self::Folio,
        Self::A3,
        Self::A4,
        Self::A5,
        Self::B4,
        Self::B5,
    ];
    pub fn size(self) -> (f32, f32) {
        match self {
            Self::Letter => (612.0, 792.0),
            Self::Tabloid => (792.0, 1224.0),
            Self::Legal => (612.0, 1008.0),
            Self::Statement => (396.0, 612.0),
            Self::Executive => (522.0, 756.0),
            Self::Folio => (612.0, 936.0),
            Self::A3 => (297.0 * 72.0 / 25.4, 420.0 * 72.0 / 25.4),
            Self::A4 => (210.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4),
            Self::A5 => (148.0 * 72.0 / 25.4, 210.0 * 72.0 / 25.4),
            Self::B4 => (250.0 * 72.0 / 25.4, 353.0 * 72.0 / 25.4),
            Self::B5 => (176.0 * 72.0 / 25.4, 250.0 * 72.0 / 25.4),
            Self::Custom { width, height } => (width, height),
        }
    }
    pub fn label(self) -> String {
        match self {
            Self::Letter => "Letter (8.5″ × 11″)",
            Self::Tabloid => "Tabloid (11″ × 17″)",
            Self::Legal => "Legal (8.5″ × 14″)",
            Self::Statement => "Statement (5.5″ × 8.5″)",
            Self::Executive => "Executive (7.25″ × 10.5″)",
            Self::Folio => "Folio (8.5″ × 13″)",
            Self::A3 => "A3 (11.69″ × 16.54″)",
            Self::A4 => "A4 (8.27″ × 11.69″)",
            Self::A5 => "A5 (5.83″ × 8.27″)",
            Self::B4 => "B4 (9.84″ × 13.90″)",
            Self::B5 => "B5 (6.93″ × 9.84″)",
            Self::Custom { width, height } => {
                return format!("Custom ({:.2}″ × {:.2}″)", width / 72.0, height / 72.0);
            }
        }
        .into()
    }
    pub fn from_size(width: f32, height: f32) -> Self {
        Self::STANDARD
            .into_iter()
            .find(|p| {
                let (w, h) = p.size();
                (w - width).abs() < 0.1 && (h - height).abs() < 0.1
            })
            .unwrap_or(Self::Custom { width, height })
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum LineNumbering {
    #[default]
    Off,
    Continuous,
    EachPage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_have_valid_physical_sizes_and_custom_size_roundtrips() {
        assert_eq!(Paper::STANDARD.len(), 11);
        for paper in Paper::STANDARD {
            let (width, height) = paper.size();
            assert!(width.is_finite() && height.is_finite());
            assert!(width >= 72.0 && height >= 72.0);
            assert_eq!(Paper::from_size(width, height), paper);
        }

        let custom = Paper::Custom {
            width: 703.25,
            height: 911.75,
        };
        let json = serde_json::to_string(&custom).unwrap();
        assert_eq!(serde_json::from_str::<Paper>(&json).unwrap(), custom);
        assert_eq!(Paper::from_size(703.25, 911.75), custom);

        let mut document = crate::model::Document::new("Custom page");
        document.page.paper = custom;
        let native = serde_json::to_vec(&document).unwrap();
        let loaded: crate::model::Document = serde_json::from_slice(&native).unwrap();
        assert_eq!(loaded.version, crate::model::DOCUMENT_VERSION);
        assert_eq!(loaded.page.paper, custom);
    }
}
