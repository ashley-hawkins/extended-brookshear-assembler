use brookshear_assembly::common::Register;
use brookshear_machine::{BrookshearMachine, float8_to_string, string_to_float8};
use egui::{Align, Frame, Layout, ScrollArea};
use egui_extras::Column;

use super::App;

#[derive(Default)]
pub struct EditableTableState {
    editing_cell: Option<((usize, usize), String)>,
    should_grab_focus: bool,
}

fn editable(
    row: &mut egui_extras::TableRow<'_, '_>,
    state: &mut EditableTableState,
    cell: (usize, usize),
    value: &mut u8,
    to_string: impl Fn(u8) -> String,
    from_string: impl Fn(&str) -> Option<u8>,
) {
    row.col(|ui| {
        if let Some((cell_being_edited, edit_str)) = &mut state.editing_cell
            && &cell == cell_being_edited
        {
            let response = ui.text_edit_singleline(edit_str);
            if state.should_grab_focus {
                response.request_focus();
                state.should_grab_focus = false;
            }
            if response.lost_focus() {
                if ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && let Some(new_value) = from_string(edit_str)
                {
                    *value = new_value;
                }
                state.editing_cell = None;
            }
        } else {
            ui.centered_and_justified(|ui| {
                if ui.label(to_string(*value)).double_clicked() {
                    state.editing_cell = Some((cell, to_string(*value)));
                    state.should_grab_focus = true;
                }
            });
        }
    });
}

impl App {
    pub(super) fn render_register_table(&mut self, ui: &mut egui::Ui) {
        ScrollArea::horizontal().show(ui, |ui| {
            Frame::group(ui.style()).show(ui, |ui: &mut egui::Ui| {
                let table_builder = egui_extras::TableBuilder::new(ui)
                    .min_scrolled_height(80.0)
                    .striped(true)
                    .column(Column::auto().resizable(true).at_least(20.0))
                    .column(Column::auto().resizable(true).at_least(64.0))
                    .column(Column::auto().resizable(true).at_least(22.0))
                    .columns(
                        Column::auto_with_initial_suggestion(50.0)
                            .resizable(true)
                            .at_least(30.0),
                        2,
                    )
                    .column(Column::auto().resizable(true).at_least(30.0))
                    .column(
                        Column::remainder()
                            .resizable(true)
                            .clip(true)
                            .at_least(30.0),
                    );

                let mut table = table_builder.header(20.0, |mut row| {
                    for header in [
                        "Register",
                        "Binary",
                        "Hex",
                        "Unsigned Decimal",
                        "Signed Decimal",
                        "Float",
                        "ASCII",
                    ] {
                        row.col(|ui| {
                            ui.centered_and_justified(|ui| {
                                ui.add(
                                    egui::Label::new(header)
                                        .truncate()
                                        .show_tooltip_when_elided(true),
                                );
                            });
                        });
                    }
                });

                table.ui_mut().style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                table.body(|body| {
                    body.rows(14.0, BrookshearMachine::REGISTER_COUNT, |mut row| {
                        let i = row.index() as u8;
                        let byte = self
                            .emulator_state
                            .get_register_mut(Register::from_repr(i).unwrap());
                        row.col(|ui| {
                            ui.centered_and_justified(|ui| {
                                ui.label(format!("{:X}", i));
                            });
                        });
                        editable(
                            &mut row,
                            &mut self.ui_state.register_table_state,
                            (i.into(), 1),
                            byte,
                            |val| format!("{:08b}", val),
                            |s| u8::from_str_radix(s, 2).ok(),
                        );
                        editable(
                            &mut row,
                            &mut self.ui_state.register_table_state,
                            (i.into(), 2),
                            byte,
                            |val| format!("{:02X}", val),
                            |s| u8::from_str_radix(s, 16).ok(),
                        );
                        editable(
                            &mut row,
                            &mut self.ui_state.register_table_state,
                            (i.into(), 3),
                            byte,
                            |val| format!("{}", val),
                            |s| s.parse::<u64>().ok().map(|v| v.rem_euclid(256) as u8),
                        );
                        editable(
                            &mut row,
                            &mut self.ui_state.register_table_state,
                            (i.into(), 4),
                            byte,
                            |val| format!("{}", val as i8),
                            |s| s.parse::<i8>().ok().map(|v| v as u8),
                        );
                        editable(
                            &mut row,
                            &mut self.ui_state.register_table_state,
                            (i.into(), 5),
                            byte,
                            float8_to_string,
                            string_to_float8,
                        );
                        editable(
                            &mut row,
                            &mut self.ui_state.register_table_state,
                            (i.into(), 6),
                            byte,
                            byte_to_ascii,
                            ascii_string_to_byte,
                        );
                    });
                });
            });
        });
    }

    pub(super) fn render_memory_table(&mut self, ui: &mut egui::Ui) {
        let layout = Layout::bottom_up(Align::TOP).with_cross_justify(true);
        ui.with_layout(layout, |ui| {
            let w = ui.available_width();
            self.render_memory_buttons(ui);
            ui.add_space(4.0);

            Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(4, 0))
                .show(ui, |ui: &mut egui::Ui| {
                    ui.set_width(w);
                    ui.set_max_width(w);
                    ui.vertical(|ui| {
                        ScrollArea::horizontal().show(ui, |ui| {
                            let mut table_builder = egui_extras::TableBuilder::new(ui)
                                .min_scrolled_height(80.0)
                                .striped(true)
                                .column(Column::auto().resizable(true).at_least(20.0))
                                .column(Column::auto().resizable(true).at_least(64.0))
                                .column(Column::auto().resizable(true).at_least(22.0))
                                .columns(
                                    Column::auto_with_initial_suggestion(50.0)
                                        .resizable(true)
                                        .at_least(30.0),
                                    2,
                                )
                                .column(Column::auto().resizable(true).at_least(30.0))
                                .column(Column::auto().resizable(true).at_least(30.0))
                                .column(
                                    Column::remainder()
                                        .resizable(true)
                                        .clip(true)
                                        .at_least(100.0),
                                );

                            if let Some(val) = self.ui_state.wants_to_jump_to_address {
                                self.ui_state.wants_to_jump_to_address = None;
                                self.highlighted_row = val;
                                table_builder = table_builder.scroll_to_row(val.into(), None);
                            }

                            let mut table = table_builder.header(20.0, |mut row| {
                                for header in [
                                    "Address",
                                    "Binary",
                                    "Hex",
                                    "Unsigned Decimal",
                                    "Signed Decimal",
                                    "Float",
                                    "ASCII",
                                    "Instruction",
                                ] {
                                    row.col(|ui| {
                                        ui.centered_and_justified(|ui| {
                                            ui.add(
                                                egui::Label::new(header)
                                                    .truncate()
                                                    .show_tooltip_when_elided(true),
                                            );
                                        });
                                    });
                                }
                            });

                            table.ui_mut().style_mut().wrap_mode =
                                Some(egui::TextWrapMode::Truncate);

                            table.body(|body| {
                                body.rows(20.0, BrookshearMachine::MEMORY_SIZE, |mut row| {
                                    if row.index() as u8 == self.highlighted_row {
                                        row.set_selected(true);
                                    }
                                    let i = row.index() as u8;

                                    let byte = self.emulator_state.get_memory_mut(i);
                                    row.col(|ui| {
                                        ui.centered_and_justified(|ui| {
                                            ui.add(egui::Label::new(format!("{:02X}", i)));
                                        });
                                    });
                                    editable(
                                        &mut row,
                                        &mut self.ui_state.memory_table_state,
                                        (i.into(), 1),
                                        byte,
                                        |val| format!("{:08b}", val),
                                        |s| u8::from_str_radix(s, 2).ok(),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.ui_state.memory_table_state,
                                        (i.into(), 2),
                                        byte,
                                        |val| format!("{:02X}", val),
                                        |s| u8::from_str_radix(s, 16).ok(),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.ui_state.memory_table_state,
                                        (i.into(), 3),
                                        byte,
                                        |val| format!("{}", val),
                                        |s| s.parse::<u64>().ok().map(|v| v.rem_euclid(256) as u8),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.ui_state.memory_table_state,
                                        (i.into(), 4),
                                        byte,
                                        |val| format!("{}", val as i8),
                                        |s| s.parse::<i8>().ok().map(|v| v as u8),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.ui_state.memory_table_state,
                                        (i.into(), 5),
                                        byte,
                                        float8_to_string,
                                        string_to_float8,
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.ui_state.memory_table_state,
                                        (i.into(), 6),
                                        byte,
                                        byte_to_ascii,
                                        ascii_string_to_byte,
                                    );
                                    row.col(|ui| {
                                        ui.centered_and_justified(|ui| {
                                            if i.is_multiple_of(2) {
                                                self.emulator_state
                                                    .fetch_instruction(i)
                                                    .inspect(|instr| {
                                                        ui.label(if self.descriptive_disassembly {
                                                            instr.describe()
                                                        } else {
                                                            instr.disasm()
                                                        });
                                                    })
                                                    .ok();
                                            }
                                        });
                                    });
                                });
                            });
                        });
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
            _ => "�", // Non-ASCII or non-printable
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
