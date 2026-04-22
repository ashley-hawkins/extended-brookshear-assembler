use std::hash::Hash;

use brookshear_assembly::{
    errors::{parse_errors_to_string, semantic_errors_to_string},
    parser::parse_asm_file,
    serialize::serialize_inline_instruction_to_binary,
    structured_instruction::StructuredInstruction,
};

use brookshear_machine::{float8_to_string, string_to_float8};
use egui::{Align, Frame, Layout, ScrollArea};
use egui_extras::Column;

use super::{App, MaybeRichError, MessageState};

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
    should_grab_cell_focus: bool,
    should_defer_cell_focus: bool,
    editing_cell: Option<((usize, usize), String)>,
    should_grab_focus: bool,
}

impl EditableTableState {
    pub fn is_editing(&self) -> bool {
        self.editing_cell.is_some()
    }

    pub fn clear_highlight(&mut self) {
        self.highlight = TableHighlight::None;
        self.should_grab_cell_focus = false;
        self.should_defer_cell_focus = false;
    }

    pub fn set_highlighted_row(&mut self, row: usize) {
        self.highlight = TableHighlight::Row(row);
        self.should_grab_cell_focus = false;
        self.should_defer_cell_focus = false;
    }

    pub fn set_highlighted_cell(&mut self, row: usize, column: usize) {
        self.highlight = TableHighlight::Cell { row, column };
    }

    pub fn focus_highlighted_cell(&mut self) {
        self.should_grab_cell_focus = true;
    }

    pub fn defer_focus_highlighted_cell(&mut self) {
        self.should_defer_cell_focus = true;
    }
}

trait TableColumn<Row, Context>: Copy {
    fn header(self) -> &'static str;

    fn column(self) -> Column;

    fn display(self, row_index: usize, rows: &[Row], context: &Context) -> String;

    fn is_editable(self, _row_index: usize, _rows: &[Row], _context: &Context) -> bool {
        false
    }

    fn try_set(
        self,
        _row_index: usize,
        _rows: &mut [Row],
        _value: &str,
        _context: &Context,
    ) -> Result<(), MaybeRichError> {
        Err(MaybeRichError::from("This cell is not editable."))
    }
}

struct SelectableTable<'a, Row, Col, Context> {
    id: egui::Id,
    columns: &'a [Col],
    rows: &'a mut [Row],
    state: &'a mut EditableTableState,
    message_state: &'a mut MessageState,
    context: &'a Context,
    row_height: f32,
    min_scrolled_height: f32,
    scroll_to_row: Option<usize>,
}

#[derive(Clone, Copy)]
struct CellLocation {
    row: usize,
    column: usize,
    row_count: usize,
    column_count: usize,
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
        message_state: &'a mut MessageState,
        context: &'a Context,
    ) -> Self {
        Self {
            id: egui::Id::new(id_salt),
            columns,
            rows,
            state,
            message_state,
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

        if self.state.should_defer_cell_focus {
            self.state.should_defer_cell_focus = false;
            self.state.should_grab_cell_focus = true;
        }

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

            table.body(|body| {
                body.rows(self.row_height, self.rows.len(), |mut row_ui| {
                    let row_index = row_ui.index();
                    if Some(row_index) == self.state.highlight.row() {
                        row_ui.set_selected(true);
                    }

                    for (column_index, column) in self.columns.iter().copied().enumerate() {
                        render_table_cell(
                            &mut row_ui,
                            self.state,
                            self.message_state,
                            CellLocation {
                                row: row_index,
                                column: column_index,
                                row_count,
                                column_count: self.columns.len(),
                            },
                            self.rows,
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
    message_state: &mut MessageState,
    cell: CellLocation,
    rows: &mut [Row],
    column: Col,
    context: &Context,
) where
    Col: TableColumn<Row, Context>,
{
    row_ui.col(|ui| {
        let cell_coords = (cell.row, cell.column);
        let is_highlighted_cell = state.highlight.cell() == Some(cell_coords);
        let is_editable = column.is_editable(cell.row, rows, context);

        if is_editable
            && let Some((cell_being_edited, edit_str)) = &mut state.editing_cell
            && *cell_being_edited == cell_coords
        {
            let response = ui.text_edit_singleline(edit_str);
            if state.should_grab_focus {
                response.request_focus();
                state.should_grab_focus = false;
            }
            if response.lost_focus() {
                let pressed_enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                let pressed_escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                if pressed_enter {
                    match column.try_set(cell.row, rows, edit_str, context) {
                        Ok(()) => {
                            message_state.set_message(String::new());
                            state.set_highlighted_cell(
                                (cell.row + 1).min(cell.row_count.saturating_sub(1)),
                                cell.column,
                            );
                            state.focus_highlighted_cell();
                            state.editing_cell = None;
                        }
                        Err(err) => {
                            message_state.set_maybe_rich_message(err);
                            state.should_grab_focus = true;
                        }
                    }
                }
                if !state.should_grab_focus {
                    state.editing_cell = None;
                    if pressed_escape {
                        state.defer_focus_highlighted_cell();
                    }
                }
            }
            return;
        }

        ui.centered_and_justified(|ui| {
            let value = column.display(cell.row, rows, context);
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
            if is_highlighted_cell && state.should_grab_cell_focus {
                response.request_focus();
                state.should_grab_cell_focus = false;
            }
            if response.clicked() {
                state.set_highlighted_cell(cell.row, cell.column);
                response.request_focus();
            }
            if is_editable && response.double_clicked() {
                state.editing_cell = Some((cell_coords, value));
                state.should_grab_focus = true;
            }
            if is_highlighted_cell && response.has_focus() && state.editing_cell.is_none() {
                let next_cell = ui.input_mut(|i| {
                    if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                        Some((cell.row.saturating_sub(1), cell.column))
                    } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        Some((
                            (cell.row + 1).min(cell.row_count.saturating_sub(1)),
                            cell.column,
                        ))
                    } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                        Some((cell.row, cell.column.saturating_sub(1)))
                    } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                        Some((
                            cell.row,
                            (cell.column + 1).min(cell.column_count.saturating_sub(1)),
                        ))
                    } else {
                        None
                    }
                });

                if let Some((next_row, next_column)) = next_cell
                    && (next_row != cell.row || next_column != cell.column)
                {
                    state.set_highlighted_cell(next_row, next_column);
                    state.focus_highlighted_cell();
                    state.defer_focus_highlighted_cell();
                }
            }
            if is_highlighted_cell
                && is_editable
                && response.has_focus()
                && state.editing_cell.is_none()
                && let Some(text) = ui.input(|i| {
                    i.events.iter().find_map(|event| match event {
                        egui::Event::Text(text) if !text.is_empty() => Some(text.clone()),
                        _ => None,
                    })
                })
            {
                state.editing_cell = Some((cell_coords, text));
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

    fn display(self, row_index: usize, rows: &[u8], _context: &()) -> String {
        let row = rows[row_index];
        match self {
            Self::Register => format!("{:X}", row_index),
            Self::Binary => format!("{:08b}", row),
            Self::Hex => format!("{:02X}", row),
            Self::UnsignedDecimal => format!("{}", row),
            Self::SignedDecimal => format!("{}", row as i8),
            Self::Float => float8_to_string(row),
            Self::Ascii => byte_to_ascii(row),
        }
    }

    fn is_editable(self, _row_index: usize, _rows: &[u8], _context: &()) -> bool {
        !matches!(self, Self::Register)
    }

    fn try_set(
        self,
        row_index: usize,
        rows: &mut [u8],
        value: &str,
        _context: &(),
    ) -> Result<(), MaybeRichError> {
        let parsed = match self {
            Self::Register => Err(MaybeRichError::from("Register labels cannot be edited.")),
            Self::Binary => parse_binary_byte(value),
            Self::Hex => parse_hex_byte(value),
            Self::UnsignedDecimal => parse_unsigned_decimal_byte(value),
            Self::SignedDecimal => parse_signed_decimal_byte(value),
            Self::Float => parse_float8_byte(value),
            Self::Ascii => parse_ascii_byte(value),
        };

        rows[row_index] = parsed?;
        Ok(())
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

struct MemoryTableContext {
    descriptive_disassembly: bool,
}

impl TableColumn<u8, MemoryTableContext> for MemoryColumn {
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

    fn display(self, row_index: usize, rows: &[u8], context: &MemoryTableContext) -> String {
        let row = rows[row_index];
        match self {
            Self::Address => format!("{:02X}", row_index),
            Self::Binary => format!("{:08b}", row),
            Self::Hex => format!("{:02X}", row),
            Self::UnsignedDecimal => format!("{}", row),
            Self::SignedDecimal => format!("{}", row as i8),
            Self::Float => float8_to_string(row),
            Self::Ascii => byte_to_ascii(row),
            Self::Instruction => display_instruction(row_index, rows, context),
        }
    }

    fn is_editable(self, row_index: usize, rows: &[u8], context: &MemoryTableContext) -> bool {
        matches!(
            self,
            Self::Binary
                | Self::Hex
                | Self::UnsignedDecimal
                | Self::SignedDecimal
                | Self::Float
                | Self::Ascii
        ) || matches!(self, Self::Instruction)
            && !context.descriptive_disassembly
            && row_index.is_multiple_of(2)
            && row_index + 1 < rows.len()
    }

    fn try_set(
        self,
        row_index: usize,
        rows: &mut [u8],
        value: &str,
        _context: &MemoryTableContext,
    ) -> Result<(), MaybeRichError> {
        let parsed = match self {
            Self::Address => Err(MaybeRichError::from("This column cannot be edited.")),
            Self::Binary => parse_binary_byte(value),
            Self::Hex => parse_hex_byte(value),
            Self::UnsignedDecimal => parse_unsigned_decimal_byte(value),
            Self::SignedDecimal => parse_signed_decimal_byte(value),
            Self::Float => parse_float8_byte(value),
            Self::Ascii => parse_ascii_byte(value),
            Self::Instruction => {
                let [first_byte, second_byte] = parse_instruction_bytes(value)?;
                rows[row_index] = first_byte;
                rows[row_index + 1] = second_byte;
                return Ok(());
            }
        };

        rows[row_index] = parsed?;
        Ok(())
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
                &mut self.ui_state.message,
                &(),
            )
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

            let context = MemoryTableContext {
                descriptive_disassembly: self.descriptive_disassembly,
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
                            &mut self.ui_state.message,
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

fn display_instruction(row_index: usize, rows: &[u8], context: &MemoryTableContext) -> String {
    if !row_index.is_multiple_of(2) || row_index + 1 >= rows.len() {
        return String::new();
    }

    StructuredInstruction::from_bytes([rows[row_index], rows[row_index + 1]])
        .map(|instruction| {
            if context.descriptive_disassembly {
                instruction.describe()
            } else {
                instruction.disasm()
            }
        })
        .unwrap_or_default()
}

fn parse_instruction_bytes(value: &str) -> Result<[u8; 2], MaybeRichError> {
    let lines = parse_asm_file(value).map_err(|errors| {
        MaybeRichError::new(
            "Failed to parse assembly instruction. Click to see details.",
            crate::ansi::ansi_to_rich_text(&parse_errors_to_string(
                value,
                "instruction".to_owned(),
                &errors,
            )),
        )
    })?;

    if lines.len() != 1 {
        return Err(MaybeRichError::from(
            "Only one instruction should be entered.".to_owned(),
        ));
    }

    let serialized = serialize_inline_instruction_to_binary(&lines[0]).map_err(|err| {
        MaybeRichError::new(
            "Failed to assemble instruction. Click to see details.",
            crate::ansi::ansi_to_rich_text(&semantic_errors_to_string(
                value,
                "instruction".to_owned(),
                &[err],
            )),
        )
    })?;

    Ok(serialized)
}

fn parse_binary_byte(value: &str) -> Result<u8, MaybeRichError> {
    u8::from_str_radix(value, 2)
        .map_err(|err| MaybeRichError::from(format!("Invalid binary byte '{value}': {err}")))
}

fn parse_hex_byte(value: &str) -> Result<u8, MaybeRichError> {
    u8::from_str_radix(value, 16)
        .map_err(|err| MaybeRichError::from(format!("Invalid hexadecimal byte '{value}': {err}")))
}

fn parse_unsigned_decimal_byte(value: &str) -> Result<u8, MaybeRichError> {
    value
        .parse::<u64>()
        .map(|v| v.rem_euclid(256) as u8)
        .map_err(|err| {
            MaybeRichError::from(format!("Invalid unsigned decimal byte '{value}': {err}"))
        })
}

fn parse_signed_decimal_byte(value: &str) -> Result<u8, MaybeRichError> {
    value.parse::<i8>().map(|v| v as u8).map_err(|err| {
        MaybeRichError::from(format!("Invalid signed decimal byte '{value}': {err}"))
    })
}

fn parse_float8_byte(value: &str) -> Result<u8, MaybeRichError> {
    string_to_float8(value)
        .map_err(|err| MaybeRichError::from(format!("Invalid float8 value '{value}': {err}")))
}

fn parse_ascii_byte(value: &str) -> Result<u8, MaybeRichError> {
    ascii_string_to_byte(value).map_err(MaybeRichError::from)
}

fn ascii_string_to_byte(s: &str) -> Result<u8, String> {
    if s.chars().count() == 1 {
        let character = s
            .chars()
            .next()
            .expect("single-char strings have a first char");
        if character.is_ascii() {
            Ok(character as u8)
        } else {
            Err(format!("'{s}' is not an ASCII character."))
        }
    } else if s.is_empty() {
        Err("Enter a single ASCII character or control-code name.".to_owned())
    } else {
        match s {
            "NUL" => Ok(0),
            "SOH" => Ok(1),
            "STX" => Ok(2),
            "ETX" => Ok(3),
            "EOT" => Ok(4),
            "ENQ" => Ok(5),
            "ACK" => Ok(6),
            "BEL" => Ok(7),
            "BS" => Ok(8),
            "HT" => Ok(9),
            "LF" => Ok(10),
            "VT" => Ok(11),
            "FF" => Ok(12),
            "CR" => Ok(13),
            "SO" => Ok(14),
            "SI" => Ok(15),
            "DLE" => Ok(16),
            "DC1" => Ok(17),
            "DC2" => Ok(18),
            "DC3" => Ok(19),
            "DC4" => Ok(20),
            "NAK" => Ok(21),
            "SYN" => Ok(22),
            "ETB" => Ok(23),
            "CAN" => Ok(24),
            "EM" => Ok(25),
            "SUB" => Ok(26),
            "ESC" => Ok(27),
            "FS" => Ok(28),
            "GS" => Ok(29),
            "RS" => Ok(30),
            "US" => Ok(31),
            "SP" => Ok(32),
            "DEL" => Ok(127),
            _ => Err(format!(
                "'{s}' is not a valid ASCII control-code name. Use a single ASCII character or one of NUL, SOH, STX, ETX, EOT, ENQ, ACK, BEL, BS, HT, LF, VT, FF, CR, SO, SI, DLE, DC1, DC2, DC3, DC4, NAK, SYN, ETB, CAN, EM, SUB, ESC, FS, GS, RS, US, SP, DEL."
            )),
        }
    }
}
