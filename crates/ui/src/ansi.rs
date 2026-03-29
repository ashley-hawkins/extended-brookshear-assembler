use egui::{Color32, FontSelection, RichText};

pub const BLACK: egui::Color32 = egui::Color32::from_rgb(0, 0, 0);
pub const RED: egui::Color32 = egui::Color32::from_rgb(205, 0, 0);
pub const GREEN: egui::Color32 = egui::Color32::from_rgb(0, 205, 0);
pub const YELLOW: egui::Color32 = egui::Color32::from_rgb(205, 205, 0);
pub const BLUE: egui::Color32 = egui::Color32::from_rgb(0, 0, 238);
pub const MAGENTA: egui::Color32 = egui::Color32::from_rgb(205, 0, 205);
pub const CYAN: egui::Color32 = egui::Color32::from_rgb(0, 205, 205);
pub const WHITE: egui::Color32 = egui::Color32::from_rgb(229, 229, 229);
pub const BRIGHT_BLACK: egui::Color32 = egui::Color32::from_rgb(127, 127, 127);
pub const BRIGHT_RED: egui::Color32 = egui::Color32::from_rgb(255, 0, 0);
pub const BRIGHT_GREEN: egui::Color32 = egui::Color32::from_rgb(0, 255, 0);
pub const BRIGHT_YELLOW: egui::Color32 = egui::Color32::from_rgb(255, 255, 0);
pub const BRIGHT_BLUE: egui::Color32 = egui::Color32::from_rgb(92, 92, 255);
pub const BRIGHT_MAGENTA: egui::Color32 = egui::Color32::from_rgb(255, 0, 255);
pub const BRIGHT_CYAN: egui::Color32 = egui::Color32::from_rgb(0, 255, 255);
pub const BRIGHT_WHITE: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);

// Standard Colours: https://en.wikipedia.org/wiki/ANSI_escape_code#3-bit_and_4-bit
pub const STANDARD_COLOURS: [Color32; 16] = [
    BLACK,
    RED,
    GREEN,
    YELLOW,
    BLUE,
    MAGENTA,
    CYAN,
    WHITE,
    BRIGHT_BLACK,
    BRIGHT_RED,
    BRIGHT_GREEN,
    BRIGHT_YELLOW,
    BRIGHT_BLUE,
    BRIGHT_MAGENTA,
    BRIGHT_CYAN,
    BRIGHT_WHITE,
];

fn color_standard(n: u8) -> Color32 {
    if n > 15 {
        panic!("Invalid standard color code: {}", n);
    }
    STANDARD_COLOURS[n as usize]
}

fn color_8bit(n: u8) -> Color32 {
    if n < 16 {
        color_standard(n)
    } else if n < 232 {
        let n = n - 16;
        let r = n / 36;
        let g = (n % 36) / 6;
        let b = n % 6;
        let r = if r == 0 { 0 } else { r * 40 + 55 };
        let g = if g == 0 { 0 } else { g * 40 + 55 };
        let b = if b == 0 { 0 } else { b * 40 + 55 };

        Color32::from_rgb(r, g, b)
    } else {
        let gray = (n - 232) * 10 + 8;

        Color32::from_rgb(gray, gray, gray)
    }
}

struct FormattedTextSegment {
    range: std::ops::Range<usize>,
    fg: Option<Color32>,
    bg: Option<Color32>,
}

pub struct MyRichText {
    text: String,
    segments: Vec<FormattedTextSegment>,
}

impl MyRichText {
    pub fn layout(&self, style: &egui::Style) -> egui::text::LayoutJob {
        egui::text::LayoutJob {
            text: self.text.clone(),
            sections: self
                .segments
                .iter()
                .map(|segment| {
                    let color = segment.fg.unwrap_or(style.visuals.text_color());
                    let bg = segment.bg.unwrap_or(Color32::TRANSPARENT);

                    egui::text::LayoutSection {
                        leading_space: 0.0,
                        byte_range: segment.range.clone(),
                        format: egui::TextFormat {
                            font_id: FontSelection::default().resolve(style),
                            color,
                            background: bg,
                            ..Default::default()
                        },
                    }
                })
                .collect(),
            ..Default::default()
        }
    }
}

struct RichTextPerformer {
    segments: Vec<FormattedTextSegment>,
    current_fg: Option<Color32>,
    current_bg: Option<Color32>,
    text: String,
}

impl RichTextPerformer {
    fn commit_current_text(&mut self) {
        if !self.text.is_empty() {
            let range = self.segments.last().map(|seg| seg.range.end).unwrap_or(0)..self.text.len();
            self.segments.push(FormattedTextSegment {
                range,
                fg: self.current_fg,
                bg: self.current_bg,
            });
        }
    }

    fn reset(&mut self) {
        self.commit_current_text();
        self.current_bg = None;
        self.current_fg = None;
    }

    fn set_fg(&mut self, color: Color32) {
        self.commit_current_text();
        self.current_fg = Some(color);
    }

    fn set_bg(&mut self, color: Color32) {
        self.commit_current_text();
        self.current_bg = Some(color);
    }

    fn consume(mut self) -> MyRichText {
        self.commit_current_text();
        MyRichText {
            text: self.text,
            segments: self.segments,
        }
    }
}

impl anstyle_parse::Perform for RichTextPerformer {
    fn print(&mut self, _c: char) {
        self.text.push(_c);
    }

    fn execute(&mut self, _byte: u8) {
        if _byte.is_ascii_whitespace() {
            self.print(_byte as char);
        }
    }

    fn csi_dispatch(
        &mut self,
        params: &anstyle_parse::Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: u8,
    ) {
        let mut parse_params = |params: &anstyle_parse::Params| -> Option<(bool, Color32)> {
            let mut iter = params.iter();
            let is_bg = match iter.next() {
                Some(&[0]) => {
                    self.reset();
                    return None;
                }
                Some(&[38]) => false,
                Some(&[48]) => true,
                Some(&[x @ 30..38]) => {
                    return Some((false, color_standard((x - 30).try_into().unwrap())));
                }
                Some(&[x @ 90..98]) => {
                    return Some((false, color_standard((x - 90 + 8).try_into().unwrap())));
                }
                Some(&[x @ 40..48]) => {
                    return Some((true, color_standard((x - 40).try_into().unwrap())));
                }
                Some(&[x @ 100..108]) => {
                    return Some((true, color_standard((x - 100 + 8).try_into().unwrap())));
                }
                _ => return None,
            };

            let is_8bit = match iter.next() {
                Some(&[5]) => true,
                Some(&[2]) => false,
                _ => return None,
            };

            if is_8bit {
                let n = match iter.next() {
                    Some(&[n]) => n,
                    _ => return None,
                };
                Some((is_bg, color_8bit(n.try_into().unwrap())))
            } else {
                let r = match iter.next() {
                    Some(&[r]) => r,
                    _ => return None,
                };
                let g = match iter.next() {
                    Some(&[g]) => g,
                    _ => return None,
                };
                let b = match iter.next() {
                    Some(&[b]) => b,
                    _ => return None,
                };
                Some((
                    is_bg,
                    Color32::from_rgb(
                        r.try_into().unwrap(),
                        g.try_into().unwrap(),
                        b.try_into().unwrap(),
                    ),
                ))
            }
        };

        let Some((is_bg, color)) = parse_params(params) else {
            return;
        };

        if is_bg {
            self.set_bg(color);
        } else {
            self.set_fg(color);
        }
    }
}

pub fn ansi_to_rich_text(ansi_string: &str) -> MyRichText {
    let mut performer = RichTextPerformer {
        segments: Vec::new(),
        current_bg: None,
        current_fg: None,
        text: String::new(),
    };

    let mut parser = anstyle_parse::Parser::<anstyle_parse::Utf8Parser>::new();
    for byte in ansi_string.bytes() {
        parser.advance(&mut performer, byte);
    }

    performer.consume()
}
