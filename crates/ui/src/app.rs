#[cfg(not(target_arch = "wasm32"))]
use std::time;
#[cfg(target_arch = "wasm32")]
use web_time as time;

use brookshear_assembly::common::Register;
use brookshear_machine::{BrookshearMachine, float8_to_string, string_to_float8};
use egui::{Align, Button, Frame, Label, Layout, RadioButton, ScrollArea, Slider, TextEdit};
use egui_extras::{Column, Size, StripBuilder};

use crate::helpers::{self, open_file};

#[derive(Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
enum EmulatorAction {
    #[default]
    Idle,
    Continue,
    Step,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PendingFileType {
    LoadMemory,
    AssembleAndLoad,
    AssembleToFile,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    message: String,
    emulator_instructions_executed: u64,
    emulator_state: BrookshearMachine,
    emulator_next_action: EmulatorAction,
    display_on: bool,
    descriptive_disassembly: bool,
    #[serde(skip)]
    register_table_state: EditableTableState,
    #[serde(skip)]
    memory_table_state: EditableTableState,
    #[serde(skip)]
    program_counter_input_buffer: String,
    highlighted_row: u8,
    instructions_per_second: f64,
    #[serde(skip)]
    last_instruction_time: Option<time::Instant>,
    #[serde(skip)]
    duration_to_account_for: std::time::Duration,
    #[serde(skip)]
    pending_file: Option<(
        PendingFileType,
        futures::channel::oneshot::Receiver<Vec<u8>>,
    )>,
    #[serde(skip)]
    last_frame_time: std::time::Duration,

    #[serde(skip)]
    wants_to_jump_to_address: Option<u8>,
    #[serde(skip)]
    jump_to_address_input_buffer: String,

    instructions_window_open: bool,
    about_window_open: bool,
    help_window_open: bool,
    assembler_help_window_open: bool,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.

        let mut res: App = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        res.update_pc_text_buffer();
        res
    }

    fn schedule_step(&mut self) {
        self.last_instruction_time = None;
        self.duration_to_account_for = time::Duration::ZERO;
        self.emulator_next_action = EmulatorAction::Step;
        self.update_pc_text_buffer();
    }

    fn schedule_unstep(&mut self) {
        todo!()
    }

    fn do_step(&mut self) -> Result<bool, brookshear_machine::BrookshearMachineError> {
        let res = self.emulator_state.step();
        if let Ok(true) = res {
            self.emulator_instructions_executed += 1;
            self.message = format!(
                "Instructions executed: {}",
                self.emulator_instructions_executed
            );
        }
        self.update_pc_text_buffer();
        self.highlighted_row = self.emulator_state.get_pc();
        res
    }

    fn cont(&mut self) {
        self.last_instruction_time = None;
        self.duration_to_account_for = time::Duration::ZERO;
        self.emulator_next_action = EmulatorAction::Continue;
        self.update_pc_text_buffer();
    }

    fn pause(&mut self) {
        self.last_instruction_time = None;
        self.duration_to_account_for = time::Duration::ZERO;
        self.emulator_next_action = EmulatorAction::Idle;
        self.update_pc_text_buffer();
    }

    fn update_pc_text_buffer(&mut self) {
        self.program_counter_input_buffer = format!("{:02X}", self.emulator_state.get_pc());
    }

    fn render_register_table(&mut self, ui: &mut egui::Ui) {
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
                            &mut self.register_table_state,
                            (i.into(), 1),
                            byte,
                            |val| format!("{:08b}", val),
                            |s| u8::from_str_radix(s, 2).ok(),
                        );
                        editable(
                            &mut row,
                            &mut self.register_table_state,
                            (i.into(), 2),
                            byte,
                            |val| format!("{:02X}", val),
                            |s| u8::from_str_radix(s, 16).ok(),
                        );
                        editable(
                            &mut row,
                            &mut self.register_table_state,
                            (i.into(), 3),
                            byte,
                            |val| format!("{}", val),
                            |s| s.parse::<u64>().ok().map(|v| v.rem_euclid(256) as u8),
                        );
                        editable(
                            &mut row,
                            &mut self.register_table_state,
                            (i.into(), 4),
                            byte,
                            |val| format!("{}", val as i8),
                            |s| s.parse::<i8>().ok().map(|v| v as u8),
                        );
                        editable(
                            &mut row,
                            &mut self.register_table_state,
                            (i.into(), 5),
                            byte,
                            float8_to_string,
                            string_to_float8,
                        );
                        editable(
                            &mut row,
                            &mut self.register_table_state,
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

    fn render_memory_buttons(&mut self, ui: &mut egui::Ui) {
        ui.allocate_ui((ui.available_width(), 50.0).into(), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            StripBuilder::new(ui)
                .size(Size::relative(0.1))
                .size(Size::remainder())
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        if ui
                            .add_sized(ui.available_size(), Button::new("Reset"))
                            .on_hover_text("Reset all memory cells to 0")
                            .clicked()
                        {
                            self.emulator_state.reset_memory();
                        }
                    });
                    strip.strip(|builder| {
                        builder
                            .size(Size::relative(0.24))
                            .size(Size::relative(0.3))
                            .size(Size::relative(0.2))
                            .size(Size::remainder())
                            .horizontal(|mut strip| {
                                strip.strip(|builder| {
                                    builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    Button::new("Save to File"),
                                                )
                                                .on_hover_text("Save the current memory to a file")
                                                .clicked()
                                            {
                                                let data = (0..BrookshearMachine::MEMORY_SIZE)
                                                    .map(|i| {
                                                        self.emulator_state.get_memory(i as u8)
                                                    })
                                                    .collect();
                                                helpers::save_file(
                                                    data,
                                                    "Untitled Memory Snapshot.bin",
                                                );
                                            }
                                        });
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    Button::new("Load from File"),
                                                )
                                                .on_hover_text(concat!(
                                                    "Load a program or memory from a ",
                                                    "file, replacing the current memory"
                                                ))
                                                .clicked()
                                            {
                                                let handle_receiver = open_file();
                                                self.pending_file = Some((
                                                    PendingFileType::LoadMemory,
                                                    handle_receiver,
                                                ));
                                            }
                                        });
                                    });
                                });
                                strip.strip(|builder| {
                                    builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    Button::new("Assemble and load"),
                                                )
                                                .on_hover_text(concat!(
                                                    "Assemble an assembly program ",
                                                    "and load it into memory"
                                                ))
                                                .clicked()
                                            {
                                                let handle_receiver = open_file();
                                                self.pending_file = Some((
                                                    PendingFileType::AssembleAndLoad,
                                                    handle_receiver,
                                                ));
                                            }
                                        });
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    Button::new("Assemble to file"),
                                                )
                                                .on_hover_text(concat!(
                                                    "Assemble an assembly program ",
                                                    "and save the resulting program to a file"
                                                ))
                                                .clicked()
                                            {
                                                let handle_receiver = open_file();
                                                self.pending_file = Some((
                                                    PendingFileType::AssembleToFile,
                                                    handle_receiver,
                                                ));
                                            }
                                        });
                                    });
                                });
                                strip.strip(|builder| {
                                    builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    Button::new("About"),
                                                )
                                                .on_hover_text(
                                                    "Show information about this application",
                                                )
                                                .clicked()
                                            {
                                                self.about_window_open = true;
                                            }
                                        });
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    TextEdit::singleline(
                                                        &mut self.jump_to_address_input_buffer,
                                                    )
                                                    .hint_text("Jump to cell..."),
                                                )
                                                .on_hover_text(concat!(
                                                    "Type an address here to jump to its ",
                                                    "corresponding row in the memory table"
                                                ))
                                                .lost_focus()
                                                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                                && let Ok(val) = u8::from_str_radix(
                                                    &self.jump_to_address_input_buffer,
                                                    16,
                                                )
                                            {
                                                self.wants_to_jump_to_address = Some(val);
                                                self.jump_to_address_input_buffer.clear();
                                            }
                                        });
                                    });
                                });
                                strip.strip(|builder| {
                                    builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    RadioButton::new(
                                                        self.descriptive_disassembly,
                                                        "descriptive",
                                                    ),
                                                )
                                                .on_hover_text(concat!(
                                                    "Show plain-English descriptions of ",
                                                    "instructions instead of an ",
                                                    "assembler-style disassembly"
                                                ))
                                                .clicked()
                                            {
                                                self.descriptive_disassembly = true;
                                            }
                                        });
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    RadioButton::new(
                                                        !self.descriptive_disassembly,
                                                        "assembler",
                                                    ),
                                                )
                                                .on_hover_text(concat!(
                                                    "Show an assembler-style disassembly ",
                                                    "instead of plain-English descriptions"
                                                ))
                                                .clicked()
                                            {
                                                self.descriptive_disassembly = false;
                                            }
                                        });
                                    });
                                });
                            });
                    });
                });
        });
    }

    fn render_register_and_display_buttons(&mut self, ui: &mut egui::Ui) {
        ui.allocate_ui((ui.available_width(), 80.0).into(), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::exact(80.0))
                .size(Size::remainder())
                .horizontal(|mut strip| {
                    strip.strip(|builder| {
                        builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                            strip.cell(|ui| {
                                egui::Frame::new()
                                    .fill(ui.visuals().faint_bg_color)
                                    .corner_radius(
                                        ui.style()
                                            .button_style(
                                                egui::widget_style::WidgetState::Active,
                                                false,
                                            )
                                            .frame
                                            .corner_radius,
                                    )
                                    .show(ui, |ui| {
                                        ui.add_sized(
                                            ui.available_size(),
                                            Label::new("Program Counter:"),
                                        );
                                    });
                            });
                            strip.cell(|ui| {
                                if ui
                                    .add_sized(ui.available_size(), Button::new("Save Image"))
                                    .on_hover_text("Save the bitmap display as a png file")
                                    .clicked()
                                {
                                    helpers::render_and_save_image(
                                        self.emulator_state.get_all_memory()[0x80..0x100]
                                            .try_into()
                                            .unwrap(),
                                        "brookshear_display.png",
                                    );
                                }
                            });
                        });
                    });
                    strip.cell(|ui| {
                        egui::Frame::new()
                            .fill(ui.visuals().extreme_bg_color)
                            .stroke(ui.visuals().widgets.inactive.bg_stroke)
                            .corner_radius(ui.visuals().widgets.inactive.corner_radius)
                            .show(ui, |ui| {
                                ui.centered_and_justified(|ui| {
                                    ui.set_max_size([20.0, 20.0].into());
                                    let response = ui.add(
                                        egui::TextEdit::singleline(
                                            &mut self.program_counter_input_buffer,
                                        )
                                        .char_limit(3)
                                        .frame(Frame::NONE)
                                        .margin(egui::Margin::ZERO)
                                        .font(egui::TextStyle::Heading),
                                    );

                                    let text_edit_id = response.id;
                                    if let Some(mut state) =
                                        egui::TextEdit::load_state(ui.ctx(), text_edit_id)
                                    {
                                        let ccursor = egui::text::CCursor::new(
                                            self.program_counter_input_buffer.chars().count(),
                                        );
                                        state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange::one(ccursor),
                                        ));
                                        state.store(ui.ctx(), text_edit_id);
                                    }

                                    self.program_counter_input_buffer = format!(
                                        "{:0>2}",
                                        &self.program_counter_input_buffer[self
                                            .program_counter_input_buffer
                                            .len()
                                            .saturating_sub(2)..]
                                    );

                                    if response.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                        && let Ok(val) = u8::from_str_radix(
                                            &self.program_counter_input_buffer,
                                            16,
                                        )
                                    {
                                        self.emulator_state.set_pc(val);
                                        self.highlighted_row = val;
                                    }
                                });
                                ui.take_available_space();
                            });
                    });
                    strip.strip(|builder| {
                        builder.sizes(Size::remainder(), 2).vertical(|mut strip| {
                            strip.cell(|ui| {
                                if ui
                                    .add_sized(ui.available_size(), Button::new("Reset"))
                                    .on_hover_text("Reset all registers to 0")
                                    .clicked()
                                {
                                    self.emulator_state.reset_registers();
                                    self.emulator_instructions_executed = 0;
                                    self.update_pc_text_buffer();
                                    self.highlighted_row = self.emulator_state.get_pc();
                                }
                            });
                            strip.cell(|ui| {
                                if ui
                                    .add_sized(
                                        ui.available_size(),
                                        Button::new(if self.display_on {
                                            "Display Off"
                                        } else {
                                            "Display On"
                                        })
                                        .selected(self.display_on),
                                    )
                                    .on_hover_text("Toggle the display on or off")
                                    .clicked()
                                {
                                    self.display_on = !self.display_on;
                                }
                            });
                        });
                    });
                });
        });
    }

    // 32x32 1-bit bitmapped display from 0x80 to 0xFF of memory
    fn render_display(&mut self, ui: &mut egui::Ui) {
        let total_size = ui.available_size();
        let cell_size = total_size / 32.0;
        let top_left_pos = ui.cursor().min;
        if self.display_on {
            for row in 0..32 {
                for col in 0..32 {
                    let address = 0x80 + row * (32 / 8) + col / 8;
                    let byte = self.emulator_state.get_memory(address);
                    let bit = byte & (1 << (7 - (col % 8)));
                    let pixel_on = bit != 0;
                    let pix_rect = egui::Rect::from_min_size(
                        top_left_pos + egui::vec2(col as f32, row as f32) * cell_size,
                        cell_size,
                    );
                    let sense = ui.allocate_rect(pix_rect, egui::Sense::click());
                    ui.painter().rect_filled(
                        pix_rect,
                        0.0,
                        if pixel_on {
                            egui::Color32::WHITE
                        } else {
                            egui::Color32::BLACK
                        },
                    );
                    if sense.clicked() {
                        if pixel_on {
                            self.emulator_state
                                .set_memory(address, byte & !(1 << (7 - (col % 8))));
                        } else {
                            self.emulator_state
                                .set_memory(address, byte | (1 << (7 - (col % 8))));
                        }
                    }
                }
            }
        } else {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(top_left_pos, total_size),
                0.0,
                ui.style().visuals.faint_bg_color,
            );
        }
    }

    fn render_memory_table(&mut self, ui: &mut egui::Ui) {
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

                            if let Some(val) = self.wants_to_jump_to_address {
                                self.wants_to_jump_to_address = None;
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
                                        &mut self.memory_table_state,
                                        (i.into(), 1),
                                        byte,
                                        |val| format!("{:08b}", val),
                                        |s| u8::from_str_radix(s, 2).ok(),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.memory_table_state,
                                        (i.into(), 2),
                                        byte,
                                        |val| format!("{:02X}", val),
                                        |s| u8::from_str_radix(s, 16).ok(),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.memory_table_state,
                                        (i.into(), 3),
                                        byte,
                                        |val| format!("{}", val),
                                        |s| s.parse::<u64>().ok().map(|v| v.rem_euclid(256) as u8),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.memory_table_state,
                                        (i.into(), 4),
                                        byte,
                                        |val| format!("{}", val as i8),
                                        |s| s.parse::<i8>().ok().map(|v| v as u8),
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.memory_table_state,
                                        (i.into(), 5),
                                        byte,
                                        float8_to_string,
                                        string_to_float8,
                                    );
                                    editable(
                                        &mut row,
                                        &mut self.memory_table_state,
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

    fn render_about_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("About")
            .open(&mut self.about_window_open)
            .resizable(false)
            .collapsible(false)
            .default_width(300.0)
            .default_height(150.0)
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    ui.heading("Brookshear Machine Emulator");
                    ui.label("Created by Ashley Hawkins");
                    ui.label("Source code available at:");
                    ui.add(
                        egui::Hyperlink::new(
                            "https://github.com/ashley-hawkins/extended-brookshear-assembler",
                        )
                        .open_in_new_tab(true),
                    );
                    powered_by_egui_and_eframe(ui);
                });
            });
    }

    fn render_instructions_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Instructions")
            .open(&mut self.instructions_window_open)
            .resizable(true)
            .collapsible(false)
            .default_width(400.0)
            .default_height(300.0)
            .show(ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    ui.heading("Extended Brookshear Machine Instructions");
                    ui.label(concat!(
                        "The Extended Brookshear Machine has 16 instructions, ",
                        "each of which is two bytes long. ",
                        "Some instructions are followed by one or two ",
                        "operand bytes, depending on the instruction. ",
                        "The instruction set is as follows:"
                    ));
                    ui.label("TODO");
                });
            });
    }

    fn render_help_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Help")
            .open(&mut self.help_window_open)
            .resizable(true)
            .collapsible(false)
            .default_width(400.0)
            .default_height(300.0)
            .show(ui.ctx(), |ui| {
                ui.label("TODO");
            });
    }

    fn render_assembler_help_window(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Assembler Help")
            .open(&mut self.assembler_help_window_open)
            .resizable(true)
            .collapsible(false)
            .default_width(400.0)
            .default_height(300.0)
            .show(ui.ctx(), |ui| {
                ui.label("TODO");
            });
    }

    fn handle_pending_file(&mut self) {
        (|| {
            if let Some((kind, handle)) = &mut self.pending_file
                && let Ok(file_result) = handle.try_recv()
            {
                let kind: PendingFileType = *kind;
                match file_result {
                    Some(file_contents) => {
                        self.pending_file = None;
                        match kind {
                            PendingFileType::AssembleAndLoad => {
                                let Ok(file_contents) = &str::from_utf8(&file_contents) else {
                                    return Err(
                                        "Failed to parse file contents as UTF-8".to_string()
                                    );
                                };
                                let program =
                                    brookshear_assembly::parser::parse_asm_file(file_contents)
                                        .map_err(|err| {
                                            format!("Failed to parse assembly: {err:?}")
                                        })?;
                                let result =
                                    brookshear_assembly::serialize::serialize_program_to_binary(
                                        &program,
                                    )
                                    .map_err(|err| {
                                        format!("Failed to process program: {err}")
                                    })?;
                                self.emulator_state.load_memory(result);
                            }
                            PendingFileType::LoadMemory => {
                                self.emulator_state.load_memory(
                                    <[u8; 256]>::try_from(&file_contents[..]).map_err(|_| {
                                        format!(
                                            "File is the wrong size. Expected 256 bytes, got: {}",
                                            file_contents.len()
                                        )
                                    })?,
                                );
                            }
                            PendingFileType::AssembleToFile => {
                                let file_contents =
                                    &str::from_utf8(&file_contents).map_err(|_| {
                                        "Failed to parse file contents as UTF-8".to_string()
                                    })?;
                                let program =
                                    brookshear_assembly::parser::parse_asm_file(file_contents)
                                        .map_err(|e| {
                                            format!("Failed to parse assembly file: {e:?}")
                                        })?;

                                let result =
                                    brookshear_assembly::serialize::serialize_program_to_binary(
                                        &program,
                                    )
                                    .map_err(|err| {
                                        format!("Failed to process program: {err}")
                                    })?;
                                helpers::save_file(result.to_vec(), "Untitled Program.bin");
                            }
                        }
                    }
                    None => { /* not received anything yet */ }
                }
            }

            Ok(())
        })().unwrap_or_else(|err| {
            eprintln!("Error handling file: {err}");
            self.pending_file = None;
            self.message = err;
        });
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let begin_frame = time::Instant::now();

        self.render_about_window(ui);
        self.render_instructions_window(ui);
        self.render_help_window(ui);
        self.render_assembler_help_window(ui);

        egui::Panel::right("right_panel")
            .resizable(false)
            .exact_size(200.0)
            .show_inside(ui, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Information");
                    if ui
                        .add_sized([ui.available_width(), 20.0], Button::new("Instructions"))
                        .clicked()
                    {
                        self.instructions_window_open = true;
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized([ui.available_width(), 20.0], Button::new("Help"))
                        .clicked()
                    {
                        self.help_window_open = true;
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized([ui.available_width(), 20.0], Button::new("Assembler help"))
                        .clicked()
                    {
                        self.assembler_help_window_open = true;
                    }
                    ui.add_space(10.0);
                    ui.label("Messages");
                });
                ui.with_layout(Layout::bottom_up(Align::TOP), |ui| {
                    // powered_by_egui_and_eframe(ui);
                    ui.label(format!(
                        "Last frame time: {:.2} ms",
                        self.last_frame_time.as_secs_f64() * 1000.0
                    ));
                    ui.label(format!(
                        "Window size: {} x {}",
                        ui.ctx().viewport_rect().width(),
                        ui.ctx().viewport_rect().height()
                    ));

                    let w = ui.available_width();
                    ui.spacing_mut().slider_width =
                        w - ui.spacing().interact_size.x - ui.spacing().button_padding.x * 2.0;
                    ui.add_sized(
                        [w, 20.0],
                        Slider::new(&mut self.instructions_per_second, 0.5..=200.0)
                            .clamping(egui::SliderClamping::Never)
                            .logarithmic(true),
                    );
                    self.instructions_per_second = self.instructions_per_second.clamp(0.5, 1000.0);
                    ui.label("Speed (instructions per second)");
                    ui.add_space(10.0);
                    if ui
                        .add_sized([w, 20.0], Button::new("Reset & Run"))
                        .clicked()
                    {
                        self.emulator_state.reset_registers();
                        self.emulator_instructions_executed = 0;
                        self.update_pc_text_buffer();
                        self.cont();
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized(
                            [w, 20.0],
                            Button::new(if self.emulator_next_action == EmulatorAction::Continue {
                                "Pause"
                            } else {
                                "Continue"
                            }),
                        )
                        .clicked()
                    {
                        self.emulator_next_action = match self.emulator_next_action {
                            EmulatorAction::Continue => EmulatorAction::Idle,
                            _ => EmulatorAction::Continue,
                        }
                    }
                    ui.add_space(10.0);
                    if ui.add_sized([w, 20.0], Button::new("Undo Step")).clicked() {
                        self.schedule_unstep()
                    }
                    ui.add_space(10.0);
                    if ui.add_sized([w, 20.0], Button::new("Step")).clicked() {
                        self.schedule_step();
                    }
                    ui.heading("CPU Controls");
                    Frame::group(ui.style()).show(ui, |ui| {
                        ui.allocate_ui_with_layout(
                            ui.available_size(),
                            Layout::top_down(Align::LEFT),
                            |ui| {
                                ui.label(&self.message);
                                ui.take_available_space();
                            },
                        );
                    });
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let h = ui.available_height();
            let half_w = ui.available_width() / 2.0;
            ui.allocate_ui_with_layout(
                (ui.available_width(), ui.available_height()).into(),
                Layout::left_to_right(Align::LEFT),
                |ui| {
                    ui.allocate_ui((half_w, h).into(), |ui| {
                        ui.push_id(0, |ui| {
                            ui.vertical(|ui| {
                                ui.heading("Memory");
                                self.render_memory_table(ui);
                            });
                        });
                    });
                    ui.push_id(1, |ui| {
                        ui.vertical(|ui| {
                            ui.heading("Registers");
                            self.render_register_table(ui);
                            self.render_register_and_display_buttons(ui);
                            self.render_display(ui);
                        });
                    });
                },
            );
        });

        let frame_time = begin_frame.elapsed();
        match self.emulator_next_action {
            EmulatorAction::Continue => {
                let this_time = time::Instant::now();
                let period = time::Duration::from_secs_f64(1.0 / self.instructions_per_second);
                let time_difference: time::Duration = self
                    .last_instruction_time
                    .map(|t| this_time - t)
                    .unwrap_or(period);
                self.last_instruction_time = Some(this_time);
                self.duration_to_account_for += time_difference;

                while self.duration_to_account_for.as_secs_f64() >= period.as_secs_f64() {
                    self.duration_to_account_for -= period;

                    match self.do_step() {
                        Ok(true) => {} // continue running
                        Ok(false) => {
                            self.pause();
                        }
                        Err(e) => {
                            eprintln!("Emulator error: {:?}", e);
                            self.pause();
                        }
                    }
                }
            }
            EmulatorAction::Step => {
                match self.do_step() {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("Emulator error: {:?}", e);
                    }
                }
                self.pause();
            }
            EmulatorAction::Idle => {}
        }

        self.handle_pending_file();

        if self.emulator_next_action != EmulatorAction::Idle {
            ui.ctx().request_repaint();
        }

        self.last_frame_time = frame_time;
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

#[derive(Default)]
struct EditableTableState {
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

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.add(
            egui::Hyperlink::from_label_and_url("egui", "https://github.com/emilk/egui")
                .open_in_new_tab(true),
        );
        ui.label(" and ");
        ui.add(
            egui::Hyperlink::from_label_and_url(
                "eframe",
                "https://github.com/emilk/egui/tree/master/crates/eframe",
            )
            .open_in_new_tab(true),
        );
        ui.label(".");
    });
}
