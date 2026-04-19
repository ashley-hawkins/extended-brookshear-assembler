use std::hash::Hash;

use brookshear_machine::{BrookshearMachine, float8_to_string, string_to_float8};
use egui::{Align, Frame, Layout, ScrollArea};
use egui_extras::Column;

use super::App;

#[derive(Clone, Copy, Default)]
pub enum TableHighlight {
    #[default]
    None,
    Row(usize),
    Cell {
        row: usize,
        column: usize,
    },
}

impl TableHighlight {
    fn row(self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Row(row) | Self::Cell { row, .. } => Some(row),
        }
    }

    fn cell(self) -> Option<(usize, usize)> {
        match self {
            Self::Cell { row, column } => Some((row, column)),
            Self::None | Self::Row(_) => None,
        }
    }
}

#[derive(Default)]
pub struct EditableTableState {
    highlight: TableHighlight,
    is_focused: bool,
    editing_cell: Option<((usize, usize), String)>,
    should_grab_focus: bool,
}

impl EditableTableState {
    pub fn clear_highlight(&mut self) {
        self.highlight = TableHighlight::None;
        self.is_focused = false;
    }

    pub fn set_highlighted_row(&mut self, row: usize) {
        self.highlight = TableHighlight::Row(row);
        self.is_focused = false;
    }

    pub fn set_highlighted_cell(&mut self, row: usize, column: usize) {
        self.highlight = TableHighlight::Cell { row, column };
        self.is_focused = true;
    }
}

trait TableColumn<Row, Context>: Copy {
    fn header(self) -> &'static str;

    fn column(self) -> Column;

    fn display(self, row_index: usize, row: &Row, context: &Context) -> String;

    fn is_editable(self) -> bool {
        false
    }

    fn try_set(self, _row_index: usize, _row: &mut Row, _value: &str, _context: &Context) -> bool {
        false
    }
}

struct SelectableTable<'a, Row, Col, Context> {
    id: egui::Id,
    columns: &'a [Col],
    rows: &'a mut [Row],
    state: &'a mut EditableTableState,
    context: &'a Context,
    row_height: f32,
    min_scrolled_height: f32,
    scroll_to_row: Option<usize>,
}

impl<'a, Row, Col, Context> SelectableTable<'a, Row, Col, Context>
where
    Col: TableColumn<Row, Context>,
{
    fn new(
        id_salt: impl Hash,
        columns: &'a [Col],
        rows: &'a mut [Row],
        state: &'a mut EditableTableState,
        context: &'a Context,
    ) -> Self {
        Self {
            id: egui::Id::new(id_salt),
            columns,
            rows,
            state,
            context,
            row_height: 20.0,
            min_scrolled_height: 80.0,
            scroll_to_row: None,
        }
    }

    fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }

    fn min_scrolled_height(mut self, min_scrolled_height: f32) -> Self {
        self.min_scrolled_height = min_scrolled_height;
        self
    }

    fn scroll_to_row(mut self, scroll_to_row: Option<usize>) -> Self {
        self.scroll_to_row = scroll_to_row;
        self
    }

    fn show(self, ui: &mut egui::Ui) {
        let row_count = self.rows.len();
        let clicked_elsewhere = ui.ctx().input(|i| i.pointer.any_pressed());
        let table_region = ui.available_rect_before_wrap();
        let pointer_in_table = ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|pos| table_region.contains(pos));

        ScrollArea::horizontal().show(ui, |ui| {
            let mut table_builder = egui_extras::TableBuilder::new(ui)
                .id_salt(self.id)
                .min_scrolled_height(self.min_scrolled_height)
                .striped(true);

            for column in self.columns {
                table_builder = table_builder.column(column.column());
            }

            if let Some(row) = self.scroll_to_row {
                table_builder = table_builder.scroll_to_row(row, None);
            }

            let mut table = table_builder.header(20.0, |mut row| {
                for column in self.columns {
                    row.col(|ui| {
                        ui.centered_and_justified(|ui| {
                            ui.add(
                                egui::Label::new(column.header())
                                    .truncate()
                                    .show_tooltip_when_elided(true),
                            );
                        });
                    });
                }
            });

            table.ui_mut().style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            if clicked_elsewhere && !pointer_in_table && self.state.editing_cell.is_none() {
                self.state.is_focused = false;
            }

            table.body(|body| {
                body.rows(self.row_height, self.rows.len(), |mut row_ui| {
                    let row_index = row_ui.index();
                    if Some(row_index) == self.state.highlight.row() {
                        row_ui.set_selected(true);
                    }

                    let row = &mut self.rows[row_index];
                    for (column_index, column) in self.columns.iter().copied().enumerate() {
                        render_table_cell(
                            &mut row_ui,
                            self.state,
                            row_index,
                            column_index,
                            row_count,
                            row,
                            column,
                            self.context,
                        );
                    }
                });
            });
        });
    }
}

fn render_table_cell<Row, Col, Context>(
    row_ui: &mut egui_extras::TableRow<'_, '_>,
    state: &mut EditableTableState,
    row_index: usize,
    column_index: usize,
    row_count: usize,
    row: &mut Row,
    column: Col,
    context: &Context,
) where
    Col: TableColumn<Row, Context>,
{
    row_ui.col(|ui| {
        let cell = (row_index, column_index);
        let is_highlighted_cell = state.highlight.cell() == Some(cell);

        if column.is_editable()
            && let Some((cell_being_edited, edit_str)) = &mut state.editing_cell
            && *cell_being_edited == cell
        {
            let response = ui.text_edit_singleline(edit_str);
            if state.should_grab_focus {
                response.request_focus();
                state.should_grab_focus = false;
            }
            if response.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let _ = column.try_set(row_index, row, edit_str, context);
                    state.set_highlighted_cell(
                        (row_index + 1).min(row_count.saturating_sub(1)),
                        column_index,
                    );
                }
                state.editing_cell = None;
            }
            return;
        }

        if is_highlighted_cell
            && column.is_editable()
            && state.is_focused
            && state.editing_cell.is_none()
            && let Some(text) = ui.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Text(text) if !text.is_empty() => Some(text.clone()),
                    _ => None,
                })
            })
        {
            state.editing_cell = Some((cell, text));
            state.should_grab_focus = true;
            return;
        }

        ui.centered_and_justified(|ui| {
            let value = column.display(row_index, row, context);
            let response = egui::Frame::new()
                .fill(if is_highlighted_cell {
                    ui.visuals().selection.bg_fill
                } else {
                    egui::Color32::TRANSPARENT
                })
                .stroke(if is_highlighted_cell {
                    ui.visuals().selection.stroke
                } else {
                    egui::Stroke::NONE
                })
                .inner_margin(egui::Margin::symmetric(4, 1))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(value.clone()).sense(egui::Sense::click()))
                })
                .inner;
            if response.clicked() {
                state.set_highlighted_cell(row_index, column_index);
            }
            if column.is_editable() && response.double_clicked() {
                state.editing_cell = Some((cell, value));
                state.should_grab_focus = true;
            }
        });
    });
}

#[derive(Clone, Copy)]
enum RegisterColumn {
    Register,
    Binary,
    Hex,
    UnsignedDecimal,
    SignedDecimal,
    Float,
    Ascii,
}

const REGISTER_COLUMNS: [RegisterColumn; 7] = [
    RegisterColumn::Register,
    RegisterColumn::Binary,
    RegisterColumn::Hex,
    RegisterColumn::UnsignedDecimal,
    RegisterColumn::SignedDecimal,
    RegisterColumn::Float,
    RegisterColumn::Ascii,
];

impl TableColumn<u8, ()> for RegisterColumn {
    fn header(self) -> &'static str {
        match self {
            Self::Register => "Register",
            Self::Binary => "Binary",
            Self::Hex => "Hex",
            Self::UnsignedDecimal => "Unsigned Decimal",
            Self::SignedDecimal => "Signed Decimal",
            Self::Float => "Float",
            Self::Ascii => "ASCII",
        }
    }

    fn column(self) -> Column {
        match self {
            Self::Register => Column::auto().resizable(true).at_least(20.0),
            Self::Binary => Column::auto().resizable(true).at_least(64.0),
            Self::Hex => Column::auto().resizable(true).at_least(22.0),
            Self::UnsignedDecimal | Self::SignedDecimal => {
                Column::auto_with_initial_suggestion(50.0)
                    .resizable(true)
                    .at_least(30.0)
            }
            Self::Float => Column::auto().resizable(true).at_least(30.0),
            Self::Ascii => Column::remainder()
                .resizable(true)
                .clip(true)
                .at_least(30.0),
        }
    }

    fn display(self, row_index: usize, row: &u8, _context: &()) -> String {
        match self {
            Self::Register => format!("{:X}", row_index),
            Self::Binary => format!("{:08b}", row),
            Self::Hex => format!("{:02X}", row),
            Self::UnsignedDecimal => format!("{}", row),
            Self::SignedDecimal => format!("{}", *row as i8),
            Self::Float => float8_to_string(*row),
            Self::Ascii => byte_to_ascii(*row),
        }
    }

    fn is_editable(self) -> bool {
        !matches!(self, Self::Register)
    }

    fn try_set(self, _row_index: usize, row: &mut u8, value: &str, _context: &()) -> bool {
        let parsed = match self {
            Self::Register => None,
            Self::Binary => u8::from_str_radix(value, 2).ok(),
            Self::Hex => u8::from_str_radix(value, 16).ok(),
            Self::UnsignedDecimal => value.parse::<u64>().ok().map(|v| v.rem_euclid(256) as u8),
            Self::SignedDecimal => value.parse::<i8>().ok().map(|v| v as u8),
            Self::Float => string_to_float8(value),
            Self::Ascii => ascii_string_to_byte(value),
        };

        if let Some(parsed) = parsed {
            *row = parsed;
            true
        } else {
            false
        }
    }
}

#[derive(Clone, Copy)]
enum MemoryColumn {
    Address,
    Binary,
    Hex,
    UnsignedDecimal,
    SignedDecimal,
    Float,
    Ascii,
    Instruction,
}

const MEMORY_COLUMNS: [MemoryColumn; 8] = [
    MemoryColumn::Address,
    MemoryColumn::Binary,
    MemoryColumn::Hex,
    MemoryColumn::UnsignedDecimal,
    MemoryColumn::SignedDecimal,
    MemoryColumn::Float,
    MemoryColumn::Ascii,
    MemoryColumn::Instruction,
];

struct MemoryTableContext<'a> {
    instruction_text: &'a [String],
}

impl TableColumn<u8, MemoryTableContext<'_>> for MemoryColumn {
    fn header(self) -> &'static str {
        match self {
            Self::Address => "Address",
            Self::Binary => "Binary",
            Self::Hex => "Hex",
            Self::UnsignedDecimal => "Unsigned Decimal",
            Self::SignedDecimal => "Signed Decimal",
            Self::Float => "Float",
            Self::Ascii => "ASCII",
            Self::Instruction => "Instruction",
        }
    }

    fn column(self) -> Column {
        match self {
            Self::Address => Column::auto().resizable(true).at_least(20.0),
            Self::Binary => Column::auto().resizable(true).at_least(64.0),
            Self::Hex => Column::auto().resizable(true).at_least(22.0),
            Self::UnsignedDecimal | Self::SignedDecimal => {
                Column::auto_with_initial_suggestion(50.0)
                    .resizable(true)
                    .at_least(30.0)
            }
            Self::Float | Self::Ascii => Column::auto().resizable(true).at_least(30.0),
            Self::Instruction => Column::remainder()
                .resizable(true)
                .clip(true)
                .at_least(100.0),
        }
    }

    fn display(self, row_index: usize, row: &u8, context: &MemoryTableContext<'_>) -> String {
        match self {
            Self::Address => format!("{:02X}", row_index),
            Self::Binary => format!("{:08b}", row),
            Self::Hex => format!("{:02X}", row),
            Self::UnsignedDecimal => format!("{}", row),
            Self::SignedDecimal => format!("{}", *row as i8),
            Self::Float => float8_to_string(*row),
            Self::Ascii => byte_to_ascii(*row),
            Self::Instruction => context.instruction_text[row_index].clone(),
        }
    }

    fn is_editable(self) -> bool {
        matches!(
            self,
            Self::Binary
                | Self::Hex
                | Self::UnsignedDecimal
                | Self::SignedDecimal
                | Self::Float
                | Self::Ascii
        )
    }

    fn try_set(
        self,
        _row_index: usize,
        row: &mut u8,
        value: &str,
        _context: &MemoryTableContext<'_>,
    ) -> bool {
        let parsed = match self {
            Self::Address | Self::Instruction => None,
            Self::Binary => u8::from_str_radix(value, 2).ok(),
            Self::Hex => u8::from_str_radix(value, 16).ok(),
            Self::UnsignedDecimal => value.parse::<u64>().ok().map(|v| v.rem_euclid(256) as u8),
            Self::SignedDecimal => value.parse::<i8>().ok().map(|v| v as u8),
            Self::Float => string_to_float8(value),
            Self::Ascii => ascii_string_to_byte(value),
        };

        if let Some(parsed) = parsed {
            *row = parsed;
            true
        } else {
            false
        }
    }
}

impl App {
    pub(super) fn render_register_table(&mut self, ui: &mut egui::Ui) {
        Frame::group(ui.style()).show(ui, |ui: &mut egui::Ui| {
            SelectableTable::new(
                "register_table",
                &REGISTER_COLUMNS,
                self.emulator_state.get_all_registers_mut(),
                &mut self.ui_state.register_table_state,
                &(),
            )
            .row_height(14.0)
            .min_scrolled_height(80.0)
            .show(ui);
        });
    }

    pub(super) fn render_memory_table(&mut self, ui: &mut egui::Ui) {
        let layout = Layout::bottom_up(Align::TOP).with_cross_justify(true);
        ui.with_layout(layout, |ui| {
            let w = ui.available_width();
            self.render_memory_buttons(ui);
            ui.add_space(4.0);

            let scroll_to_row = self.ui_state.wants_to_jump_to_address.take().map(|val| {
                self.set_highlighted_row(val);
                usize::from(val)
            });

            let instruction_text = (0..BrookshearMachine::MEMORY_SIZE)
                .map(|index| {
                    let address = index as u8;
                    if !address.is_multiple_of(2) {
                        return String::new();
                    }

                    self.emulator_state
                        .fetch_instruction(address)
                        .ok()
                        .map(|instruction| {
                            if self.descriptive_disassembly {
                                instruction.describe()
                            } else {
                                instruction.disasm()
                            }
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>();
            let context = MemoryTableContext {
                instruction_text: &instruction_text,
            };

            Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(4, 0))
                .show(ui, |ui: &mut egui::Ui| {
                    ui.set_width(w);
                    ui.set_max_width(w);
                    ui.vertical(|ui| {
                        SelectableTable::new(
                            "memory_table",
                            &MEMORY_COLUMNS,
                            self.emulator_state.get_all_memory_mut(),
                            &mut self.ui_state.memory_table_state,
                            &context,
                        )
                        .row_height(20.0)
                        .min_scrolled_height(80.0)
                        .scroll_to_row(scroll_to_row)
                        .show(ui);
                    });
                });
        });
    }
}

fn byte_to_ascii(byte: u8) -> String {
    if byte.is_ascii_graphic() {
        format!("{}", byte as char)
    } else {
        match byte {
            0 => "NUL",
            1 => "SOH",
            2 => "STX",
            3 => "ETX",
            4 => "EOT",
            5 => "ENQ",
            6 => "ACK",
            7 => "BEL",
            8 => "BS",
            9 => "HT",
            10 => "LF",
            11 => "VT",
            12 => "FF",
            13 => "CR",
            14 => "SO",
            15 => "SI",
            16 => "DLE",
            17 => "DC1",
            18 => "DC2",
            19 => "DC3",
            20 => "DC4",
            21 => "NAK",
            22 => "SYN",
            23 => "ETB",
            24 => "CAN",
            25 => "EM",
            26 => "SUB",
            27 => "ESC",
            28 => "FS",
            29 => "GS",
            30 => "RS",
            31 => "US",
            32 => "SP",
            127 => "DEL",
            _ => "�",
        }
        .to_owned()
    }
}

fn ascii_string_to_byte(s: &str) -> Option<u8> {
    if s.len() == 1 {
        Some(s.as_bytes()[0])
    } else {
        match s {
            "NUL" => Some(0),
            "SOH" => Some(1),
            "STX" => Some(2),
            "ETX" => Some(3),
            "EOT" => Some(4),
            "ENQ" => Some(5),
            "ACK" => Some(6),
            "BEL" => Some(7),
            "BS" => Some(8),
            "HT" => Some(9),
            "LF" => Some(10),
            "VT" => Some(11),
            "FF" => Some(12),
            "CR" => Some(13),
            "SO" => Some(14),
            "SI" => Some(15),
            "DLE" => Some(16),
            "DC1" => Some(17),
            "DC2" => Some(18),
            "DC3" => Some(19),
            "DC4" => Some(20),
            "NAK" => Some(21),
            "SYN" => Some(22),
            "ETB" => Some(23),
            "CAN" => Some(24),
            "EM" => Some(25),
            "SUB" => Some(26),
            "ESC" => Some(27),
            "FS" => Some(28),
            "GS" => Some(29),
            "RS" => Some(30),
            "US" => Some(31),
            "SP" => Some(32),
            "DEL" => Some(127),
            _ => None,
        }
    }
}
