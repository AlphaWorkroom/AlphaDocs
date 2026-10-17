use serde::{Deserialize, Serialize};

/// Physical document colours and points, independent of the workspace theme.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CellBorder {
    pub colour: [u8; 3],
    pub width: f32,
}
impl Default for CellBorder {
    fn default() -> Self {
        Self {
            colour: [128; 3],
            width: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BoxStyle {
    pub fill: Option<[u8; 3]>,
    /// Top, bottom, left, right. Zero width explicitly removes an edge.
    pub borders: [CellBorder; 4],
}
impl BoxStyle {
    pub fn unbordered() -> Self {
        Self {
            borders: [CellBorder {
                width: 0.0,
                ..Default::default()
            }; 4],
            ..Default::default()
        }
    }
    pub fn valid(self) -> bool {
        self.borders
            .iter()
            .all(|b| b.width.is_finite() && (0.0..=12.0).contains(&b.width))
    }
}
