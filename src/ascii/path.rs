//! SVG path data, transforms, and shape geometry.

use std::f64::consts::PI;

use super::{Attributes, Matrix, Point};

/// The identity transform.
pub(crate) const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// Applies `matrix` to `point`.
pub(crate) fn apply(matrix: Matrix, point: Point) -> Point {
    let [a, b, c, d, e, f] = matrix;
    Point {
        x: a * point.x + c * point.y + e,
        y: b * point.x + d * point.y + f,
    }
}

/// Maps a point back through a matrix without skew.
pub(crate) fn invert(matrix: Matrix, point: Point) -> Point {
    let [a, b, c, d, e, f] = matrix;
    let determinant = a * d - b * c;
    let determinant = if determinant == 0.0 { 1.0 } else { determinant };
    let x = point.x - e;
    let y = point.y - f;
    Point {
        x: (d * x - c * y) / determinant,
        y: (-b * x + a * y) / determinant,
    }
}

/// Multiplies two matrices.
pub(crate) fn multiply(left: Matrix, right: Matrix) -> Matrix {
    let [a, b, c, d, e, f] = left;
    let [g, h, i, j, k, l] = right;
    [
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ]
}

/// Parses a `transform` attribute into one matrix.
pub(crate) fn transform(value: &str) -> Matrix {
    let mut matrix = IDENTITY;
    let mut rest = value;

    while let Some(open) = rest.find('(') {
        let name = rest[..open].trim();
        let Some(close) = rest[open + 1..].find(')') else {
            break;
        };
        let values = parse_numbers(&rest[open + 1..open + 1 + close]);
        let a = values.first().copied().unwrap_or(0.0);
        let b = values.get(1).copied();
        let c = values.get(2).copied().unwrap_or(0.0);

        let next = match name {
            "translate" => [1.0, 0.0, 0.0, 1.0, a, b.unwrap_or(0.0)],
            "scale" => [a, 0.0, 0.0, b.unwrap_or(a), 0.0, 0.0],
            "matrix" if values.len() == 6 => [
                values[0], values[1], values[2], values[3], values[4], values[5],
            ],
            "rotate" => {
                let radians = a * PI / 180.0;
                let cos = radians.cos();
                let sin = radians.sin();
                let x = b.unwrap_or(0.0);
                let y = c;
                [
                    cos,
                    sin,
                    -sin,
                    cos,
                    x - cos * x + sin * y,
                    y - sin * x - cos * y,
                ]
            }
            _ => IDENTITY,
        };

        matrix = multiply(matrix, next);
        rest = &rest[open + close + 2..];
    }

    matrix
}

/// Returns the subpaths of a shape element in its own coordinates.
pub(crate) fn geometry(tag: &str, attributes: &Attributes) -> Vec<Vec<Point>> {
    let number = |name: &str| attributes.get(name).and_then(parse_float).unwrap_or(0.0);

    match tag {
        "rect" => {
            let x = number("x");
            let y = number("y");
            let width = number("width");
            let height = number("height");
            if width <= 0.0 || height <= 0.0 {
                return Vec::new();
            }
            vec![vec![
                Point { x, y },
                Point { x: x + width, y },
                Point {
                    x: x + width,
                    y: y + height,
                },
                Point { x, y: y + height },
                Point { x, y },
            ]]
        }
        "circle" | "ellipse" => {
            let rx = if tag == "circle" {
                number("r")
            } else {
                number("rx")
            };
            let ry = if tag == "circle" {
                number("r")
            } else {
                number("ry")
            };
            if rx <= 0.0 || ry <= 0.0 {
                return Vec::new();
            }
            let cx = number("cx");
            let cy = number("cy");
            vec![
                (0..=48)
                    .map(|index| {
                        let angle = (index as f64 / 48.0) * 2.0 * PI;
                        Point {
                            x: cx + rx * angle.cos(),
                            y: cy + ry * angle.sin(),
                        }
                    })
                    .collect(),
            ]
        }
        "line" => vec![vec![
            Point {
                x: number("x1"),
                y: number("y1"),
            },
            Point {
                x: number("x2"),
                y: number("y2"),
            },
        ]],
        "polygon" | "polyline" => {
            let values = attributes
                .get("points")
                .map(parse_numbers)
                .unwrap_or_default();
            let mut points = Vec::new();
            let mut index = 0;
            while index + 1 < values.len() {
                points.push(Point {
                    x: values[index],
                    y: values[index + 1],
                });
                index += 2;
            }
            if tag == "polygon"
                && let Some(first) = points.first().copied()
            {
                points.push(first);
            }
            if points.len() > 1 {
                vec![points]
            } else {
                Vec::new()
            }
        }
        "path" => parse_path(attributes.get("d").unwrap_or("")),
        _ => Vec::new(),
    }
}

/// Flattens path data into subpaths. Curves and arcs become short straight segments.
pub(crate) fn parse_path(value: &str) -> Vec<Vec<Point>> {
    PathParser::new(tokenize(value)).run()
}

/// Parses the leading number in `value`, as JavaScript's `parseFloat` does.
pub(crate) fn parse_float(value: &str) -> Option<f64> {
    parse_number_prefix(value.trim_start()).map(|(number, _)| number)
}

/// Parses every number in `value`, ignoring separators and other text.
fn parse_numbers(value: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut index = 0;
    while index < value.len() {
        let byte = value.as_bytes()[index];
        if byte.is_ascii_whitespace() || byte == b',' {
            index += 1;
            continue;
        }
        if let Some((number, consumed)) = parse_number_prefix(&value[index..]) {
            numbers.push(number);
            index += consumed;
            continue;
        }
        let character = value[index..].chars().next().expect("valid UTF-8");
        index += character.len_utf8();
    }
    numbers
}

fn parse_number_prefix(value: &str) -> Option<(f64, usize)> {
    let bytes = value.as_bytes();
    let mut index = 0;

    if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
        index += 1;
    }

    let mut digits = false;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
        digits = true;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits = true;
        }
    }
    if !digits {
        return None;
    }

    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        let exponent_start = index;
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let digit_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == digit_start {
            index = exponent_start;
        }
    }

    let number = value[..index].parse::<f64>().ok()?;
    Some((number, index))
}

#[derive(Clone, Copy, Debug)]
enum Token {
    Command(char),
    Number(f64),
}

fn tokenize(value: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < value.len() {
        let byte = value.as_bytes()[index];
        if byte.is_ascii_whitespace() || byte == b',' {
            index += 1;
            continue;
        }
        if byte.is_ascii_alphabetic() && byte != b'e' && byte != b'E' {
            tokens.push(Token::Command(char::from(byte)));
            index += 1;
            continue;
        }
        if let Some((number, consumed)) = parse_number_prefix(&value[index..]) {
            tokens.push(Token::Number(number));
            index += consumed;
            continue;
        }
        let character = value[index..].chars().next().expect("valid UTF-8");
        index += character.len_utf8();
    }
    tokens
}

struct PathParser {
    control: Option<Point>,
    current: Vec<Point>,
    index: usize,
    paths: Vec<Vec<Point>>,
    point: Point,
    start: Point,
    tokens: Vec<Token>,
}

impl PathParser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            control: None,
            current: Vec::new(),
            index: 0,
            paths: Vec::new(),
            point: Point { x: 0.0, y: 0.0 },
            start: Point { x: 0.0, y: 0.0 },
            tokens,
        }
    }

    fn run(mut self) -> Vec<Vec<Point>> {
        let mut command: Option<char> = None;

        while self.index < self.tokens.len() {
            if let Some(Token::Command(character)) = self.tokens.get(self.index).copied() {
                command = Some(character);
                self.index += 1;
            } else if command.is_none() {
                break;
            }

            let Some(character) = command else {
                break;
            };
            let relative = character.is_ascii_lowercase();
            let base = if relative {
                self.point
            } else {
                Point { x: 0.0, y: 0.0 }
            };
            let upper = character.to_ascii_uppercase();

            if upper == 'Z' {
                if !self.current.is_empty() {
                    self.to(self.start);
                    self.paths.push(std::mem::take(&mut self.current));
                }
                self.current = vec![self.start];
                self.control = None;
                continue;
            }

            if !self.has() {
                self.index += 1;
                continue;
            }

            if upper == 'M' {
                if self.current.len() > 1 {
                    self.paths.push(std::mem::take(&mut self.current));
                }
                self.point = self.at(base);
                self.start = self.point;
                self.current = vec![self.point];
                command = Some(if relative { 'l' } else { 'L' });
                self.control = None;
                continue;
            }

            if upper == 'L' {
                let point = self.at(base);
                self.to(point);
            } else if upper == 'H' {
                let x = if relative { self.point.x } else { 0.0 } + self.next();
                let y = self.point.y;
                self.to(Point { x, y });
            } else if upper == 'V' {
                let y = if relative { self.point.y } else { 0.0 } + self.next();
                let x = self.point.x;
                self.to(Point { x, y });
            } else if upper == 'C' {
                let first = self.at(base);
                let second = self.at(base);
                let end = self.at(base);
                self.curve(&[first, second, end]);
                self.control = Some(second);
                continue;
            } else if upper == 'S' {
                let first = self.control.map_or(self.point, |control| Point {
                    x: 2.0 * self.point.x - control.x,
                    y: 2.0 * self.point.y - control.y,
                });
                let second = self.at(base);
                let end = self.at(base);
                self.curve(&[first, second, end]);
                self.control = Some(second);
                continue;
            } else if upper == 'Q' {
                let first = self.at(base);
                let end = self.at(base);
                self.curve(&[first, end]);
                self.control = Some(first);
                continue;
            } else if upper == 'T' {
                let first = self.control.map_or(self.point, |control| Point {
                    x: 2.0 * self.point.x - control.x,
                    y: 2.0 * self.point.y - control.y,
                });
                let end = self.at(base);
                self.curve(&[first, end]);
                self.control = Some(first);
                continue;
            } else if upper == 'A' {
                let rx = self.next().abs();
                let ry = self.next().abs();
                let rotation = self.next();
                let large = self.next() != 0.0;
                let sweep = self.next() != 0.0;
                let end = self.at(base);
                self.current
                    .extend(arc(self.point, end, rx, ry, rotation, large, sweep));
                self.point = end;
            } else {
                self.index += 1;
            }

            self.control = None;
        }

        if self.current.len() > 1 {
            self.paths.push(self.current);
        }
        self.paths
    }

    fn next(&mut self) -> f64 {
        let value = match self.tokens.get(self.index) {
            Some(Token::Number(number)) => *number,
            _ => f64::NAN,
        };
        self.index += 1;
        value
    }

    fn has(&self) -> bool {
        matches!(self.tokens.get(self.index), Some(Token::Number(_)))
    }

    fn at(&mut self, base: Point) -> Point {
        Point {
            x: base.x + self.next(),
            y: base.y + self.next(),
        }
    }

    fn to(&mut self, point: Point) {
        self.current.push(point);
        self.point = point;
    }

    fn curve(&mut self, points: &[Point]) {
        let mut level = Vec::with_capacity(points.len() + 1);
        level.push(self.point);
        level.extend_from_slice(points);

        for step in 1..=16 {
            let t = step as f64 / 16.0;
            let mut current = level.clone();
            while current.len() > 1 {
                current = current
                    .windows(2)
                    .map(|pair| Point {
                        x: pair[0].x + (pair[1].x - pair[0].x) * t,
                        y: pair[0].y + (pair[1].y - pair[0].y) * t,
                    })
                    .collect();
            }
            self.current.push(current[0]);
        }

        if let Some(last) = points.last() {
            self.point = *last;
        }
    }
}

/// Returns points along an elliptical arc after its start, with SVG's endpoint parameters.
fn arc(
    from: Point,
    to: Point,
    mut rx: f64,
    mut ry: f64,
    rotation: f64,
    large: bool,
    sweep: bool,
) -> Vec<Point> {
    if rx == 0.0 || ry == 0.0 {
        return vec![to];
    }

    let phi = rotation * PI / 180.0;
    let cos = phi.cos();
    let sin = phi.sin();
    let dx = (from.x - to.x) / 2.0;
    let dy = (from.y - to.y) / 2.0;
    let x1 = cos * dx + sin * dy;
    let y1 = -sin * dx + cos * dy;

    let scale = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if scale > 1.0 {
        rx *= scale.sqrt();
        ry *= scale.sqrt();
    }

    let numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let denominator = if denominator == 0.0 { 1.0 } else { denominator };
    let factor =
        if large == sweep { -1.0 } else { 1.0 } * (numerator / denominator).max(0.0).sqrt();
    let cx1 = factor * rx * y1 / ry;
    let cy1 = -factor * ry * x1 / rx;
    let cx = cos * cx1 - sin * cy1 + (from.x + to.x) / 2.0;
    let cy = sin * cx1 + cos * cy1 + (from.y + to.y) / 2.0;

    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let theta = angle(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut delta = angle(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    if !sweep && delta > 0.0 {
        delta -= 2.0 * PI;
    }
    if sweep && delta < 0.0 {
        delta += 2.0 * PI;
    }

    let steps = ((delta.abs() / PI) * 24.0).ceil().max(4.0) as usize;
    let mut points = Vec::with_capacity(steps);
    for step in 1..=steps {
        let t = theta + (delta * step as f64) / steps as f64;
        let x = rx * t.cos();
        let y = ry * t.sin();
        points.push(Point {
            x: cos * x - sin * y + cx,
            y: sin * x + cos * y + cy,
        });
    }
    points
}
