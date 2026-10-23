//! Draws shapes and labels into a character grid.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use unicode_width::UnicodeWidthChar;

use super::{Anchor, Bounds, Point, Shape, Text};

const LEFT: u8 = 1;
const RIGHT: u8 = 2;
const UP: u8 = 4;
const DOWN: u8 = 8;

/// Returns the box around `points`, in pixels.
pub(crate) fn bounds(points: &[Point]) -> Bounds {
    let mut bounds = Bounds {
        bottom: f64::NEG_INFINITY,
        left: f64::INFINITY,
        right: f64::NEG_INFINITY,
        top: f64::INFINITY,
    };
    for point in points {
        bounds.bottom = bounds.bottom.max(point.y);
        bounds.left = bounds.left.min(point.x);
        bounds.right = bounds.right.max(point.x);
        bounds.top = bounds.top.min(point.y);
    }
    bounds
}

/// A character grid that shapes and labels paint into, in document order.
pub(crate) struct Grid {
    cell_height: f64,
    cell_width: f64,
    glyphs: BTreeMap<i32, BTreeMap<i32, String>>,
    lines: BTreeMap<i32, BTreeMap<i32, u8>>,
    slants: BTreeMap<i32, BTreeMap<i32, char>>,
}

impl Grid {
    pub(crate) fn create(cell_height: f64, cell_width: f64) -> Self {
        Self {
            cell_height,
            cell_width,
            glyphs: BTreeMap::new(),
            lines: BTreeMap::new(),
            slants: BTreeMap::new(),
        }
    }

    /// Clears the cells whose centers fall inside `shape`, by the even-odd rule.
    pub(crate) fn fill(&mut self, paths: &[Vec<Point>]) {
        let points = paths.iter().flatten().copied().collect::<Vec<Point>>();
        let bounds = bounds(&points);

        for y in self.line(bounds.top)..=self.line(bounds.bottom) {
            let center = (y as f64 + 0.5) * self.cell_height;
            let mut crossings = Vec::new();

            for shape in paths {
                for index in 1..shape.len() {
                    let a = shape[index - 1];
                    let b = shape[index];
                    if (a.y <= center) == (b.y <= center) {
                        continue;
                    }
                    crossings.push(a.x + ((center - a.y) / (b.y - a.y)) * (b.x - a.x));
                }
            }
            crossings.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

            let mut index = 0;
            while index + 1 < crossings.len() {
                let start = crossings[index];
                let end = crossings[index + 1];
                for x in self.column(start)..=self.column(end) {
                    let middle = (x as f64 + 0.5) * self.cell_width;
                    if middle < start || middle > end {
                        continue;
                    }
                    if let Some(row) = self.glyphs.get_mut(&y) {
                        row.remove(&x);
                    }
                    if let Some(row) = self.lines.get_mut(&y) {
                        row.remove(&x);
                    }
                    if let Some(row) = self.slants.get_mut(&y) {
                        row.remove(&x);
                    }
                }
                index += 2;
            }
        }
    }

    /// Draws a small shape as one character: an arrowhead, `o` for a dot or an outline, or `#`.
    pub(crate) fn mark(&mut self, element: &Shape) {
        let shape = element
            .paths
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<Point>>();
        let bounds = bounds(&shape);
        let center = Point {
            x: (bounds.left + bounds.right) / 2.0,
            y: (bounds.top + bounds.bottom) / 2.0,
        };

        let glyph = if element.round {
            'o'
        } else {
            let corners = shape
                .iter()
                .enumerate()
                .filter(|(index, point)| {
                    shape
                        .iter()
                        .position(|other| other.x == point.x && other.y == point.y)
                        == Some(*index)
                })
                .map(|(_, point)| *point)
                .collect::<Vec<Point>>();

            if corners.len() != 3 {
                if element.fill == super::Fill::Paint {
                    '#'
                } else {
                    'o'
                }
            } else {
                // The tip of a triangle is the corner farthest from the middle of the other two.
                let mut tip = corners[0];
                let mut far = -1.0;
                for point in &corners {
                    let others = corners
                        .iter()
                        .copied()
                        .filter(|other| other != point)
                        .collect::<Vec<Point>>();
                    let base = Point {
                        x: (others[0].x + others[1].x) / 2.0,
                        y: (others[0].y + others[1].y) / 2.0,
                    };
                    let distance = (point.x - base.x).hypot(point.y - base.y);
                    if distance > far {
                        far = distance;
                        tip = *point;
                    }
                }
                let dx = (tip.x - center.x) / self.cell_width;
                let dy = (tip.y - center.y) / self.cell_height;
                if dx.abs() > dy.abs() {
                    if dx > 0.0 { '>' } else { '<' }
                } else if dy > 0.0 {
                    'v'
                } else {
                    '^'
                }
            }
        };

        let x = self.column(center.x);
        let y = self.line(center.y);
        self.glyphs
            .entry(y)
            .or_default()
            .insert(x, glyph.to_string());
        if let Some(row) = self.lines.get_mut(&y) {
            row.remove(&x);
        }
        if let Some(row) = self.slants.get_mut(&y) {
            row.remove(&x);
        }
    }

    /// Returns lines of text without trailing spaces or blank edges.
    pub(crate) fn print(&self) -> String {
        let mut rows = self
            .glyphs
            .keys()
            .chain(self.lines.keys())
            .chain(self.slants.keys())
            .copied()
            .collect::<Vec<i32>>();
        rows.sort_unstable();
        rows.dedup();
        if rows.is_empty() {
            return String::new();
        }

        let all = self
            .glyphs
            .values()
            .flat_map(|row| row.keys().copied())
            .chain(self.lines.values().flat_map(|row| row.keys().copied()))
            .chain(self.slants.values().flat_map(|row| row.keys().copied()))
            .collect::<Vec<i32>>();
        let first = all.iter().min().copied().unwrap_or(0);
        let top = rows[0];
        let bottom = rows[rows.len() - 1];

        let mut output = Vec::new();
        for y in top..=bottom {
            let mut last = first - 1;
            if let Some(row) = self.glyphs.get(&y)
                && let Some(max) = row.keys().max()
            {
                last = last.max(*max);
            }
            if let Some(row) = self.lines.get(&y)
                && let Some(max) = row.keys().max()
            {
                last = last.max(*max);
            }
            if let Some(row) = self.slants.get(&y)
                && let Some(max) = row.keys().max()
            {
                last = last.max(*max);
            }

            let mut cells = String::new();
            for x in first..=last {
                if let Some(glyph) = self.glyphs.get(&y).and_then(|row| row.get(&x)) {
                    cells.push_str(glyph);
                    continue;
                }

                let bits = self
                    .lines
                    .get(&y)
                    .and_then(|row| row.get(&x))
                    .copied()
                    .unwrap_or(0);
                let horizontal = bits & (LEFT | RIGHT) != 0;
                let vertical = bits & (UP | DOWN) != 0;
                let character = if horizontal && vertical {
                    '+'
                } else if horizontal {
                    '-'
                } else if vertical {
                    '|'
                } else {
                    self.slants
                        .get(&y)
                        .and_then(|row| row.get(&x))
                        .copied()
                        .unwrap_or(' ')
                };
                cells.push(character);
            }
            output.push(cells.trim_end().to_string());
        }

        while output.first().is_some_and(|line| line.is_empty()) {
            output.remove(0);
        }
        while output.last().is_some_and(|line| line.is_empty()) {
            output.pop();
        }

        format!("{}\n", output.join("\n"))
    }

    /// Draws a box with round sides: `.` and `'` corners, and `(` and `)` edges.
    pub(crate) fn round(&mut self, bounds: Bounds) {
        let x0 = self.column(bounds.left);
        let x1 = self.column(bounds.right);
        let y0 = self.line(bounds.top);
        let y1 = self.line(bounds.bottom);

        for x in x0 + 1..x1 {
            self.put(x, y0, '-');
            self.put(x, y1, '-');
        }
        for y in y0 + 1..y1 {
            self.put(x0, y, '(');
            self.put(x1, y, ')');
        }
        self.put(x0, y0, '.');
        self.put(x1, y0, '.');
        self.put(x0, y1, '\'');
        self.put(x1, y1, '\'');
    }

    /// Whether a circle is small enough to draw as one `o`, such as a commit or a start state.
    pub(crate) fn dot(&self, bounds: Bounds) -> bool {
        bounds.right - bounds.left < self.cell_width * 3.0
            && bounds.bottom - bounds.top < self.cell_height * 2.0
    }

    /// Whether a box is too small for its shape to show, as an arrowhead or a diamond is.
    pub(crate) fn small(&self, bounds: Bounds) -> bool {
        bounds.right - bounds.left < self.cell_width * 2.5
            && bounds.bottom - bounds.top < self.cell_height * 1.5
    }

    /// Draws the segments of `shape` as connected strokes.
    pub(crate) fn stroke(&mut self, paths: &[Vec<Point>]) {
        for shape in paths {
            for index in 1..shape.len() {
                self.segment(shape[index - 1], shape[index]);
            }
        }
    }

    /// Writes the lines of a label over strokes.
    pub(crate) fn text(&mut self, element: &Text) {
        for entry in &element.lines {
            let characters = entry
                .text
                .chars()
                .map(|character| {
                    if is_js_whitespace(character) {
                        ' '
                    } else {
                        character
                    }
                })
                .collect::<Vec<char>>();
            let width = characters
                .iter()
                .map(|character| columns(*character))
                .sum::<i32>();
            let offset = match element.anchor {
                Anchor::Middle => width as f64 / 2.0,
                Anchor::End => width as f64,
                Anchor::Start => 0.0,
            };
            // The baseline sits about a third of an em below the middle of the glyphs.
            let y = self.line(entry.y - element.size * 0.35);
            let mut x = js_round(entry.x / self.cell_width - offset);

            for character in characters {
                let span = columns(character);
                if span == 0 {
                    let previous = x - 1;
                    let existing = self
                        .glyphs
                        .entry(y)
                        .or_default()
                        .remove(&previous)
                        .unwrap_or_default();
                    self.glyphs
                        .entry(y)
                        .or_default()
                        .insert(previous, format!("{existing}{character}"));
                    continue;
                }

                self.glyphs
                    .entry(y)
                    .or_default()
                    .insert(x, character.to_string());
                // A wide character covers the next cell, which then prints nothing.
                if span == 2 {
                    self.glyphs
                        .entry(y)
                        .or_default()
                        .insert(x + 1, String::new());
                }
                if let Some(row) = self.lines.get_mut(&y) {
                    row.remove(&x);
                    if span == 2 {
                        row.remove(&(x + 1));
                    }
                }
                if let Some(row) = self.slants.get_mut(&y) {
                    row.remove(&x);
                    if span == 2 {
                        row.remove(&(x + 1));
                    }
                }
                x += span;
            }
        }
    }

    fn put(&mut self, x: i32, y: i32, character: char) {
        if self.glyphs.get(&y).is_some_and(|row| row.contains_key(&x)) {
            return;
        }
        self.glyphs
            .entry(y)
            .or_default()
            .insert(x, character.to_string());
        if let Some(row) = self.lines.get_mut(&y) {
            row.remove(&x);
        }
        if let Some(row) = self.slants.get_mut(&y) {
            row.remove(&x);
        }
    }

    fn column(&self, x: f64) -> i32 {
        (x / self.cell_width).floor() as i32
    }

    fn line(&self, y: f64) -> i32 {
        (y / self.cell_height).floor() as i32
    }

    fn connect(&mut self, x: i32, y: i32, bits: u8) {
        if self.glyphs.get(&y).is_some_and(|row| row.contains_key(&x)) {
            return;
        }
        let row = self.lines.entry(y).or_default();
        *row.entry(x).or_insert(0) |= bits;
    }

    /// Walks the cells a segment crosses, joining each to the next.
    fn segment(&mut self, from: Point, to: Point) {
        let mut x = self.column(from.x);
        let mut y = self.line(from.y);
        let end_x = self.column(to.x);
        let end_y = self.line(to.y);
        let dx = (to.x - from.x) / self.cell_width;
        let dy = (to.y - from.y) / self.cell_height;
        let step_x = js_sign(dx);
        let step_y = js_sign(dy);

        // A straight diagonal across several rows and columns reads better as slashes.
        let rows = (end_y - y).abs();
        let cols = (end_x - x).abs();
        let slant = dx.abs() >= 1.5 && dy.abs() >= 1.5;
        if rows > 0 && cols > 0 && slant && dx.abs() < 4.0 * dy.abs() {
            let character = if step_x == step_y { '\\' } else { '/' };
            for step in 0..=rows {
                let at = (from.x / self.cell_width + (dx * step as f64) / rows.max(1) as f64)
                    .floor() as i32;
                let target = y + step_y * step;
                if self
                    .glyphs
                    .get(&target)
                    .is_some_and(|row| row.contains_key(&at))
                {
                    continue;
                }
                // Two slants that meet at a corner, such as a diamond's, join as a plus sign.
                let cells = self.slants.entry(target).or_default();
                let existing = cells.get(&at).copied();
                cells.insert(
                    at,
                    if existing.is_none() || existing == Some(character) {
                        character
                    } else {
                        '+'
                    },
                );
            }
            return;
        }

        // Distance along the segment, as a fraction, to the next column and row boundary.
        let fx = from.x / self.cell_width;
        let fy = from.y / self.cell_height;
        let mut next_x = if dx == 0.0 {
            f64::INFINITY
        } else {
            ((if step_x > 0 { x + 1 } else { x }) as f64 - fx) / dx
        };
        let mut next_y = if dy == 0.0 {
            f64::INFINITY
        } else {
            ((if step_y > 0 { y + 1 } else { y }) as f64 - fy) / dy
        };
        let delta_x = if dx == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / dx).abs()
        };
        let delta_y = if dy == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / dy).abs()
        };

        for _ in 0..10_000 {
            if x == end_x && y == end_y {
                break;
            }
            if next_x < next_y {
                self.connect(x, y, if step_x > 0 { RIGHT } else { LEFT });
                x += step_x;
                self.connect(x, y, if step_x > 0 { LEFT } else { RIGHT });
                next_x += delta_x;
            } else {
                self.connect(x, y, if step_y > 0 { DOWN } else { UP });
                y += step_y;
                self.connect(x, y, if step_y > 0 { UP } else { DOWN });
                next_y += delta_y;
            }
        }
    }
}

/// Terminal columns a character takes: 0 for marks and joiners, 2 for wide characters.
fn columns(character: char) -> i32 {
    character.width().unwrap_or(0) as i32
}

fn is_js_whitespace(character: char) -> bool {
    character.is_whitespace() || character == '\u{FEFF}'
}

fn js_sign(value: f64) -> i32 {
    if value > 0.0 {
        1
    } else if value < 0.0 {
        -1
    } else {
        0
    }
}

fn js_round(value: f64) -> i32 {
    if value.is_finite() {
        (value + 0.5).floor() as i32
    } else {
        0
    }
}
