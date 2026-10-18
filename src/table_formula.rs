//! Bounded, offline Word-style table calculations. No scripting or I/O.
use crate::model::{Paragraph, TableFormula};

pub fn evaluate(
    rows: &[Vec<Paragraph>],
    row: usize,
    column: usize,
    formula: &str,
) -> Result<f64, String> {
    if formula.len() > 4096 || !formula.is_ascii() {
        return Err("Use an ASCII formula of at most 4096 characters".into());
    }
    if rows.get(row).and_then(|r| r.get(column)).is_none()
        || rows.iter().map(Vec::len).sum::<usize>() > 100_000
    {
        return Err("Table is outside the calculation limits".into());
    }
    let mut visited = vec![(row, column)];
    let mut budget = 100_000;
    expression(rows, (row, column), formula, &mut visited, &mut budget)
}
/// Evaluate a whole table before writing anything. One shared work budget
/// bounds automatic recalculation, including repeated dependency traversal.
pub fn calculate(rows: &[Vec<Paragraph>]) -> Result<Vec<(usize, usize, String)>, String> {
    if rows.iter().map(Vec::len).sum::<usize>() > 100_000 {
        return Err("Table is outside the calculation limits".into());
    }
    let mut results = Vec::new();
    let mut budget = 100_000;
    for (row, cells) in rows.iter().enumerate() {
        for (column, paragraph) in cells.iter().enumerate() {
            let Some(formula) = &paragraph.formula else {
                continue;
            };
            let value = if formula.word_number_format.is_some() {
                Err("This Word number format is preserved with its existing result. Choose Formula and a supported number format to recalculate it.".into())
            } else if formula.expression.len() > 4096 || !formula.expression.is_ascii() {
                Err("Use an ASCII formula of at most 4096 characters".into())
            } else {
                expression(rows, (row,column), &formula.expression, &mut vec![(row,column)], &mut budget)
            }.map_err(|e: String| format!("{}: {e}", cell_name(row,column)))?;
            results.push((row, column, formatted(value, formula.decimals)));
        }
    }
    Ok(results)
}
pub fn cell_name(row: usize, column: usize) -> String {
    let mut letters = Vec::new();
    let mut column = column;
    loop {
        letters.push((b'A' + (column % 26) as u8) as char);
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    format!(
        "{}{}",
        letters.into_iter().rev().collect::<String>(),
        row.saturating_add(1)
    )
}
/// Suggest a sum from actual numeric cells, rather than from row position alone.
pub fn suggested_formula(rows: &[Vec<Paragraph>], row: usize, column: usize) -> &'static str {
    let numeric = |p: &Paragraph| {
        p.formula.is_some() || p.text().trim().parse::<f64>().is_ok_and(f64::is_finite)
    };
    let above = rows
        .iter()
        .take(row)
        .any(|r| r.get(column).is_some_and(numeric));
    let left = rows
        .get(row)
        .is_some_and(|r| r.iter().take(column).any(numeric));
    if above || !left && row > 0 {
        "=SUM(ABOVE)"
    } else {
        "=SUM(LEFT)"
    }
}
pub fn formatted(value: f64, decimals: Option<u8>) -> String {
    if let Some(digits) = decimals {
        format!("{value:.precision$}", precision = digits.min(8) as usize)
    } else if value == 0.0 {
        "0".into()
    } else {
        format!("{value}")
    }
}
fn expression(
    rows: &[Vec<Paragraph>],
    at: (usize, usize),
    formula: &str,
    visited: &mut Vec<(usize, usize)>,
    budget: &mut usize,
) -> Result<f64, String> {
    let source = formula
        .trim()
        .strip_prefix('=')
        .unwrap_or(formula.trim())
        .as_bytes();
    let mut parser = Parser {
        source,
        position: 0,
        rows,
        at,
        visited,
        depth: 0,
        budget,
    };
    let value = parser.expr(0)?;
    parser.space();
    if parser.position != source.len() {
        return Err("Unexpected text in formula".into());
    }
    finite(value)
}
fn finite(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Calculation overflow or division by zero".into())
    }
}
struct Parser<'a, 'b> {
    source: &'a [u8],
    position: usize,
    rows: &'a [Vec<Paragraph>],
    at: (usize, usize),
    visited: &'b mut Vec<(usize, usize)>,
    depth: usize,
    budget: &'b mut usize,
}
impl Parser<'_, '_> {
    fn space(&mut self) {
        while self
            .source
            .get(self.position)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.position += 1;
        }
    }
    fn eat(&mut self, ch: u8) -> bool {
        self.space();
        if self.source.get(self.position) == Some(&ch) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    fn word(&mut self) -> String {
        self.space();
        let start = self.position;
        while self
            .source
            .get(self.position)
            .is_some_and(u8::is_ascii_alphanumeric)
        {
            self.position += 1;
        }
        String::from_utf8_lossy(&self.source[start..self.position]).to_ascii_uppercase()
    }
    fn expr(&mut self, minimum: u8) -> Result<f64, String> {
        if *self.budget == 0 {
            return Err("Calculation is too complex".into());
        }
        *self.budget -= 1;
        self.depth += 1;
        if self.depth > 64 {
            return Err("Formula nesting is too deep".into());
        }
        let mut left = self.atom()?;
        loop {
            self.space();
            let Some(&op) = self.source.get(self.position) else {
                break;
            };
            let (precedence, right) = match op {
                b'+' | b'-' => (1, 2),
                b'*' | b'/' => (3, 4),
                b'^' => (5, 5),
                _ => break,
            };
            if precedence < minimum {
                break;
            }
            self.position += 1;
            let value = self.expr(right)?;
            left = finite(match op {
                b'+' => left + value,
                b'-' => left - value,
                b'*' => left * value,
                b'/' => left / value,
                _ => left.powf(value),
            })?;
        }
        self.depth -= 1;
        Ok(left)
    }
    fn atom(&mut self) -> Result<f64, String> {
        self.space();
        if self.eat(b'+') {
            return self.expr(5);
        }
        if self.eat(b'-') {
            return self.expr(5).map(|n| -n);
        }
        if self.eat(b'(') {
            let value = self.expr(0)?;
            if !self.eat(b')') {
                return Err("Missing closing parenthesis".into());
            }
            return Ok(value);
        }
        let start = self.position;
        if self
            .source
            .get(start)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            while self
                .source
                .get(self.position)
                .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
            {
                self.position += 1;
            }
            return String::from_utf8_lossy(&self.source[start..self.position])
                .parse::<f64>()
                .map_err(|_| "Invalid number".into())
                .and_then(finite);
        }
        let name = self.word();
        if name.is_empty() {
            return Err("Expected a number, cell reference or function".into());
        }
        if !self.eat(b'(') {
            return self.cell(reference(&name)?);
        }
        let mut values = vec![];
        if !self.eat(b')') {
            loop {
                let saved = self.position;
                let range = self.word();
                if matches!(range.as_str(), "ABOVE" | "BELOW" | "LEFT" | "RIGHT") {
                    let (r, c) = self.at;
                    let coordinates: Vec<_> = match range.as_str() {
                        "ABOVE" => (0..r).map(|row| (row, c)).collect(),
                        "BELOW" => (r + 1..self.rows.len()).map(|row| (row, c)).collect(),
                        "LEFT" => (0..c).map(|col| (r, col)).collect(),
                        _ => (c + 1..self.rows.get(r).map_or(0, Vec::len))
                            .map(|col| (r, col))
                            .collect(),
                    };
                    for at in coordinates {
                        if self
                            .rows
                            .get(at.0)
                            .and_then(|row| row.get(at.1))
                            .is_some_and(|p| {
                                p.formula.is_some() || p.text().trim().parse::<f64>().is_ok()
                            })
                        {
                            values.push(self.cell(at)?);
                        }
                    }
                } else if !range.is_empty() && self.eat(b':') {
                    let a = reference(&range)?;
                    let b = reference(&self.word())?;
                    if a.0
                        .abs_diff(b.0)
                        .saturating_add(1)
                        .saturating_mul(a.1.abs_diff(b.1).saturating_add(1))
                        > 100_000
                    {
                        return Err("Cell range is too large".into());
                    }
                    for row in a.0.min(b.0)..=a.0.max(b.0) {
                        for col in a.1.min(b.1)..=a.1.max(b.1) {
                            values.push(self.cell((row, col))?);
                        }
                    }
                } else {
                    self.position = saved;
                    values.push(self.expr(0)?);
                }
                if self.eat(b')') {
                    break;
                }
                if !self.eat(b',') {
                    return Err("Separate function arguments with commas".into());
                }
            }
        }
        let result = match name.as_str() {
            "SUM" => values.iter().sum(),
            "PRODUCT" => values.iter().product(),
            "COUNT" => values.len() as f64,
            "AVERAGE" if !values.is_empty() => values.iter().sum::<f64>() / values.len() as f64,
            "MIN" if !values.is_empty() => values.iter().copied().fold(f64::INFINITY, f64::min),
            "MAX" if !values.is_empty() => values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            "ABS" if values.len() == 1 => values[0].abs(),
            "INT" if values.len() == 1 => values[0].floor(),
            "ROUND" if values.len() == 2 && values[1].fract() == 0.0 && values[1].abs() <= 8.0 => {
                let factor = 10f64.powf(values[1]);
                (values[0] * factor).round() / factor
            }
            _ => return Err(format!("Unsupported function or argument count: {name}")),
        };
        finite(result)
    }
    fn cell(&mut self, at: (usize, usize)) -> Result<f64, String> {
        if *self.budget == 0 {
            return Err("Calculation is too complex".into());
        }
        *self.budget -= 1;
        if self.visited.len() >= 32 || self.visited.contains(&at) {
            return Err("Circular or excessively nested table formula".into());
        }
        let cell = self
            .rows
            .get(at.0)
            .and_then(|row| row.get(at.1))
            .ok_or("Cell reference is outside this table")?;
        if let Some(TableFormula {
            expression: formula,
            ..
        }) = &cell.formula
        {
            self.visited.push(at);
            let result = expression(self.rows, at, formula, self.visited, self.budget);
            self.visited.pop();
            result
        } else {
            let text = cell.text();
            let text = text.trim();
            if text.is_empty() {
                Ok(0.0)
            } else {
                text.parse()
                    .map_err(|_| "Referenced cell is not a number".into())
                    .and_then(finite)
            }
        }
    }
}
fn reference(text: &str) -> Result<(usize, usize), String> {
    let split = text
        .find(|c: char| c.is_ascii_digit())
        .ok_or("Use a cell reference such as A1")?;
    let (letters, digits) = text.split_at(split);
    if letters.is_empty() || !letters.bytes().all(|c| c.is_ascii_uppercase()) {
        return Err("Invalid cell reference".into());
    }
    let column = letters
        .bytes()
        .try_fold(0usize, |n, c| {
            n.checked_mul(26)?.checked_add((c - b'A' + 1) as usize)
        })
        .ok_or("Cell reference is too large")?;
    let row: usize = digits.parse().map_err(|_| "Invalid cell row")?;
    if row == 0 || column == 0 {
        return Err("Cell references start at A1".into());
    }
    Ok((row - 1, column - 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calculations_ranges_dependencies_and_failures() {
        let mut rows: Vec<Vec<Paragraph>> = [
            ["Price", "Qty", "Total"],
            ["12.5", "2", ""],
            ["7.5", "3", ""],
        ]
        .into_iter()
        .map(|r| r.into_iter().map(Paragraph::from_text).collect())
        .collect();
        assert_eq!(evaluate(&rows, 1, 2, "=PRODUCT(LEFT)").unwrap(), 25.0);
        assert_eq!(
            evaluate(&rows, 2, 2, "=SUM(A2:A3)+ROUND(2/3,2)").unwrap(),
            20.67
        );
        rows[1][2].formula = Some(TableFormula {
            expression: "=A2*B2".into(),
            decimals: Some(2),
            word_number_format: None,
        });
        assert_eq!(evaluate(&rows, 2, 2, "=SUM(ABOVE)").unwrap(), 25.0);
        assert_eq!(formatted(25.0, Some(2)), "25.00");
        for formula in [
            "=C3",
            "=A0",
            "=Z99",
            "=1/0",
            "=9^9999",
            "=UNKNOWN(1)",
            "=SUM(A1:A99999999)",
            "=ABS(1,2)",
            "=(2+3",
            "=A2 junk",
        ] {
            assert!(evaluate(&rows, 2, 2, formula).is_err(), "{formula}");
        }
        assert_eq!(evaluate(&rows, 2, 2, "=-2^2 + 3*(4+1)").unwrap(), 11.0);
        assert!(
            evaluate(
                &rows,
                2,
                2,
                &format!("={}1{}", "(".repeat(100), ")".repeat(100))
            )
            .is_err()
        );
    }
}
