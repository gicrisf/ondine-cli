//! Scans the compiler's SVG into shapes and labels in paint order.

use super::path::{self, IDENTITY};
use super::{Anchor, Attributes, Element, Fill, Matrix, Point, Shape, Text, TextLine};

#[derive(Clone)]
struct Frame {
    fill: String,
    hidden: bool,
    marker: bool,
    matrix: Matrix,
    size: f64,
    stroke: String,
}

impl Frame {
    fn root() -> Self {
        Self {
            fill: String::from("black"),
            hidden: false,
            marker: false,
            matrix: IDENTITY,
            size: 16.0,
            stroke: String::from("none"),
        }
    }
}

struct TextBuilder {
    anchor: Anchor,
    lines: Vec<TextLine>,
    size: f64,
}

/// Returns the shapes and labels of `svg` in paint order.
pub(crate) fn parse(svg: &str) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut stack = vec![Frame::root()];
    let mut text: Option<TextBuilder> = None;
    let mut cursor = Point { x: 0.0, y: 0.0 };
    let mut index = 0;

    while index < svg.len() {
        if svg.as_bytes()[index] != b'<' {
            let next = svg[index..]
                .find('<')
                .map_or(svg.len(), |offset| index + offset);
            let content = &svg[index..next];

            if let Some(builder) = text.as_mut()
                && let Some(parent) = stack.last()
                && !parent.hidden
            {
                let value = decode(content);
                if let Some(line) = builder.lines.last_mut() {
                    line.text.push_str(&value);
                } else {
                    builder.lines.push(TextLine {
                        text: value,
                        x: cursor.x,
                        y: cursor.y,
                    });
                }
            }

            index = next;
            continue;
        }

        if svg[index..].starts_with("<!--") {
            index = svg[index..]
                .find("-->")
                .map_or(svg.len(), |offset| index + offset + 3);
            continue;
        }

        if svg[index..].starts_with("<![CDATA[") {
            let start = index + "<![CDATA[".len();
            let end = svg[start..]
                .find("]]>")
                .map_or(svg.len(), |offset| start + offset);
            if let Some(builder) = text.as_mut()
                && let Some(parent) = stack.last()
                && !parent.hidden
            {
                let value = &svg[start..end];
                if let Some(line) = builder.lines.last_mut() {
                    line.text.push_str(value);
                } else {
                    builder.lines.push(TextLine {
                        text: value.to_string(),
                        x: cursor.x,
                        y: cursor.y,
                    });
                }
            }
            index = (end + 3).min(svg.len());
            continue;
        }

        if svg[index..].starts_with("<!") || svg[index..].starts_with("<?") {
            index = svg[index..]
                .find('>')
                .map_or(svg.len(), |offset| index + offset + 1);
            continue;
        }

        if svg[index..].starts_with("</") {
            let Some(end) = svg[index..].find('>') else {
                break;
            };
            let tag = svg[index + 2..index + end].trim();
            index += end + 1;
            stack.pop();

            if tag == "text"
                && let Some(builder) = text.take()
                && builder
                    .lines
                    .iter()
                    .any(|line| !line.text.trim().is_empty())
            {
                elements.push(Element::Text(Text {
                    anchor: builder.anchor,
                    lines: builder.lines,
                    size: builder.size,
                }));
            }
            continue;
        }

        let (tag, attributes, self_closing, end) = parse_open_tag(svg, index);
        index = end;
        if tag.is_empty() {
            continue;
        }

        let parent = stack.last().cloned().unwrap_or_else(Frame::root);
        let fill = attributes
            .read("fill")
            .map_or_else(|| parent.fill.clone(), str::to_string);
        let hidden = parent.hidden
            || matches!(
                tag.as_str(),
                "clipPath" | "defs" | "desc" | "marker" | "mask" | "pattern" | "style" | "title"
            )
            || attributes.read("display") == Some("none")
            || attributes.read("visibility") == Some("hidden");
        let marker = parent.marker
            || attributes
                .get("class")
                .is_some_and(|class| class.split_whitespace().any(|word| word == "ondine-marker"));
        let matrix = path::multiply(
            parent.matrix,
            path::transform(attributes.get("transform").unwrap_or("")),
        );
        let size = attributes
            .read("font-size")
            .and_then(path::parse_float)
            .filter(|value| *value != 0.0)
            .unwrap_or(parent.size);
        let stroke = attributes
            .read("stroke")
            .map_or_else(|| parent.stroke.clone(), str::to_string);
        let frame = Frame {
            fill,
            hidden,
            marker,
            matrix,
            size,
            stroke,
        };

        if !self_closing {
            stack.push(frame.clone());
        }
        if frame.hidden {
            continue;
        }

        if tag == "text" {
            let anchor = match attributes.read("text-anchor") {
                Some("middle") => Anchor::Middle,
                Some("end") => Anchor::End,
                _ => Anchor::Start,
            };
            text = Some(TextBuilder {
                anchor,
                lines: Vec::new(),
                size: frame.size,
            });
            cursor = path::apply(
                frame.matrix,
                Point {
                    x: attributes.number("x"),
                    y: attributes.number("y"),
                },
            );
            // Rotated labels, such as axis titles, do not fit in rows of text.
            if frame.matrix[1].abs() > 1e-6 {
                text = None;
            }
            continue;
        }

        if tag == "tspan"
            && let Some(builder) = text.as_mut()
        {
            if attributes.get("x").is_none() && attributes.get("y").is_none() {
                continue;
            }
            let local = path::invert(frame.matrix, cursor);
            let x = if attributes.get("x").is_none() {
                local.x
            } else {
                attributes.number("x")
            };
            let y = if attributes.get("y").is_none() {
                local.y
            } else {
                attributes.number("y")
            } + attributes.number("dy");
            cursor = path::apply(frame.matrix, Point { x, y });
            builder.lines.push(TextLine {
                text: String::new(),
                x: cursor.x,
                y: cursor.y,
            });
            continue;
        }

        let paths = path::geometry(&tag, &attributes);
        if paths.is_empty() {
            continue;
        }

        elements.push(Element::Shape(Shape {
            fill: classify_fill(&frame.fill),
            marker: frame.marker,
            paths: paths
                .into_iter()
                .map(|path| {
                    path.into_iter()
                        .map(|point| path::apply(frame.matrix, point))
                        .collect()
                })
                .collect(),
            round: tag == "circle" || tag == "ellipse",
            stroke: frame.stroke.trim() != "none"
                && attributes
                    .read("stroke-width")
                    .is_none_or(|value| value != "0"),
        }));
    }

    elements
}

fn classify_fill(value: &str) -> Fill {
    let fill = value.trim().to_ascii_lowercase();
    if fill == "none" || fill == "transparent" {
        Fill::None
    } else if matches!(
        fill.as_str(),
        "#fff" | "#ffffff" | "var(--_bg)" | "var(--_s)" | "white"
    ) {
        Fill::Background
    } else {
        Fill::Paint
    }
}

/// Parses an opening tag, returning its name, attributes, self-closing flag, and end offset.
fn parse_open_tag(svg: &str, start: usize) -> (String, Attributes, bool, usize) {
    let mut index = start + 1;
    while index < svg.len() && svg.as_bytes()[index].is_ascii_whitespace() {
        index += 1;
    }

    let tag_start = index;
    while index < svg.len() {
        let byte = svg.as_bytes()[index];
        if byte.is_ascii_whitespace() || byte == b'/' || byte == b'>' {
            break;
        }
        index += 1;
    }
    let tag = svg[tag_start..index].to_string();
    let mut pairs = Vec::new();

    loop {
        while index < svg.len() && svg.as_bytes()[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= svg.len() {
            return (tag, Attributes::new(pairs), false, index);
        }

        let byte = svg.as_bytes()[index];
        if byte == b'>' {
            return (tag, Attributes::new(pairs), false, index + 1);
        }
        if byte == b'/' {
            if index + 1 < svg.len() && svg.as_bytes()[index + 1] == b'>' {
                return (tag, Attributes::new(pairs), true, index + 2);
            }
            index += 1;
            continue;
        }

        let name_start = index;
        while index < svg.len() {
            let byte = svg.as_bytes()[index];
            if byte.is_ascii_whitespace() || byte == b'=' || byte == b'/' || byte == b'>' {
                break;
            }
            index += 1;
        }
        let name = svg[name_start..index].to_string();

        while index < svg.len() && svg.as_bytes()[index].is_ascii_whitespace() {
            index += 1;
        }

        let mut value = String::new();
        if index < svg.len() && svg.as_bytes()[index] == b'=' {
            index += 1;
            while index < svg.len() && svg.as_bytes()[index].is_ascii_whitespace() {
                index += 1;
            }
            if index < svg.len() {
                let quote = svg.as_bytes()[index];
                if quote == b'"' || quote == b'\'' {
                    index += 1;
                    let value_start = index;
                    while index < svg.len() && svg.as_bytes()[index] != quote {
                        index += 1;
                    }
                    value = svg[value_start..index].to_string();
                    if index < svg.len() {
                        index += 1;
                    }
                } else {
                    let value_start = index;
                    while index < svg.len() {
                        let byte = svg.as_bytes()[index];
                        if byte.is_ascii_whitespace() || byte == b'>' || byte == b'/' {
                            break;
                        }
                        index += 1;
                    }
                    value = svg[value_start..index].to_string();
                }
            }
        }

        if !name.is_empty() {
            pairs.push((name, value));
        }
    }
}

/// Replaces the XML entities that the compiler emits with their characters.
fn decode(text: &str) -> String {
    let mut output = String::new();
    let mut index = 0;

    while index < text.len() {
        if text.as_bytes()[index] == b'&'
            && let Some((character, length)) = decode_entity(&text[index..])
        {
            output.push(character);
            index += length;
            continue;
        }

        let character = text[index..].chars().next().expect("valid UTF-8");
        output.push(character);
        index += character.len_utf8();
    }

    output
}

fn decode_entity(value: &str) -> Option<(char, usize)> {
    let end = value[1..].find(';')? + 1;
    let entity = &value[1..end];

    let character = if let Some(hex) = entity
        .strip_prefix("#x")
        .or_else(|| entity.strip_prefix("#X"))
    {
        char::from_u32(u32::from_str_radix(hex, 16).ok()?)?
    } else if let Some(decimal) = entity.strip_prefix('#') {
        char::from_u32(decimal.parse().ok()?)?
    } else if entity.eq_ignore_ascii_case("amp") {
        '&'
    } else if entity.eq_ignore_ascii_case("lt") {
        '<'
    } else if entity.eq_ignore_ascii_case("gt") {
        '>'
    } else if entity.eq_ignore_ascii_case("quot") {
        '"'
    } else if entity.eq_ignore_ascii_case("apos") {
        '\''
    } else {
        return None;
    };

    Some((character, end + 1))
}
