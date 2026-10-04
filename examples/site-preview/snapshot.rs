use std::io::{self, Read};
use vt100::{Cell, Color, Screen};

const CELL_WIDTH: u16 = 8;
const CELL_HEIGHT: u16 = 17;
const BASELINE: u16 = 13;

#[derive(Clone, Copy)]
pub(super) enum Format {
    Svg,
    Text,
}

struct TextRun<'screen> {
    cell: &'screen Cell,
    column: u16,
    row: u16,
    columns: u16,
    text: String,
}

pub(super) fn write(format: Format) -> Result<(), Box<dyn std::error::Error>> {
    let mut dimensions = std::env::args().skip(2);
    let columns = dimensions.next().ok_or("Pass columns")?.parse()?;
    let rows = dimensions.next().ok_or("Pass rows")?.parse()?;
    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes)?;
    let mut parser = vt100::Parser::new(rows, columns, 0);
    parser.process(&bytes);
    match format {
        Format::Text => println!("{}", parser.screen().contents()),
        Format::Svg => svg(parser.screen())?,
    }
    Ok(())
}

fn svg(screen: &Screen) -> Result<(), Box<dyn std::error::Error>> {
    let (rows, columns) = screen.size();
    println!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}">
<rect width="100%" height="100%" fill="#272a2c"/>
<g font-family="Menlo,Consolas,monospace" font-size="13">"##,
        columns * CELL_WIDTH,
        rows * CELL_HEIGHT,
    );
    for row in 0..rows {
        paint_row(screen, row)?;
    }
    println!("</g>\n</svg>");
    Ok(())
}

fn paint_row(screen: &Screen, row: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut column = 0;
    while column < screen.size().1 {
        let start = column;
        let cell = screen.cell(row, column).expect("cell is inside the screen");
        let mut text = String::new();
        while let Some(next) = screen
            .cell(row, column)
            .filter(|next| same_style(cell, next))
        {
            if next.has_contents() {
                text.push_str(&next.contents());
            } else if !next.is_wide_continuation() {
                text.push(' ');
            }
            column += 1;
        }
        paint(&TextRun {
            cell,
            column: start,
            row,
            columns: column - start,
            text,
        })?;
    }
    Ok(())
}

fn same_style(first: &Cell, second: &Cell) -> bool {
    first.fgcolor() == second.fgcolor()
        && first.bgcolor() == second.bgcolor()
        && first.bold() == second.bold()
        && first.italic() == second.italic()
        && first.underline() == second.underline()
        && first.inverse() == second.inverse()
}

fn paint(run: &TextRun<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let cell = run.cell;
    let left = run.column * CELL_WIDTH;
    let top = run.row * CELL_HEIGHT;
    let width = run.columns * CELL_WIDTH;
    let mut foreground = color(cell.fgcolor(), "#faf9f6")?;
    let mut background = color(cell.bgcolor(), "#272a2c")?;
    if cell.inverse() {
        std::mem::swap(&mut foreground, &mut background);
    }
    println!(
        r#"<rect x="{left}" y="{top}" width="{width}" height="{CELL_HEIGHT}" fill="{background}"/>"#,
    );
    if !run.text.trim().is_empty() {
        let weight = if cell.bold() { "bold" } else { "normal" };
        let style = if cell.italic() { "italic" } else { "normal" };
        let decoration = if cell.underline() {
            "underline"
        } else {
            "none"
        };
        let baseline = top + BASELINE;
        let escaped = run
            .text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        println!(
            r#"<text x="{left}" y="{baseline}" fill="{foreground}" font-weight="{weight}"
style="white-space:pre"
font-style="{style}" text-decoration="{decoration}" textLength="{width}"
lengthAdjust="spacingAndGlyphs">{escaped}</text>"#,
        );
    }
    Ok(())
}

fn color(color: Color, default: &str) -> Result<String, Box<dyn std::error::Error>> {
    match color {
        Color::Default => Ok(default.into()),
        Color::Rgb(red, green, blue) => Ok(format!("#{red:02x}{green:02x}{blue:02x}")),
        Color::Idx(index) => {
            Err(format!("Expected native RGB colors, found palette index {index}").into())
        }
    }
}
