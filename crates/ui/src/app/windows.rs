use egui::{Frame, ScrollArea};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::ansi::MyRichText;

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub enum HelpPage {
    #[default]
    General,
    Assembler,
}

pub enum WindowOpenId {
    Instructions,
    About,
    Help(Option<HelpPage>),
    MessageDetails,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct AboutWindow {
    open: bool,
}

impl AboutWindow {
    fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new("About")
            .open(&mut self.open)
            .resizable(false)
            .collapsible(false)
            .default_width(300.0)
            .default_height(150.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Brookshear Machine Emulator");
                    ui.label("Created by Ashley Hawkins");
                    ui.label("UI layout and extended instructions are based on JBrookshearMachine by Milan Gritta");
                    ui.horizontal(|ui| {
                        ui.label("Source code available at:");
                        ui.add(
                            egui::Hyperlink::new(
                                "https://github.com/ashley-hawkins/extended-brookshear-assembler",
                            )
                            .open_in_new_tab(true),
                        );
                    });
                    powered_by_egui_and_eframe(ui);
                });
            });
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct InstructionsWindow {
    open: bool,
}

impl InstructionsWindow {
    fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new("Instructions")
            .open(&mut self.open)
            .resizable(true)
            .collapsible(false)
            .default_width(400.0)
            .default_height(300.0)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Extended Brookshear Machine Instructions");
                    ui.label(concat!(
                        "The Extended Brookshear Machine has 16 instructions, ",
                        "having a fixed length of two bytes per instruction. ",
                        "Those two bytes contain 1 nibble for the opcode, and ",
                        "up to 3 nibbles for operands. ",
                        "The instruction set is as follows:",
                    ));
                    egui::Grid::new("instructions_grid").show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        for (name, description) in [
                            ("0FFF", "No operation.."),
                            ("1rxy", "Load memory[xy] into Rr."),
                            ("2rxy", "Load value xy into Rr."),
                            ("3rxy", "Store Rr into memory[xy]."),
                            ("40rs", "Move Rr to Rs"),
                            ("5rst", "Add as ints, Rs, Rt, put result in Rr"),
                            ("6rst", "Add as floats, Rs, Rt, put result in Rr"),
                            ("7rst", "OR each bit of Rs and Rt, put result in Rr"),
                            ("8rst", "AND each bit of Rs and Rt, put result in Rr"),
                            ("9rst", "XOR each bit of Rs and Rt, put result in Rr"),
                            ("Ar0x", "Rotate Rr right by x bits"),
                            ("Brxy", "Jump to address xy if Rr equals R0"),
                            ("C000", "Halt"),
                            ("D0rs", "Load Rr from memory[Rs]"),
                            ("E0rs", "Store Rr in memory[Rs]"),
                            (
                                "Frxt",
                                r#"Jump to address in Rt if Rr test R0
x = 0 means test is equals
x = 1 means test is not equals
x = 2 means test is greater or equal
x = 3 means test is less or equal
x = 4 means test is greater than
x = 5 means test is less than"#,
                            ),
                        ] {
                            ui.vertical(|ui| {
                                ui.label(name);
                            });
                            ui.label(description);
                            ui.end_row();
                        }
                    });
                });
            });
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct HelpWindow {
    open: bool,
    page: HelpPage,
    #[serde(skip)]
    md_cache: CommonMarkCache,
}

impl HelpWindow {
    const BM_HELP_LINK: &'static str = "./bmhelp.md";
    const ASM_HELP_LINK: &'static str = "./asmhelp.md";

    fn initialize(&mut self) {
        self.md_cache.add_link_hook(Self::BM_HELP_LINK);
        self.md_cache.add_link_hook(Self::ASM_HELP_LINK);
    }

    fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new(match self.page {
            HelpPage::General => "General Help",
            HelpPage::Assembler => "Assembler Help",
        })
        .id(egui::Id::new("help_window"))
        .open(&mut self.open)
        .resizable(true)
        .collapsible(false)
        .default_width(400.0)
        .default_height(300.0)
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                CommonMarkViewer::new().enable_scroll_to_heading(true).show(
                    ui,
                    &mut self.md_cache,
                    match self.page {
                        HelpPage::General => {
                            include_str!("../../../../doc/for_embedding/bmhelp.md")
                        }
                        HelpPage::Assembler => {
                            include_str!("../../../../doc/for_embedding/asmhelp.md")
                        }
                    },
                );
            });
        });

        if self.md_cache.get_link_hook(Self::ASM_HELP_LINK) == Some(true) {
            self.page = HelpPage::Assembler;
        } else if self.md_cache.get_link_hook(Self::BM_HELP_LINK) == Some(true) {
            self.page = HelpPage::General;
        }
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct MessageDetailsWindow {
    open: bool,
}

impl MessageDetailsWindow {
    fn show(&mut self, ctx: &egui::Context, rich_text: Option<&MyRichText>) {
        if rich_text.is_none() {
            self.open = false;
        }

        egui::Window::new("Message Details")
            .open(&mut self.open)
            .frame(Frame::window(&ctx.global_style()).inner_margin(egui::Margin::ZERO))
            .resizable(true)
            .collapsible(false)
            .default_width(400.0)
            .default_height(300.0)
            .show(ctx, |ui| {
                *ui.style_mut() = ui.ctx().options(|o| (*o.dark_style).clone());
                Frame::group(ui.style())
                    .fill(egui::Color32::from_gray(20))
                    .show(ui, |ui| {
                        ScrollArea::both()
                            .auto_shrink(egui::Vec2b::FALSE)
                            .show(ui, |ui| {
                                if let Some(rich_text) = rich_text {
                                    ui.style_mut().override_text_style =
                                        Some(egui::TextStyle::Monospace);
                                    ui.style_mut().visuals.override_text_color =
                                        Some(crate::ansi::WHITE);
                                    let job = rich_text.layout(ui.style());
                                    let galley = ui.fonts_mut(|f| f.layout_job(job));
                                    ui.label(galley);
                                } else {
                                    ui.label("No details available.");
                                }
                            });
                    });
            });
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct AppWindows {
    instructions: InstructionsWindow,
    about: AboutWindow,
    help: HelpWindow,
    message_details: MessageDetailsWindow,
}

impl AppWindows {
    pub fn initialize(&mut self) {
        self.help.initialize();
    }

    pub fn open(&mut self, window: WindowOpenId) {
        match window {
            WindowOpenId::Instructions => self.instructions.open = true,
            WindowOpenId::About => self.about.open = true,
            WindowOpenId::Help(Some(page)) => {
                self.help.open = true;
                self.help.page = page;
            }
            WindowOpenId::Help(None) => {
                self.help.open = true;
            }
            WindowOpenId::MessageDetails => self.message_details.open = true,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, message_details_rich_text: Option<&MyRichText>) {
        self.instructions.show(ctx);
        self.about.show(ctx);
        self.help.show(ctx);
        self.message_details.show(ctx, message_details_rich_text);
    }
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
