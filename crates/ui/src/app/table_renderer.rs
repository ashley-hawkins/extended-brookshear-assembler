use egui::ScrollArea;
use egui_extras::Column;
use std::hash::Hash;

use crate::app::{MaybeRichError, MessageState};

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

pub trait TableColumn<D>: Copy
where
    D: TableData,
{
    type Context;

    fn header(self) -> &'static str;

    fn column(self) -> Column;

    fn display(self, row_index: usize, data: &D, context: &Self::Context) -> String;

    fn is_editable(self, _row_index: usize, _data: &D, _context: &Self::Context) -> bool {
        false
    }

    fn edit_row_advance(self, _row_index: usize, _data: &D, _context: &Self::Context) -> usize {
        1
    }

    fn try_set(
        self,
        _row_index: usize,
        _data: &mut D,
        _value: &str,
        _context: &Self::Context,
    ) -> Result<(), MaybeRichError> {
        Err(MaybeRichError::from("This cell is not editable."))
    }
}

pub trait TableData {
    type Row;

    fn len(&self) -> usize;
    fn get_row(&self, index: usize) -> &Self::Row;
    fn modify_row(&mut self, index: usize, f: impl FnOnce(&mut Self::Row));
}

impl<T, const N: usize> TableData for [T; N] {
    type Row = T;

    fn len(&self) -> usize {
        <[T]>::len(self)
    }

    fn get_row(&self, index: usize) -> &Self::Row {
        &self[index]
    }
    fn modify_row(&mut self, index: usize, f: impl FnOnce(&mut Self::Row)) {
        f(&mut self[index]);
    }
}

pub struct SelectableTable<'a, D, Col>
where
    D: TableData,
    Col: TableColumn<D>,
{
    id: egui::Id,
    columns: &'a [Col],
    data: &'a mut D,
    state: &'a mut EditableTableState,
    message_state: &'a mut MessageState,
    context: &'a Col::Context,
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

impl<'a, D, Col> SelectableTable<'a, D, Col>
where
    D: TableData,
    Col: TableColumn<D>,
{
    pub fn new(
        id_salt: impl Hash + std::fmt::Debug,
        columns: &'a [Col],
        data: &'a mut D,
        state: &'a mut EditableTableState,
        message_state: &'a mut MessageState,
        context: &'a Col::Context,
    ) -> Self {
        Self {
            id: egui::Id::unique(id_salt),
            columns,
            data,
            state,
            message_state,
            context,
            row_height: 20.0,
            min_scrolled_height: 80.0,
            scroll_to_row: None,
        }
    }

    pub fn row_height(mut self, row_height: f32) -> Self {
        self.row_height = row_height;
        self
    }

    pub fn min_scrolled_height(mut self, min_scrolled_height: f32) -> Self {
        self.min_scrolled_height = min_scrolled_height;
        self
    }

    pub fn scroll_to_row(mut self, scroll_to_row: Option<usize>) -> Self {
        self.scroll_to_row = scroll_to_row;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) {
        let row_count = self.data.len();

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
                body.rows(self.row_height, row_count, |mut row_ui| {
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
                            self.data,
                            column,
                            self.context,
                        );
                    }
                });
            });
        });
    }
}

fn render_table_cell<D, Col>(
    row_ui: &mut egui_extras::TableRow<'_, '_>,
    state: &mut EditableTableState,
    message_state: &mut MessageState,
    cell: CellLocation,
    data: &mut D,
    column: Col,
    context: &Col::Context,
) where
    D: TableData,
    Col: TableColumn<D>,
{
    row_ui.col(|ui| {
        let cell_coords = (cell.row, cell.column);
        let is_highlighted_cell = state.highlight.cell() == Some(cell_coords);
        let is_editable = column.is_editable(cell.row, data, context);

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
                    let row_advance = column.edit_row_advance(cell.row, data, context);
                    match column.try_set(cell.row, data, edit_str, context) {
                        Ok(()) => {
                            message_state.set_message(String::new());
                            state.set_highlighted_cell(
                                cell.row
                                    .saturating_add(row_advance)
                                    .min(cell.row_count.saturating_sub(1)),
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
            let value = column.display(cell.row, data, context);
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
