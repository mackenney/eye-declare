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
        if !text.is_empty() {
            self.current_spans
                .push(Span::styled(text.to_string(), self.current_style()));
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
                        format!("{}• ", indent)
                    }
                } else {
                    "• ".to_string()
                };
                self.current_spans
                    .push(Span::styled(marker, self.styles.marker_style));
            }
            Event::End(TagEnd::Item) => {
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
                _ => {}
            }
        }

        let num_cols = header_cells.len();
        if num_cols == 0 {
            return;
        }

        let available_width: usize = 120;
        let border_overhead = 1 + num_cols + 1;

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
            let padded = format!(" {:width$}", truncated, width = content_width);
            spans.push(Span::styled(padded, style));
            spans.push(Span::styled("│".to_string(), self.styles.base_style));
        }

        for &width in widths.iter().skip(cells.len()) {
            let content_width = width - 2;
            let padded = format!(" {:width$}", "", width = content_width);
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

/// Convert a pulldown-cmark event to an owned ('static) version.
fn event_to_owned(event: Event<'_>) -> Event<'static> {
    match event {
        Event::Start(tag) => Event::Start(tag_to_owned(tag)),
        Event::End(tag_end) => Event::End(tag_end),
        Event::Text(text) => Event::Text(text.to_string().into()),
        Event::Code(text) => Event::Code(text.to_string().into()),
        Event::Html(text) => Event::Html(text.to_string().into()),
        Event::InlineHtml(text) => Event::InlineHtml(text.to_string().into()),
        Event::FootnoteReference(text) => Event::FootnoteReference(text.to_string().into()),
        Event::SoftBreak => Event::SoftBreak,
        Event::HardBreak => Event::HardBreak,
        Event::Rule => Event::Rule,
        Event::TaskListMarker(checked) => Event::TaskListMarker(checked),
        Event::InlineMath(text) => Event::InlineMath(text.to_string().into()),
        Event::DisplayMath(text) => Event::DisplayMath(text.to_string().into()),
    }
}

fn tag_to_owned(tag: Tag<'_>) -> Tag<'static> {
    match tag {
        Tag::Paragraph => Tag::Paragraph,
        Tag::Heading {
            level,
            id,
            classes,
            attrs,
        } => Tag::Heading {
            level,
            id: id.map(|s| s.to_string().into()),
            classes: classes.into_iter().map(|s| s.to_string().into()).collect(),
            attrs: attrs
                .into_iter()
                .map(|(k, v)| (k.to_string().into(), v.map(|s| s.to_string().into())))
                .collect(),
        },
        Tag::BlockQuote(kind) => Tag::BlockQuote(kind),
        Tag::CodeBlock(kind) => Tag::CodeBlock(match kind {
            pulldown_cmark::CodeBlockKind::Indented => pulldown_cmark::CodeBlockKind::Indented,
            pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                pulldown_cmark::CodeBlockKind::Fenced(lang.to_string().into())
            }
        }),
        Tag::List(start) => Tag::List(start),
        Tag::Item => Tag::Item,
        Tag::FootnoteDefinition(text) => Tag::FootnoteDefinition(text.to_string().into()),
        Tag::DefinitionList => Tag::DefinitionList,
        Tag::DefinitionListTitle => Tag::DefinitionListTitle,
        Tag::DefinitionListDefinition => Tag::DefinitionListDefinition,
        Tag::Table(alignments) => Tag::Table(alignments),
        Tag::TableHead => Tag::TableHead,
        Tag::TableRow => Tag::TableRow,
        Tag::TableCell => Tag::TableCell,
        Tag::Emphasis => Tag::Emphasis,
        Tag::Strong => Tag::Strong,
        Tag::Strikethrough => Tag::Strikethrough,
        Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        } => Tag::Link {
            link_type,
            dest_url: dest_url.to_string().into(),
            title: title.to_string().into(),
            id: id.to_string().into(),
        },
        Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        } => Tag::Image {
            link_type,
            dest_url: dest_url.to_string().into(),
            title: title.to_string().into(),
            id: id.to_string().into(),
        },
        Tag::HtmlBlock => Tag::HtmlBlock,
        Tag::MetadataBlock(kind) => Tag::MetadataBlock(kind),
        Tag::Superscript => Tag::Superscript,
        Tag::Subscript => Tag::Subscript,
    }
}

/// Parse markdown source into styled ratatui Text.
fn render_markdown(source: &str, styles: &MarkdownState) -> Text<'static> {
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options);

    let mut state = RenderState::new(styles);

    for event in parser {
        state.process_event(event_to_owned(event));
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
}
