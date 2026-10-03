use arrow_array::RecordBatch;
use arrow_cast::display::{ArrayFormatter, FormatOptions};
use arrow_schema::Schema;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

const MIN_COLUMN_WIDTH: usize = 20;
const HEADER_ROWS: usize = 1;
const CELL_PADDING: usize = 2;
const NULL: &str = "NULL";
const LINK_LABEL: &str = "Open link ↗";

#[derive(Clone, PartialEq)]
pub(crate) struct Cell {
    pub(crate) column: usize,
    pub(crate) preview: Preview,
    pub(crate) selected: bool,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Preview {
    pub(crate) text: Rc<str>,
    pub(crate) href: Rc<str>,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Row {
    pub(crate) number: usize,
    pub(crate) cells: Vec<Cell>,
    pub(crate) selected: bool,
}

struct Batch {
    start: usize,
    records: RecordBatch,
}

pub(crate) struct Results {
    pub(crate) schema: Schema,
    pub(crate) received: usize,
    batches: Vec<Batch>,
    previews: RefCell<BTreeMap<(usize, usize, usize), Preview>>,
}

impl Default for Results {
    fn default() -> Self {
        Self {
            schema: Schema::empty(),
            received: 0,
            batches: Vec::new(),
            previews: RefCell::default(),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Viewport {
    pub(crate) row: usize,
    pub(crate) column: usize,
    top: usize,
    left: usize,
    rows: usize,
    columns: usize,
    width: usize,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            row: 0,
            column: 0,
            top: 0,
            left: 0,
            rows: 1,
            columns: 1,
            width: MIN_COLUMN_WIDTH,
        }
    }
}

impl Viewport {
    pub(crate) fn reset(&mut self) {
        self.row = 0;
        self.column = 0;
        self.reveal();
    }

    pub(crate) fn resize(&mut self, width: u16, height: u16) {
        self.rows = usize::from(height).saturating_sub(HEADER_ROWS).max(1);
        self.columns = (usize::from(width) / MIN_COLUMN_WIDTH).max(1);
        self.width = usize::from(width);
        self.reveal();
    }

    pub(crate) fn navigate(&mut self, key: hypercmd::Key, results: &Results) {
        use hypercmd::Key;
        match key {
            Key::Up => self.row = self.row.saturating_sub(1),
            Key::Down => self.row = self.row.saturating_add(1),
            Key::Left => self.column = self.column.saturating_sub(1),
            Key::Right => self.column = self.column.saturating_add(1),
            Key::PageUp => self.row = self.row.saturating_sub(self.rows),
            Key::PageDown => self.row = self.row.saturating_add(self.rows),
            Key::Home => self.row = 0,
            Key::End => self.row = results.received.saturating_sub(1),
            _ => return,
        }
        self.row = self.row.min(results.received.saturating_sub(1));
        self.column = self
            .column
            .min(results.schema.fields().len().saturating_sub(1));
        self.reveal();
    }

    fn reveal(&mut self) {
        self.top = self
            .top
            .min(self.row)
            .max((self.row + 1).saturating_sub(self.rows));
        self.left = self
            .left
            .min(self.column)
            .max((self.column + 1).saturating_sub(self.columns));
    }

    fn cell_width(&self, results: &Results) -> usize {
        (self.width / self.columns(results).len().max(1)).saturating_sub(CELL_PADDING)
    }

    fn columns(&self, results: &Results) -> std::ops::Range<usize> {
        self.left..(self.left + self.columns).min(results.schema.fields().len())
    }
}

impl Results {
    pub(crate) fn append(&mut self, records: RecordBatch) {
        let start = self.received;
        self.received += records.num_rows();
        if records.num_rows() > 0 {
            self.batches.push(Batch { start, records });
        }
    }

    pub(crate) fn headers(&self, viewport: Viewport) -> Vec<Cell> {
        let cell_width = viewport.cell_width(self);
        viewport
            .columns(self)
            .map(|column| Cell {
                column,
                preview: Preview {
                    text: hypercmd::text::ellipsize(self.schema.field(column).name(), cell_width)
                        .into(),
                    href: Rc::default(),
                },
                selected: column == viewport.column,
            })
            .collect()
    }

    pub(crate) fn rows(&self, viewport: Viewport) -> Vec<Row> {
        let cell_width = viewport.cell_width(self);
        self.previews
            .borrow_mut()
            .retain(|(row, column, width), _| {
                (viewport.top..viewport.top + viewport.rows).contains(row)
                    && viewport.columns(self).contains(column)
                    && *width == cell_width
            });
        (viewport.top..(viewport.top + viewport.rows).min(self.received))
            .map(|number| Row {
                number,
                selected: number == viewport.row,
                cells: viewport
                    .columns(self)
                    .map(|column| Cell {
                        column,
                        preview: self.preview(number, column, cell_width),
                        selected: number == viewport.row && column == viewport.column,
                    })
                    .collect(),
            })
            .collect()
    }

    fn preview(&self, row: usize, column: usize, width: usize) -> Preview {
        self.previews
            .borrow_mut()
            .entry((row, column, width))
            .or_insert_with(|| {
                let value = self.value(row, column);
                let hyperlink = value.starts_with("https://") || value.starts_with("http://");
                let text =
                    hypercmd::text::ellipsize(if hyperlink { LINK_LABEL } else { &value }, width)
                        .into();
                let href = if hyperlink {
                    value.into()
                } else {
                    Rc::default()
                };
                Preview { text, href }
            })
            .clone()
    }

    pub(crate) fn value(&self, row: usize, column: usize) -> String {
        let index = self.batches.partition_point(|batch| batch.start <= row);
        let index = index
            .checked_sub(1)
            .expect("a displayed row belongs to a retained batch");
        let batch = &self.batches[index];
        let options = FormatOptions::new().with_null(NULL);
        let value = ArrayFormatter::try_new(batch.records.column(column).as_ref(), &options)
            .and_then(|formatter| formatter.value(row - batch.start).try_to_string());
        match value {
            Ok(value) => value,
            Err(error) => format!("Cannot display this value: {error}"),
        }
    }

    pub(crate) fn position(&self, viewport: Viewport) -> String {
        if self.received == 0 || self.schema.fields().is_empty() {
            return format!(
                "{} rows · {} columns",
                self.received,
                self.schema.fields().len()
            );
        }
        format!(
            "Row {} / {} · Column {} / {} · {}",
            viewport.row + 1,
            self.received,
            viewport.column + 1,
            self.schema.fields().len(),
            self.schema.field(viewport.column).name()
        )
    }
}
