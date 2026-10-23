//! Renders compiled diagrams as terminal-friendly ASCII art.
//!
//! The compiler measures every glyph with a 0.6 em advance, so one character cell is 0.6 em wide
//! and 1 em high, and labels keep their column positions. Strokes become `-`, `|`, `+`, `/`, and
//! `\`, small filled marks become arrowheads and dots, and filled shapes hide what they cover, as
//! in the SVG paint order.

mod grid;
mod path;
mod svg;

/// A point in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Point {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

/// How a shape's fill affects the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fill {
    /// Hides what the shape covers.
    Background,
    /// Paints nothing.
    None,
    /// Paints a small mark, such as an arrowhead.
    Paint,
}

/// A label's horizontal alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Anchor {
    End,
    Middle,
    Start,
}

/// A drawn shape in document order, with its transform applied to its points.
#[derive(Clone, Debug)]
pub(crate) struct Shape {
    pub(crate) fill: Fill,
    /// Whether the shape is an edge marker, such as an arrowhead, or inside one.
    pub(crate) marker: bool,
    /// Subpaths as points in pixels. Closed subpaths repeat their first point.
    pub(crate) paths: Vec<Vec<Point>>,
    /// Circles and ellipses, which draw as boxes with round sides.
    pub(crate) round: bool,
    pub(crate) stroke: bool,
}

/// One line of a label.
#[derive(Clone, Debug)]
pub(crate) struct TextLine {
    pub(crate) text: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
}

/// A label with its anchor, lines, and font size.
#[derive(Clone, Debug)]
pub(crate) struct Text {
    pub(crate) anchor: Anchor,
    pub(crate) lines: Vec<TextLine>,
    pub(crate) size: f64,
}

/// A drawn element in document order.
#[derive(Clone, Debug)]
pub(crate) enum Element {
    Shape(Shape),
    Text(Text),
}

/// A box around a set of points, in pixels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub(crate) bottom: f64,
    pub(crate) left: f64,
    pub(crate) right: f64,
    pub(crate) top: f64,
}

/// An affine transform `[a, b, c, d, e, f]`, as SVG's `matrix()`.
pub(crate) type Matrix = [f64; 6];

/// Attributes of an SVG element, with its inline style resolved separately.
pub(crate) struct Attributes {
    pairs: Vec<(String, String)>,
    style: Vec<(String, String)>,
}

impl Attributes {
    pub(crate) fn new(pairs: Vec<(String, String)>) -> Self {
        let mut style = Vec::new();
        if let Some((_, value)) = pairs.iter().rev().find(|(name, _)| name == "style") {
            for declaration in value.split(';') {
                let Some(at) = declaration.find(':') else {
                    continue;
                };
                let name = declaration[..at].trim();
                let value = declaration[at + 1..].trim();
                if !name.is_empty() {
                    style.push((name.to_string(), value.to_string()));
                }
            }
        }
        Self { pairs, style }
    }

    /// Returns an attribute, with the last duplicate winning.
    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.pairs
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Returns a styled attribute, with inline style taking precedence.
    pub(crate) fn read(&self, name: &str) -> Option<&str> {
        self.style
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .or_else(|| self.get(name))
    }

    /// Parses an attribute as a number, defaulting to zero.
    pub(crate) fn number(&self, name: &str) -> f64 {
        self.get(name).and_then(path::parse_float).unwrap_or(0.0)
    }
}

/// Returns `svg`, a document the compiler emitted, as lines of text that end in a newline.
pub fn from_svg(svg: &str) -> String {
    let elements = svg::parse(svg);
    let size = font_size(&elements);
    let mut grid = grid::Grid::create(size, size * 0.6);
    let mut marks = Vec::new();

    for element in &elements {
        match element {
            Element::Text(text) => grid.text(text),
            Element::Shape(shape) => {
                let points = shape
                    .paths
                    .iter()
                    .flatten()
                    .copied()
                    .collect::<Vec<Point>>();
                let bounds = grid::bounds(&points);
                let filled = shape.fill != Fill::None;

                if shape.round && grid.dot(bounds) {
                    marks.push(shape);
                    continue;
                }

                // A small shape is a marker, such as an arrowhead, unless it only covers a label's
                // background.
                if (filled || shape.stroke) && grid.small(bounds) {
                    if shape.marker || shape.fill == Fill::Paint {
                        marks.push(shape);
                    }
                    continue;
                }

                if shape.fill == Fill::Background {
                    grid.fill(&shape.paths);
                }
                if shape.stroke && shape.round {
                    grid.round(bounds);
                } else if shape.stroke {
                    grid.stroke(&shape.paths);
                }
            }
        }
    }

    for mark in marks {
        grid.mark(mark);
    }

    grid.print()
}

/// Returns the font size most labels use, which sets the cell size.
fn font_size(elements: &[Element]) -> f64 {
    let mut counts: Vec<(f64, usize)> = Vec::new();
    for element in elements {
        if let Element::Text(text) = element {
            if let Some((_, count)) = counts.iter_mut().find(|(size, _)| *size == text.size) {
                *count += text.lines.len();
            } else {
                counts.push((text.size, text.lines.len()));
            }
        }
    }

    let mut best = 14.0;
    let mut most = 0;
    for (size, count) in counts {
        if count > most || (count == most && size < best) {
            best = size;
            most = count;
        }
    }
    best
}
