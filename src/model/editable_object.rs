use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EditableObject {
    pub source: ObjectSource,
    /// Generated vector preview, shared by screen rasterisation and PDF.
    pub svg: std::sync::Arc<str>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ObjectSource {
    Equation {
        latex: String,
        size: f32,
        colour: [u8; 3],
    },
    Drawing {
        kind: ShapeKind,
        text: Box<Paragraph>,
        fill: Option<[u8; 3]>,
        border: CellBorder,
    },
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ShapeKind {
    #[default]
    TextBox,
    Rectangle,
    Ellipse,
    Line,
    Arrow,
}
impl ShapeKind {
    pub const ALL: [Self; 5] = [
        Self::TextBox,
        Self::Rectangle,
        Self::Ellipse,
        Self::Line,
        Self::Arrow,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::TextBox => "Text box",
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Line => "Line",
            Self::Arrow => "Arrow",
        }
    }
}
