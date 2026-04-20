mod tables;
mod windows;

#[cfg(not(target_arch = "wasm32"))]
use std::time;
#[cfg(target_arch = "wasm32")]
use web_time as time;

use brookshear_assembly::errors::{parse_errors_to_string, semantic_errors_to_string};
use brookshear_machine::BrookshearMachine;
use egui::{Align, Button, FontData, FontDefinitions, FontFamily, Frame, Label, Layout, RadioButton, Slider, TextEdit};
use egui_extras::{Size, StripBuilder};

use crate::{
    ansi::MyRichText,
    app::{
        tables::EditableTableState,
        windows::{AppWindows, HelpPage, WindowOpenId},
    },
    helpers::{self, DisplayImageReceiver, FileReceiver, open_file},
};

#[derive(Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
enum EmulatorAction {
    #[default]
    Idle,
    Continue,
    Step,
    Unstep,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PendingFileType {
    LoadMemory,
    AssembleAndLoad,
    AssembleToFile,
    LoadDisplayImage,
}

#[derive(Default)]
struct MessageState {
    text: String,
    rich_text: Option<MyRichText>,
}

impl MessageState {
    fn set_message(&mut self, message: impl Into<String>) {
        self.text = message.into();
        self.rich_text = None;
    }

    fn set_maybe_rich_message(&mut self, error: MaybeRichError) {
        self.text = error.message;
        self.rich_text = error.rich_text;
    }
}

pub(super) struct MaybeRichError {
    message: String,
    rich_text: Option<MyRichText>,
}

impl MaybeRichError {
    pub(super) fn new(message: impl Into<String>, rich_text: MyRichText) -> Self {
        Self {
            message: message.into(),
            rich_text: Some(rich_text),
        }
    }
}

impl From<String> for MaybeRichError {
    fn from(message: String) -> Self {
        Self {
            message,
            rich_text: None,
        }
    }
}

impl From<&str> for MaybeRichError {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

type PendingFile = (PendingFileType, FileReceiver);

type MaybePendingFile = Option<PendingFile>;
type MaybePendingDisplayImage = Option<DisplayImageReceiver>;

#[derive(Default)]
struct AppUiState {
    message: MessageState,
    register_table_state: EditableTableState,
    memory_table_state: EditableTableState,
    program_counter_input_buffer: String,
    last_instruction_time: Option<time::Instant>,
    duration_to_account_for: std::time::Duration,
    pending_file: MaybePendingFile,
    pending_display_image: MaybePendingDisplayImage,
    last_frame_time: std::time::Duration,
    wants_to_jump_to_address: Option<u8>,
    jump_to_address_input_buffer: String,
    pasted_program_buffer: String,
    show_paste_dialog: bool,
}

struct InfoPanel;

impl InfoPanel {
    fn show(app: &mut App, ui: &mut egui::Ui) {
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
                        app.windows.open(WindowOpenId::Instructions);
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized([ui.available_width(), 20.0], Button::new("Help"))
                        .clicked()
                    {
                        app.windows
                            .open(WindowOpenId::Help(Some(HelpPage::General)));
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized([ui.available_width(), 20.0], Button::new("Assembler help"))
                        .clicked()
                    {
                        app.windows
                            .open(WindowOpenId::Help(Some(HelpPage::Assembler)));
                    }
                    ui.add_space(10.0);
                    ui.label("Messages");
                });
                ui.with_layout(Layout::bottom_up(Align::TOP), |ui| {
                    ui.label(format!(
                        "Last frame time: {:.2} ms",
                        app.ui_state.last_frame_time.as_secs_f64() * 1000.0
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
                        Slider::new(&mut app.instructions_per_second, 0.5..=200.0)
                            .clamping(egui::SliderClamping::Never)
                            .logarithmic(true),
                    );
                    app.instructions_per_second = app.instructions_per_second.clamp(0.5, 1000.0);
                    ui.label("Speed (instructions per second)");
                    ui.add_space(10.0);
                    if ui
                        .add_sized([w, 20.0], Button::new("Reset & Run"))
                        .on_hover_text("Reset registers and start running")
                        .clicked()
                    {
                        app.emulator_state.reset_registers();
                        app.emulator_instructions_executed = 0;
                        app.update_pc_text_buffer();
                        app.cont();
                    }
                    ui.add_space(10.0);
                    if ui
                        .add_sized(
                            [w, 20.0],
                            Button::new(if app.emulator_next_action == EmulatorAction::Continue {
                                "Pause"
                            } else {
                                "Continue"
                            }),
                        )
                        .on_hover_text("Pause/resume execution")
                        .clicked()
                    {
                        match app.emulator_next_action {
                            EmulatorAction::Continue => app.pause(),
                            _ => app.cont(),
                        }
                    }
                    ui.add_space(10.0);
                    if ui.add_sized([w, 20.0], Button::new("Undo Step")).clicked() {
                        app.schedule_unstep();
                    }

                    ui.add_space(10.0);
                    if ui
                        .add_sized([w, 20.0], Button::new("Step"))
                        .on_hover_text("Execute one step at a time")
                        .clicked()
                    {
                        app.schedule_step();
                    }
                    ui.heading("CPU Controls");
                    let frame_rect = Frame::group(ui.style())
                        .show(ui, |ui| {
                            ui.allocate_ui_with_layout(
                                ui.available_size(),
                                Layout::top_down(Align::LEFT),
                                |ui| {
                                    ui.label(&app.ui_state.message.text);
                                    ui.take_available_space();
                                },
                            )
                        })
                        .inner
                        .response
                        .rect;
                    if ui
                        .interact(
                            frame_rect,
                            "msgbox".into(),
                            egui::Sense::click() | egui::Sense::hover(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        app.windows.open(WindowOpenId::MessageDetails);
                    };
                });
            });
    }
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize, Default)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    emulator_instructions_executed: u64,
    emulator_state: BrookshearMachine,
    emulator_next_action: EmulatorAction,
    display_on: bool,
    descriptive_disassembly: bool,
    instructions_per_second: f64,
    windows: AppWindows,
    #[serde(skip)]
    ui_state: AppUiState,
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.

        let mut res: App = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        };

        res.windows.initialize();

        // history entries are only 4 bytes so this is 4KB max
        res.emulator_state.set_history_limit(1000);
        res.update_pc_text_buffer();
        res
    }

    fn schedule_step(&mut self) {
        self.ui_state.last_instruction_time = None;
        self.ui_state.duration_to_account_for = time::Duration::ZERO;
        self.emulator_next_action = EmulatorAction::Step;
        self.update_pc_text_buffer();
    }

    fn schedule_unstep(&mut self) {
        self.emulator_next_action = EmulatorAction::Unstep;
    }

    fn do_step(&mut self) -> Result<bool, brookshear_machine::BrookshearMachineError> {
        let res = self.emulator_state.step();
        if let Ok(true) = res {
            self.emulator_instructions_executed += 1;
            self.set_message(format!(
                "Instructions executed: {}",
                self.emulator_instructions_executed
            ));
        }
        self.update_pc_text_buffer();
        self.set_highlighted_row(self.emulator_state.get_pc());
        res
    }

    fn undo_step(&mut self) {
        if self.emulator_state.undo_step() {
            self.emulator_instructions_executed -= 1;
            self.set_message(format!(
                "Instructions executed: {}",
                self.emulator_instructions_executed
            ));
        } else {
            self.set_message("No more steps to undo.");
        }
        self.update_pc_text_buffer();
        self.set_highlighted_row(self.emulator_state.get_pc());
    }

    fn cont(&mut self) {
        self.ui_state.last_instruction_time = None;
        self.ui_state.duration_to_account_for = time::Duration::ZERO;
        self.emulator_next_action = EmulatorAction::Continue;
        self.update_pc_text_buffer();
    }

    fn pause(&mut self) {
        self.ui_state.last_instruction_time = None;
        self.ui_state.duration_to_account_for = time::Duration::ZERO;
        self.emulator_next_action = EmulatorAction::Idle;
        self.update_pc_text_buffer();
    }

    fn set_message(&mut self, message: impl Into<String>) {
        self.ui_state.message.set_message(message);
    }

    fn set_maybe_rich_message(&mut self, error: MaybeRichError) {
        self.ui_state.message.set_maybe_rich_message(error);
    }

    fn update_pc_text_buffer(&mut self) {
        self.ui_state.program_counter_input_buffer =
            format!("{:02X}", self.emulator_state.get_pc());
    }

    fn normalize_program_counter_input(&mut self) {
        let mut filtered = self
            .ui_state
            .program_counter_input_buffer
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect::<String>();

        if filtered.len() > 2 {
            filtered = filtered[filtered.len().saturating_sub(2)..].to_owned();
        }

        self.ui_state.program_counter_input_buffer = format!("{:0>2}", filtered);
    }

    fn any_inline_editor_active(&self) -> bool {
        self.ui_state.register_table_state.is_editing()
            || self.ui_state.memory_table_state.is_editing()
    }

    fn handle_global_paste(&mut self, ctx: &egui::Context) {
        if self.ui_state.show_paste_dialog
            || self.any_inline_editor_active()
            || ctx.egui_wants_keyboard_input()
        {
            return;
        }

        let pasted_text = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Paste(text) if !text.trim().is_empty() => Some(text.clone()),
                _ => None,
            })
        });

        if let Some(text) = pasted_text {
            self.ui_state.pasted_program_buffer = text;
            self.ui_state.show_paste_dialog = true;
        }
    }

    fn assemble_and_load_text(
        &mut self,
        file_contents: &str,
        file_name: impl Into<String>,
    ) -> Result<(), MaybeRichError> {
        let file_name = file_name.into();
        let program = brookshear_assembly::parser::parse_asm_file(file_contents).map_err(|e| {
            MaybeRichError::new(
                "Failed to parse assembly, click to see details.",
                crate::ansi::ansi_to_rich_text(&parse_errors_to_string(
                    file_contents,
                    file_name.clone(),
                    &e,
                )),
            )
        })?;
        let result = brookshear_assembly::serialize::serialize_program_to_binary(&program)
            .map_err(|err| {
                MaybeRichError::new(
                    "Failed to assemble program. Click to see details.",
                    crate::ansi::ansi_to_rich_text(&semantic_errors_to_string(
                        file_contents,
                        file_name,
                        &[err],
                    )),
                )
            })?;
        self.emulator_state.load_memory(result);
        self.set_message("Successfully loaded program.");
        Ok(())
    }

    fn render_paste_dialog(&mut self, ctx: &egui::Context) {
        if !self.ui_state.show_paste_dialog {
            return;
        }

        let mut open = self.ui_state.show_paste_dialog;
        let mut should_close = false;

        egui::Window::new("Pasted Text")
            .open(&mut open)
            .resizable(true)
            .collapsible(false)
            .default_width(500.0)
            .default_height(320.0)
            .show(ctx, |ui| {
                ui.label("Paste detected. Would you like to assemble and load this text?");
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("pasted_program_scroll")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        ui.add(
                            TextEdit::multiline(&mut self.ui_state.pasted_program_buffer)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12)
                                .code_editor(),
                        );
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Assemble and Load").clicked() {
                        let pasted_program = self.ui_state.pasted_program_buffer.clone();
                        match self.assemble_and_load_text(&pasted_program, "Pasted Program") {
                            Ok(()) => {
                                self.ui_state.pasted_program_buffer.clear();
                                should_close = true;
                            }
                            Err(err) => self.set_maybe_rich_message(err),
                        }
                    }

                    if ui.button("Cancel").clicked() {
                        self.ui_state.pasted_program_buffer.clear();
                        should_close = true;
                    }
                });
            });

        self.ui_state.show_paste_dialog = open && !should_close;
        if should_close {
            self.ui_state.show_paste_dialog = false;
        }
    }

    fn set_highlighted_row(&mut self, row: u8) {
        self.ui_state
            .memory_table_state
            .set_highlighted_row(usize::from(row));
        self.ui_state.register_table_state.clear_highlight();
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
                                                if let Err(err) = helpers::save_file(
                                                    data,
                                                    "Untitled Memory Snapshot.bin",
                                                ) {
                                                    self.set_maybe_rich_message(err.into());
                                                }
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
                                                self.ui_state.pending_file = Some((
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
                                                self.ui_state.pending_file = Some((
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
                                                self.ui_state.pending_file = Some((
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
                                                self.windows.open(WindowOpenId::About);
                                            }
                                        });
                                        strip.cell(|ui| {
                                            if ui
                                                .add_sized(
                                                    ui.available_size(),
                                                    TextEdit::singleline(
                                                        &mut self
                                                            .ui_state
                                                            .jump_to_address_input_buffer,
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
                                                    &self.ui_state.jump_to_address_input_buffer,
                                                    16,
                                                )
                                            {
                                                self.ui_state.wants_to_jump_to_address = Some(val);
                                                self.ui_state.jump_to_address_input_buffer.clear();
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
                        builder.sizes(Size::remainder(), 3).vertical(|mut strip| {
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
                                    if let Err(err) = helpers::render_and_save_image(
                                        self.emulator_state.get_all_memory()[0x80..0x100]
                                            .try_into()
                                            .unwrap(),
                                        "brookshear_display.png",
                                    ) {
                                        self.set_maybe_rich_message(err.into());
                                    }
                                }
                            });
                            strip.cell(|ui| {
                                if ui
                                    .add_sized(ui.available_size(), Button::new("Load Image"))
                                    .on_hover_text(
                                        "Load a 32x32 two-color PNG into the display memory region",
                                    )
                                    .clicked()
                                {
                                    let handle_receiver = open_file();
                                    self.ui_state.pending_file = Some((
                                        PendingFileType::LoadDisplayImage,
                                        handle_receiver,
                                    ));
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
                                            &mut self.ui_state.program_counter_input_buffer,
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
                                            self.ui_state
                                                .program_counter_input_buffer
                                                .chars()
                                                .count(),
                                        );
                                        state.cursor.set_char_range(Some(
                                            egui::text::CCursorRange::one(ccursor),
                                        ));
                                        state.store(ui.ctx(), text_edit_id);
                                    }

                                    self.normalize_program_counter_input();

                                    if response.lost_focus()
                                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                        && let Ok(val) = u8::from_str_radix(
                                            &self.ui_state.program_counter_input_buffer,
                                            16,
                                        )
                                    {
                                        self.emulator_state.set_pc(val);
                                        self.set_highlighted_row(val);
                                    }
                                });
                                ui.take_available_space();
                            });
                    });
                    strip.strip(|builder| {
                        builder.sizes(Size::remainder(), 3).vertical(|mut strip| {
                            strip.cell(|ui| {
                                if ui
                                    .add_sized(ui.available_size(), Button::new("Reset"))
                                    .on_hover_text("Reset all registers to 0")
                                    .clicked()
                                {
                                    self.emulator_state.reset_registers();
                                    self.emulator_instructions_executed = 0;
                                    self.update_pc_text_buffer();
                                    self.set_highlighted_row(self.emulator_state.get_pc());
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
                            strip.cell(|ui| {
                                let response = ui
                                    .add_sized(ui.available_size(), Button::new("Clear Display"))
                                    .on_hover_text(
                                        "Left-click to clear the display to black. Right-click to fill the display white.",
                                    );

                                if response.clicked() {
                                    self.emulator_state.get_all_memory_mut()[0x80..0x100].fill(0);
                                } else if response.secondary_clicked() {
                                    self.emulator_state.get_all_memory_mut()[0x80..0x100]
                                        .fill(0xFF);
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

    fn handle_pending_file(&mut self) {
        (|| {
            if let Some((kind, handle)) = &mut self.ui_state.pending_file
                && let Ok(file_result) = handle.try_recv()
            {
                let kind: PendingFileType = *kind;
                match file_result {
                    Some(Ok((file_name, file_contents))) => {
                        self.ui_state.pending_file = None;
                        match kind {
                            PendingFileType::AssembleAndLoad => {
                                let Ok(file_contents) = &str::from_utf8(&file_contents) else {
                                    return Err(MaybeRichError::from(
                                        "Failed to parse file contents as UTF-8",
                                    ));
                                };
                                self.assemble_and_load_text(file_contents, file_name)?;
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
                                self.set_message("Successfully loaded memory.");
                            }
                            PendingFileType::AssembleToFile => {
                                let file_contents =
                                    &str::from_utf8(&file_contents).map_err(|_| {
                                        "File does not contain valid UTF-8. Only UTF-8 encoded files are supported.".to_string()
                                    })?;
                                let program = brookshear_assembly::parser::parse_asm_file(file_contents)
                                    .map_err(|e| {
                                        MaybeRichError::new(
                                            "Failed to parse assembly, click to see details.",
                                            crate::ansi::ansi_to_rich_text(&parse_errors_to_string(
                                                file_contents,
                                                file_name.to_string(),
                                                &e,
                                            )),
                                        )
                                    })?;
                                let result =
                                    brookshear_assembly::serialize::serialize_program_to_binary(
                                        &program,
                                    )
                                    .map_err(|err| {
                                        MaybeRichError::new(
                                            "Failed to assemble program. Click to see details.",
                                            crate::ansi::ansi_to_rich_text(
                                                &semantic_errors_to_string(
                                                    file_contents,
                                                    file_name.to_string(),
                                                    &[err],
                                                ),
                                            ),
                                        )
                                    })?;
                                helpers::save_file(result.to_vec(), "Untitled Program.bin")
                                    .map_err(MaybeRichError::from)?;
                                self.set_message("Successfully assembled program.");
                            }
                            PendingFileType::LoadDisplayImage => {
                                self.ui_state.pending_display_image =
                                    Some(helpers::decode_display_image_async(file_contents));
                            }
                        }
                    }
                    Some(Err(err)) => {
                        self.ui_state.pending_file = None;
                        return Err(MaybeRichError::from(err));
                    }
                    None => { /* not received anything yet */ }
                }
            }

            if let Some(handle) = &mut self.ui_state.pending_display_image
                && let Ok(image_result) = handle.try_recv()
            {
                match image_result {
                    Some(Ok(display)) => {
                        self.ui_state.pending_display_image = None;
                        self.emulator_state.get_all_memory_mut()[0x80..0x100]
                            .copy_from_slice(&display);
                        self.set_message("Successfully loaded display image.");
                    }
                    Some(Err(err)) => {
                        self.ui_state.pending_display_image = None;
                        return Err(MaybeRichError::from(err));
                    }
                    None => { /* not received anything yet */ }
                }
            }

            Ok(())
        })()
        .unwrap_or_else(|err| {
            eprintln!("Error handling file: {}", err.message);
            self.ui_state.pending_file = None;
            self.set_maybe_rich_message(err);
        });
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::empty();
    fonts.font_data.insert(
        "ui".into(),
        FontData::from_static(include_bytes!("fonts/UbuntuLightSubset.ttf")).into(),
    );
    fonts.font_data.insert(
        "mono".into(),
        FontData::from_static(include_bytes!("fonts/HackRegularSubset.ttf")).into(),
    );

    fonts
        .families
        .insert(FontFamily::Proportional, vec!["ui".into(), "mono".into()]);
    fonts
        .families
        .insert(FontFamily::Monospace, vec!["mono".into(), "ui".into()]);

    ctx.set_fonts(fonts);
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let begin_frame = time::Instant::now();

        self.handle_global_paste(ui.ctx());

        self.windows
            .show(ui.ctx(), self.ui_state.message.rich_text.as_ref());
        self.render_paste_dialog(ui.ctx());

        InfoPanel::show(self, ui);
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
                    .ui_state
                    .last_instruction_time
                    .map(|t| this_time - t)
                    .unwrap_or(period);
                self.ui_state.last_instruction_time = Some(this_time);
                self.ui_state.duration_to_account_for += time_difference;

                while self.ui_state.duration_to_account_for.as_secs_f64() >= period.as_secs_f64() {
                    self.ui_state.duration_to_account_for -= period;

                    match self.do_step() {
                        Ok(true) => {} // continue running
                        Ok(false) => {
                            self.pause();
                        }
                        Err(e) => {
                            self.set_message(format!("Emulator error: {}", e));
                            self.pause();
                        }
                    }
                }
            }
            EmulatorAction::Step => {
                match self.do_step() {
                    Ok(_) => {}
                    Err(e) => {
                        self.set_message(format!("Emulator error: {}", e));
                    }
                }
                self.pause();
            }
            EmulatorAction::Unstep => {
                self.undo_step();
                self.pause();
            }
            EmulatorAction::Idle => {}
        }

        self.handle_pending_file();

        if self.emulator_next_action != EmulatorAction::Idle {
            ui.ctx().request_repaint();
        }

        self.ui_state.last_frame_time = frame_time;
    }
}
