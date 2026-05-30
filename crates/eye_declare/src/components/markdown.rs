use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::Widget,
};

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::Elements;
use crate::components::Canvas;

/// Renders a subset of Markdown as styled terminal text.
///
/// Supports headings (`#`, `##`, `###`), **bold**, *italic*, `inline code`,
/// fenced code blocks, and unordered lists. Designed for rendering
/// LLM/AI chat output in the terminal.
///
/// The markdown source is a prop; style configuration lives in
/// [`MarkdownState`] (internal state with sensible defaults).
///
/// # Examples
///
/// ```ignore
/// // Constructor
/// Markdown::new("# Hello\n\nThis is **bold** and `code`.")
///
/// // In the element! macro
/// element! {
///     Markdown(key: "response", source: response_text.clone())
/// }
/// ```
#[derive(Default, typed_builder::TypedBuilder)]
pub struct Markdown {
    /// The markdown source text to render.
    #[builder(default, setter(into))]
    pub source: String,
}

impl Markdown {
    /// Create a new markdown component with the given source text.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

/// Style configuration for a [`Markdown`] component.
///
/// Initialized with sensible terminal defaults (cyan headings, yellow
/// inline code, green code blocks, etc.). Override individual fields
/// to match your application's color scheme.
///
/// The markdown source text is a prop on the [`Markdown`] struct,
/// not part of state.
pub struct MarkdownState {
    /// Base style for normal text.
    pub base_style: Style,
    /// Style for inline code.
    pub code_style: Style,
    /// Style for code blocks.
    pub block_code_style: Style,
    /// Style for bold text.
    pub bold_style: Style,
    /// Style for italic text.
    pub italic_style: Style,
    /// Style for headings.
    pub heading_style: Style,
    /// Style for list markers.
    pub marker_style: Style,
}

impl MarkdownState {
    /// Create the default style configuration matching pi's dark theme RGB values.
    /// Colors sourced from pi's dark.json: mdHeading=#f0c674, mdCode=#8abeb7 (accent),
    /// mdCodeBlock=#b5bd68 (green), mdListBullet=#8abeb7 (accent).
    pub fn new() -> Self {
        let base = Style::default();
        Self {
            base_style: base,
            // mdCode: accent = #8abeb7 = Rgb(138, 190, 183)
            code_style: Style::default().fg(Color::Rgb(138, 190, 183)),
            // mdCodeBlock: green = #b5bd68 = Rgb(181, 189, 104)
            block_code_style: Style::default().fg(Color::Rgb(181, 189, 104)),
            bold_style: base.add_modifier(Modifier::BOLD),
            italic_style: base.add_modifier(Modifier::ITALIC),
            // mdHeading: #f0c674 = Rgb(240, 198, 116)
            heading_style: Style::default()
                .fg(Color::Rgb(240, 198, 116))
                .add_modifier(Modifier::BOLD),
            // mdListBullet: accent = #8abeb7 = Rgb(138, 190, 183)
            marker_style: Style::default().fg(Color::Rgb(138, 190, 183)),
        }
    }
}

impl Default for MarkdownState {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal state for the pulldown-cmark event walker.
struct RenderState<'a> {
    styles: &'a MarkdownState,
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<ListContext>,
    blockquote_depth: u32,
    in_code_block: bool,
    table_state: Option<TableState>,
    pending_list_marker: Option<String>,
}

#[derive(Clone)]
struct ListContext {
    ordered: bool,
    next_number: u64,
}

struct TableState {
    events: Vec<Event<'static>>,
}

impl<'a> RenderState<'a> {
    fn new(styles: &'a MarkdownState) -> Self {
        Self {
            styles,
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![styles.base_style],
            list_stack: Vec::new(),
            blockquote_depth: 0,
            in_code_block: false,
            table_state: None,
            pending_list_marker: None,
        }
    }

    fn current_style(&self) -> Style {
        *self.style_stack.last().unwrap_or(&self.styles.base_style)
    }

    fn push_style(&mut self, style: Style) {
        self.style_stack.push(style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let prefix = self.line_prefix();
            let mut spans = if prefix.is_empty() {
                Vec::new()
            } else {
                vec![Span::styled(prefix, self.styles.marker_style)]
            };
            spans.append(&mut self.current_spans);
            self.lines.push(Line::from(spans));
        }
    }

    fn push_empty_line(&mut self) {
        self.lines.push(Line::from(""));
    }

    fn line_prefix(&self) -> String {
        let mut prefix = String::new();
        for _ in 0..self.blockquote_depth {
            prefix.push_str("│ ");
        }
        prefix
    }

    fn emit_text(&mut self, text: &str) {
        self.flush_pending_marker();
        if !text.is_empty() {
            self.current_spans
                .push(Span::styled(text.to_string(), self.current_style()));
        }
    }

    fn flush_pending_marker(&mut self) {
        if let Some(marker) = self.pending_list_marker.take() {
            self.current_spans
                .push(Span::styled(marker, self.styles.marker_style));
        }
    }

    fn finish(mut self) -> Text<'static> {
        self.flush_line();
        Text::from(self.lines)
    }

    fn process_event(&mut self, event: Event<'static>) {
        if let Some(ref mut table_state) = self.table_state {
            if matches!(event, Event::End(TagEnd::Table)) {
                let events = std::mem::take(&mut table_state.events);
                self.table_state = None;
                self.render_table(events);
                return;
            }
            table_state.events.push(event);
            return;
        }

        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                self.flush_line();
                let style = match level {
                    HeadingLevel::H1 => {
                        self.styles.heading_style.add_modifier(Modifier::UNDERLINED)
                    }
                    HeadingLevel::H2 => self.styles.heading_style,
                    _ => self.styles.heading_style,
                };
                if level as u8 >= 3 {
                    let prefix = "#".repeat(level as usize) + " ";
                    self.current_spans.push(Span::styled(prefix, style));
                }
                self.push_style(style);
            }
            Event::End(TagEnd::Heading(_)) => {
                self.pop_style();
                self.flush_line();
            }

            Event::Start(Tag::Paragraph) => {
                self.flush_line();
            }
            Event::End(TagEnd::Paragraph) => {
                self.flush_line();
                self.push_empty_line();
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                self.flush_line();
                self.in_code_block = true;
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                };
                let fence = if lang.is_empty() {
                    "```".to_string()
                } else {
                    format!("```{}", lang)
                };
                self.lines
                    .push(Line::from(Span::styled(fence, self.styles.marker_style)));
            }
            Event::End(TagEnd::CodeBlock) => {
                self.in_code_block = false;
                self.lines.push(Line::from(Span::styled(
                    "```".to_string(),
                    self.styles.marker_style,
                )));
            }

            Event::Start(Tag::List(first_number)) => {
                self.flush_line();
                self.list_stack.push(ListContext {
                    ordered: first_number.is_some(),
                    next_number: first_number.unwrap_or(1),
                });
            }
            Event::End(TagEnd::List(_)) => {
                self.list_stack.pop();
            }

            Event::Start(Tag::Item) => {
                self.flush_line();
                let indent = "    ".repeat(self.list_stack.len().saturating_sub(1));
                let marker = if let Some(ctx) = self.list_stack.last_mut() {
                    if ctx.ordered {
                        let m = format!("{}{}. ", indent, ctx.next_number);
                        ctx.next_number += 1;
                        m
                    } else {
                        format!("{}\u{2022} ", indent)
                    }
                } else {
                    "\u{2022} ".to_string()
                };
                self.pending_list_marker = Some(marker);
            }
            Event::End(TagEnd::Item) => {
                self.flush_pending_marker();
                self.flush_line();
            }

            Event::Start(Tag::BlockQuote(_)) => {
                self.flush_line();
                self.blockquote_depth += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                self.flush_line();
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
            }

            Event::Start(Tag::Table(_)) => {
                self.flush_line();
                self.table_state = Some(TableState { events: Vec::new() });
            }
            Event::End(TagEnd::Table) => {}

            Event::Start(Tag::Strong) => {
                self.push_style(self.styles.bold_style);
            }
            Event::End(TagEnd::Strong) => {
                self.pop_style();
            }

            Event::Start(Tag::Emphasis) => {
                self.push_style(self.styles.italic_style);
            }
            Event::End(TagEnd::Emphasis) => {
                self.pop_style();
            }

            Event::Start(Tag::Strikethrough) => {
                let style = self.current_style().add_modifier(Modifier::CROSSED_OUT);
                self.push_style(style);
            }
            Event::End(TagEnd::Strikethrough) => {
                self.pop_style();
            }

            Event::Start(Tag::Link { .. }) => {
                let style = self.styles.base_style.add_modifier(Modifier::UNDERLINED);
                self.push_style(style);
            }
            Event::End(TagEnd::Link) => {
                self.pop_style();
            }

            Event::Code(text) => {
                self.flush_pending_marker();
                self.current_spans
                    .push(Span::styled(text.to_string(), self.styles.code_style));
            }

            Event::Text(text) => {
                if self.in_code_block {
                    for line in text.lines() {
                        self.lines.push(Line::from(Span::styled(
                            format!("  {}", line),
                            self.styles.block_code_style,
                        )));
                    }
                } else {
                    self.emit_text(&text);
                }
            }

            Event::SoftBreak => {
                self.emit_text(" ");
            }
            Event::HardBreak => {
                self.flush_line();
            }

            Event::Rule => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(40),
                    self.styles.marker_style,
                )));
                self.push_empty_line();
            }

            Event::TaskListMarker(checked) => {
                self.pending_list_marker = None;
                let marker = if checked { "[x] " } else { "[ ] " };
                self.current_spans
                    .push(Span::styled(marker.to_string(), self.styles.marker_style));
            }

            _ => {}
        }
    }

    fn render_table(&mut self, events: Vec<Event<'static>>) {
        let mut header_cells: Vec<String> = Vec::new();
        let mut body_rows: Vec<Vec<String>> = Vec::new();
        let mut current_row: Vec<String> = Vec::new();
        let mut current_cell = String::new();
        let mut in_header = false;
        let mut in_cell = false;

        for event in events {
            match event {
                Event::Start(Tag::TableHead) => {
                    in_header = true;
                }
                Event::End(TagEnd::TableHead) => {
                    header_cells = std::mem::take(&mut current_row);
                    in_header = false;
                }
                Event::Start(Tag::TableRow) => {
                    current_row.clear();
                }
                Event::End(TagEnd::TableRow) if !in_header && !current_row.is_empty() => {
                    body_rows.push(std::mem::take(&mut current_row));
                }
                Event::End(TagEnd::TableRow) => {}
                Event::Start(Tag::TableCell) => {
                    in_cell = true;
                    current_cell.clear();
                }
                Event::End(TagEnd::TableCell) => {
                    in_cell = false;
                    current_row.push(std::mem::take(&mut current_cell));
                }
                Event::Text(text) if in_cell => {
                    current_cell.push_str(&text);
                }
                Event::Code(text) if in_cell => {
                    current_cell.push_str(&text);
                }
                Event::SoftBreak if in_cell => {
                    current_cell.push(' ');
                }
                _ => {}
            }
        }

        let num_cols = header_cells.len();
        if num_cols == 0 {
            return;
        }

        let available_width: usize = 120;
        let border_overhead = num_cols + 1;

        let mut natural_widths: Vec<usize> =
            header_cells.iter().map(|c| c.chars().count()).collect();
        for row in &body_rows {
            for (i, cell) in row.iter().enumerate() {
                if i < natural_widths.len() {
                    natural_widths[i] = natural_widths[i].max(cell.chars().count());
                }
            }
        }

        let padded_widths: Vec<usize> = natural_widths.iter().map(|w| w + 2).collect();
        let total_natural: usize = padded_widths.iter().sum::<usize>() + border_overhead;

        let final_widths = if total_natural <= available_width {
            padded_widths
        } else {
            let min_widths: Vec<usize> = natural_widths.iter().map(|_| 3).collect();
            let total_min: usize = min_widths.iter().sum::<usize>() + border_overhead;

            if total_min > available_width {
                self.render_table_fallback(&header_cells, &body_rows);
                return;
            }

            let available_for_content = available_width - border_overhead;
            let total_natural_content: usize = natural_widths.iter().sum();
            let ratio = available_for_content as f64 / total_natural_content as f64;
            natural_widths
                .iter()
                .map(|w| ((*w as f64 * ratio).floor() as usize).max(1) + 2)
                .collect()
        };

        self.render_table_line('┌', '─', '┬', '┐', &final_widths);
        self.render_table_row(&header_cells, &final_widths, true);
        self.render_table_line('├', '─', '┼', '┤', &final_widths);

        for (i, row) in body_rows.iter().enumerate() {
            self.render_table_row(row, &final_widths, false);
            if i < body_rows.len() - 1 {
                self.render_table_line('├', '─', '┼', '┤', &final_widths);
            }
        }

        self.render_table_line('└', '─', '┴', '┘', &final_widths);
    }

    fn render_table_line(
        &mut self,
        left: char,
        fill: char,
        mid: char,
        right: char,
        widths: &[usize],
    ) {
        let mut line = String::new();
        line.push(left);
        for (i, &w) in widths.iter().enumerate() {
            line.push_str(&fill.to_string().repeat(w));
            if i < widths.len() - 1 {
                line.push(mid);
            }
        }
        line.push(right);
        self.lines
            .push(Line::from(Span::styled(line, self.styles.base_style)));
    }

    fn render_table_row(&mut self, cells: &[String], widths: &[usize], is_header: bool) {
        let style = if is_header {
            self.styles.bold_style
        } else {
            self.styles.base_style
        };

        let mut spans = Vec::new();
        spans.push(Span::styled("│".to_string(), self.styles.base_style));

        for (cell, &width) in cells.iter().zip(widths.iter()) {
            let content_width = width - 2;
            let truncated: String = cell.chars().take(content_width).collect();
            let padded = format!(" {:<width$} ", truncated, width = content_width);
            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│".to_string(), self.styles.base_style));
        }

        for &width in widths.iter().skip(cells.len()) {
            let content_width = width - 2;
            let padded = format!(" {:<width$} ", "", width = content_width);
            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│".to_string(), self.styles.base_style));
        }

        self.lines.push(Line::from(spans));
    }

    fn render_table_fallback(&mut self, header: &[String], body: &[Vec<String>]) {
        let header_line = format!("| {} |", header.join(" | "));
        let separator = format!(
            "|{}|",
            header.iter().map(|_| "---").collect::<Vec<_>>().join("|")
        );

        self.lines.push(Line::from(Span::styled(
            header_line,
            self.styles.base_style,
        )));
        self.lines
            .push(Line::from(Span::styled(separator, self.styles.base_style)));

        for row in body {
            let row_line = format!("| {} |", row.join(" | "));
            self.lines
                .push(Line::from(Span::styled(row_line, self.styles.base_style)));
        }
    }
}

#[eye_declare_macros::component(props = Markdown, state = MarkdownState, initial_state = MarkdownState::new(), crate_path = crate)]
fn markdown(props: &Markdown, state: &MarkdownState) -> Elements {
    if props.source.is_empty() {
        return Elements::new();
    }
    let text = render_markdown(&props.source, state);
    let text_for_height = text.clone();
    let mut els = Elements::new();
    els.add(
        Canvas::builder()
            .render_fn(move |area: Rect, buf: &mut Buffer| {
                crate::wrap::wrapping_paragraph(text.clone()).render(area, buf);
            })
            .desired_height_fn(move |width: u16| {
                crate::wrap::wrapped_line_count(&text_for_height, width)
            })
            .build(),
    );
    els
}

/// Parse markdown source into styled ratatui Text.
fn render_markdown(source: &str, styles: &MarkdownState) -> Text<'static> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options);

    let mut state = RenderState::new(styles);

    for event in parser {
        state.process_event(event.into_static());
    }

    state.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_renders() {
        let md = Markdown::new("# Title");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        assert_eq!(text.lines.len(), 1);
        assert!(
            text.lines[0]
                .spans
                .iter()
                .any(|s| s.content.contains("Title"))
        );
    }

    #[test]
    fn code_block_indented() {
        let md = Markdown::new("```rust\nfn main() {}\n```");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        // opening fence + content + closing fence = 3 lines
        assert!(text.lines.len() >= 3);
        assert!(
            text.lines[0].to_string().contains("```rust"),
            "opening fence must appear"
        );
        assert!(
            text.lines[1].to_string().contains("fn main"),
            "code content must appear"
        );
        assert!(
            text.lines.last().unwrap().to_string().contains("```"),
            "closing fence must appear"
        );
    }

    #[test]
    fn inline_bold() {
        let md = Markdown::new("This is **bold** text");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let spans = &text.lines[0].spans;
        assert!(spans.len() >= 3);
        let bold_span = spans.iter().find(|s| s.content.contains("bold")).unwrap();
        assert!(bold_span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn inline_italic() {
        let md = Markdown::new("This is *italic* text");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let spans = &text.lines[0].spans;
        let italic_span = spans.iter().find(|s| s.content.contains("italic")).unwrap();
        assert!(italic_span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn inline_code() {
        let md = Markdown::new("Use `println!` here");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let spans = &text.lines[0].spans;
        let code_span = spans
            .iter()
            .find(|s| s.content.contains("println!"))
            .unwrap();
        assert_eq!(code_span.style.fg, Some(Color::Rgb(138, 190, 183)));
    }

    #[test]
    fn list_items() {
        let md = Markdown::new("- item one\n- item two");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        assert_eq!(text.lines.len(), 2);
    }

    #[test]
    fn unclosed_markers_render_as_text() {
        let md = Markdown::new("This has an unclosed **bold");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(full_text.contains("**bold"));
    }

    #[test]
    fn mixed_formatting() {
        let md = Markdown::new(
            "# Welcome\n\nThis is **bold** and *italic* with `code`.\n\n```\nlet x = 1;\n```\n\n- item",
        );
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        assert!(text.lines.len() >= 5);
    }

    #[test]
    fn table_basic_structure() {
        let md = Markdown::new("| A | B |\n|---|---|\n| 1 | 2 |");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        assert!(text.lines.len() >= 5, "table should have at least 5 lines");
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(full_text.contains("┌"), "should have top-left corner");
        assert!(full_text.contains("┘"), "should have bottom-right corner");
        assert!(full_text.contains("│"), "should have column dividers");
    }

    #[test]
    fn table_header_is_bold() {
        let md = Markdown::new("| Header |\n|--------|\n| Cell |");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let header_line = text
            .lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.content.contains("Header")))
            .expect("should have header line");
        let header_span = header_line
            .spans
            .iter()
            .find(|s| s.content.contains("Header"))
            .expect("should have header span");
        assert!(
            header_span.style.add_modifier.contains(Modifier::BOLD),
            "header should be bold"
        );
    }

    #[test]
    fn table_row_separators() {
        let md = Markdown::new("| A |\n|---|\n| 1 |\n| 2 |");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        let separator_count = full_text.matches('├').count();
        assert!(
            separator_count >= 2,
            "should have separators between all rows, found {}",
            separator_count
        );
    }

    #[test]
    fn ordered_list_numbered() {
        let md = Markdown::new("1. first\n2. second\n3. third");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(full_text.contains("1."), "should have '1.'");
        assert!(full_text.contains("2."), "should have '2.'");
        assert!(full_text.contains("3."), "should have '3.'");
    }

    #[test]
    fn nested_list_indented() {
        let md = Markdown::new("- parent\n  - child\n  - child2\n- parent2");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        assert!(text.lines.len() >= 4, "should have at least 4 list items");
    }

    #[test]
    fn strikethrough_modifier() {
        let md = Markdown::new("This is ~~deleted~~ text");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let strike_span = text.lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("deleted"))
            .expect("should have deleted span");
        assert!(
            strike_span
                .style
                .add_modifier
                .contains(Modifier::CROSSED_OUT),
            "strikethrough should have CROSSED_OUT modifier"
        );
    }

    #[test]
    fn horizontal_rule() {
        let md = Markdown::new("above\n\n---\n\nbelow");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(
            full_text.contains("─"),
            "should have horizontal rule character"
        );
    }

    #[test]
    fn link_underlined() {
        let md = Markdown::new("Click [here](http://example.com) for more");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let link_span = text.lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("here"))
            .expect("should have link text span");
        assert!(
            link_span.style.add_modifier.contains(Modifier::UNDERLINED),
            "link text should be underlined"
        );
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(
            !full_text.contains("http://"),
            "URL should not appear in rendered text"
        );
    }

    #[test]
    fn task_list_unchecked() {
        let md = Markdown::new("- [ ] todo item");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(full_text.contains("[ ]"), "should have unchecked marker");
    }

    #[test]
    fn task_list_checked() {
        let md = Markdown::new("- [x] done item");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text
            .lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(full_text.contains("[x]"), "should have checked marker");
    }

    #[test]
    fn heading_h1_underlined() {
        let md = Markdown::new("# Title");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let title_span = text.lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("Title"))
            .expect("should have title span");
        assert!(
            title_span.style.add_modifier.contains(Modifier::UNDERLINED),
            "H1 should be underlined"
        );
        assert!(
            title_span.style.add_modifier.contains(Modifier::BOLD),
            "H1 should be bold (from heading_style)"
        );
    }

    #[test]
    fn heading_h2_bold_not_underlined() {
        let md = Markdown::new("## Subtitle");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let subtitle_span = text.lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("Subtitle"))
            .expect("should have subtitle span");
        assert!(
            subtitle_span.style.add_modifier.contains(Modifier::BOLD),
            "H2 should be bold"
        );
        assert!(
            !subtitle_span
                .style
                .add_modifier
                .contains(Modifier::UNDERLINED),
            "H2 should NOT be underlined"
        );
    }

    #[test]
    fn heading_h3_shows_prefix() {
        let md = Markdown::new("### Section");
        let state = MarkdownState::new();
        let text = render_markdown(&md.source, &state);
        let full_text: String = text.lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(
            full_text.contains("###"),
            "H3 should show ### prefix, got: {}",
            full_text
        );
    }
}
