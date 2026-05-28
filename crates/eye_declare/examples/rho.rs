//! Rho — proof-of-concept replica of a pi coding-agent TUI session.
//!
//! Renders the fact-check conversation with: human messages in grey,
//! AI responses as markdown with a 1-column left margin, tool calls in
//! green/red, animated thinking spinner, and a static footer + editor.
//!
//! Run with:  cargo run --example rho
//! Exit with: Esc or q

use std::io;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use eye_declare::{
    Application, BorderType, Canvas, Cells, ControlFlow, Elements, Markdown, Spinner, View, element,
};
use ratatui_core::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use ratatui_widgets::paragraph::Paragraph;

#[derive(Clone)]
struct BashBlock {
    cmd: String,
    truncation: Option<String>,
    output: Vec<String>,
    elapsed: String,
    success: bool,
}

#[derive(Clone)]
struct AgentLine {
    indent: usize,
    success: bool,
    text: String,
}

#[derive(Clone)]
struct SubagentBlock {
    label: String,
    lines: Vec<AgentLine>,
    success: bool,
}

#[derive(Clone)]
enum Msg {
    Human(String),
    Think,
    SkillTag(String),
    ToolRead(String),
    ToolBash(BashBlock),
    ToolWrite { path: String, bytes: usize },
    ToolSubagent(SubagentBlock),
    AiMarkdown(String),
    Gap,
}

struct AppState {
    messages: Vec<Msg>,
}

fn session() -> Vec<Msg> {
    vec![
        Msg::Human(
            "I need a thorough fact-check pass over \
             artifacts/RHO_TUI_ARCHITECTURE_DECISION.md . I've been discussing / brainstorming \
             for this and the process has been notoriously flaky and error prone. I need you to \
             be brutal in terms of checking everything.\n\nmy plan is to see if the approach is \
             viable, if it is I want to build a mock app with the tui framework to see it in \
             action before integrating into rho"
                .into(),
        ),
        Msg::Gap,
        Msg::Think,
        Msg::Gap,
        Msg::SkillTag("fact-checker".into()),
        Msg::Gap,
        Msg::ToolRead("~/pr/rho/artifacts/RHO_TUI_ARCHITECTURE_DECISION.md".into()),
        Msg::Gap,
        Msg::AiMarkdown(
            "Now I'll read the document carefully and identify all verifiable claims \
             before dispatching checkers."
                .into(),
        ),
        Msg::Gap,
        Msg::ToolBash(BashBlock {
            cmd: "$ ls reference/pi/packages/tui/src/ 2>/dev/null || echo \"not found\"\n  \
                    ls reference/pi/packages/ 2>/dev/null | head -20"
                .into(),
            truncation: Some("... (14 earlier lines, ctrl+o to expand)".into()),
            output: vec![
                "agent".into(),
                "ai".into(),
                "coding-agent".into(),
                "tui".into(),
                "web-ui".into(),
            ],
            elapsed: "0.0s".into(),
            success: true,
        }),
        Msg::Gap,
        Msg::ToolBash(BashBlock {
            cmd: "$ ls crates/ 2>/dev/null && cat Cargo.toml 2>/dev/null | head -30".into(),
            truncation: Some("... (31 earlier lines, ctrl+o to expand)".into()),
            output: vec![
                "[workspace.dependencies]".into(),
                "# Async runtime".into(),
                "tokio = { version = \"1\", features = [\"full\"] }".into(),
                "tokio-util = { version = \"0.7\", features = [\"rt\"] }".into(),
                "futures = \"0.3\"".into(),
            ],
            elapsed: "0.0s".into(),
            success: true,
        }),
        Msg::Gap,
        Msg::AiMarkdown("Good. Now I'll dispatch three parallel checker agents.".into()),
        Msg::Gap,
        Msg::ToolSubagent(SubagentBlock {
            label: "subagent parallel (3)".into(),
            success: true,
            lines: vec![
                AgentLine {
                    indent: 0,
                    success: true,
                    text: "parallel · 3/3 done · 134 tool uses · 49k token · 6m59s".into(),
                },
                AgentLine {
                    indent: 2,
                    success: true,
                    text: "Agent 1/3: worker · 18 tool uses · 12k token · 3m45s".into(),
                },
                AgentLine {
                    indent: 4,
                    success: false,
                    text: "output: /home/ignacio/pr/rho/artifacts/factcheck_cluster_a_pi_source.md"
                        .into(),
                },
                AgentLine {
                    indent: 2,
                    success: true,
                    text: "Agent 2/3: worker · 77 tool uses · 23k token · 6m59s".into(),
                },
                AgentLine {
                    indent: 4,
                    success: false,
                    text: "output: /home/ignacio/pr/rho/artifacts/factcheck_cluster_b_ratatui.md"
                        .into(),
                },
                AgentLine {
                    indent: 2,
                    success: true,
                    text: "Agent 3/3: worker · 39 tool uses · 14k token · 4m30s".into(),
                },
                AgentLine {
                    indent: 4,
                    success: false,
                    text: "output: /home/ignacio/pr/rho/artifacts/factcheck_cluster_c_ecosystem.md"
                        .into(),
                },
            ],
        }),
        Msg::Gap,
        Msg::AiMarkdown(
            "Now I'll synthesize all three cluster reports into a final fact-check report.".into(),
        ),
        Msg::Gap,
        Msg::ToolWrite {
            path: "~/pr/rho/artifacts/factcheck_RHO_TUI_ARCHITECTURE_DECISION.md".into(),
            bytes: 14411,
        },
        Msg::Gap,
        Msg::AiMarkdown(
            "Report written to `artifacts/factcheck_RHO_TUI_ARCHITECTURE_DECISION.md`. \
             Here's the executive summary:\n\n\
             ## The architecture is viable — with 3 blockers to fix first\n\n\
             **37 claims checked: 23 confirmed, 3 refuted, 9 partial, 2 unverifiable.**\n\n\
             ### ❌ Three load-bearing errors\n\n\
             **1. ratatui-core/ratatui-widgets don't exist in the current workspace.** \
             The crate split happened at 0.30.0, not ~0.29.\n\n\
             **2. The `eye_declare` dismissal rests on a false technical claim.** \
             `eye_declare` does **not** use `insert_before`. It uses a newline-growth model. \
             The conclusion (dismiss it) is still correct but the stated reason is wrong.\n\n\
             **3. `ftui`/FrankenTUI was dismissed under a crate name that doesn't exist.** \
             The real crate is `ftui` with 7,379 downloads.\n\n\
             ### ⚠️ Key implementation gaps\n\n\
             - Synchronized output protocol completely missing (`\\x1b[?2026h/l`)\n\
             - Resize does not call `invalidate()` — width mismatch drives cache misses\n\
             - `Paragraph::line_count` requires the `unstable-rendered-line-info` feature flag"
                .into(),
        ),
        Msg::Gap,
        Msg::Human("given the error in eye_declare, should we evaluate it as an option?".into()),
        Msg::Gap,
        Msg::AiMarkdown(
            "No. The corrected mechanism doesn't change the conclusion — it just changes \
             the reason.\n\n\
             The fundamental rejection criterion for rho is: **the application must retain \
             control of all rendered content** so it can clear and re-render from scratch on \
             resize. `eye_declare` fails this regardless of mechanism:\n\n\
             - `insert_before` → content copied to terminal scrollback, app loses it\n\
             - **newline-growth** → content scrolls off, terminal owns it\n\n\
             The only scenario where `eye_declare` would be worth evaluating is if rho's \
             resize requirement were relaxed. That's a product decision, not a technical one."
                .into(),
        ),
        Msg::Gap,
        Msg::Human(
            "I cloned the repo in ~/s/eye-declare and it has a chat example that looks quite \
             similar to what we want to do/achieve. could you scan the source code and re-asses?"
                .into(),
        ),
        Msg::Gap,
        Msg::Think,
        Msg::Gap,
        Msg::ToolBash(BashBlock {
            cmd: "$ ls ~/s/eye-declare".into(),
            truncation: None,
            output: vec![
                ".git/  .github/  .gitignore  .planning/  AGENTS.md  assets/".into(),
                "Cargo.lock  Cargo.toml  CHANGELOG.md  CLAUDE.md  crates/  docs/".into(),
                "GEMINI.md  README.md  scripts/  target/".into(),
            ],
            elapsed: "0.0s".into(),
            success: true,
        }),
        Msg::Gap,
        Msg::ToolSubagent(SubagentBlock {
            label: "subagent scout".into(),
            success: true,
            lines: vec![
                AgentLine {
                    indent: 0,
                    success: true,
                    text: "scout (claude-haiku-4-5 · thinking low) · ⟳ 11 · 22 tool uses \
                           · 9.6k token · 1m37s"
                        .into(),
                },
                AgentLine {
                    indent: 2,
                    success: false,
                    text: "output: ~/.pi/agent/sessions/.../41a224be_scout_0_output.md".into(),
                },
            ],
        }),
        Msg::Gap,
        Msg::AiMarkdown(
            "Here's the honest reassessment:\n\n\
             ## eye-declare Reassessment\n\n\
             **Short answer: it's worth serious consideration.** The scrollback-reflow \
             limitation reduces to a one-line fix.\n\n\
             ### The core issue is almost trivial to fix\n\n\
             `eye_declare`'s `resize()` emits `\\x1b[2J\\x1b[H` but not `\\x1b[3J` (clear \
             scrollback). Add `\\x1b[3J` to `inline.rs:235`, and the resize path becomes:\n\n\
             1. Clear visible screen **and scrollback**\n\
             2. Reset `prev_frame = None`, `emitted_rows = 0`\n\
             3. Re-render full component tree at new width from state\n\n\
             That's exactly pi's resize behavior. **One byte sequence away.**\n\n\
             ### Recommendation\n\n\
             Build the mock app against eye-declare rather than from scratch. Add `\\x1b[3J` \
             to the resize path, keep all messages in state, and validate:\n\n\
             1. Resize reflow of a long chat history\n\
             2. Streaming assistant responses\n\
             3. A basic editor at the bottom"
                .into(),
        ),
        Msg::Gap,
        Msg::Human(
            "can you build a new example in ~/s/eye-declare called rho where you replicate \
             this session? I'll paste the text, things to take into account:\n\
             1. markdown rendering, 1 col of margin for your output, light grey for the human \
             messages, green for successful commands, red for unsuccessful,\n\
             2. note the footer and the edit section. no need to replicate any of the edit \
             behaviors, I just want to see a poc."
                .into(),
        ),
    ]
}

fn header_elements() -> Elements {
    let title_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let hint_style = Style::default().fg(Color::DarkGray);

    element! {
        Canvas(
            render_fn: move |area: Rect, buf: &mut Buffer| {
                Paragraph::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled("pi v0.75.5", title_style),
                ]))
                .render(Rect::new(area.x, area.y, area.width, 1), buf);

                Paragraph::new(Line::styled(
                    "  escape interrupt · ctrl+c/ctrl+d clear/exit \
                     · / commands · ! bash · ctrl+o more",
                    hint_style,
                ))
                .render(Rect::new(area.x, area.y + 1, area.width, 1), buf);

                Paragraph::new(Line::styled(
                    "  Press ctrl+o to show full startup help and loaded resources.",
                    hint_style,
                ))
                .render(Rect::new(area.x, area.y + 2, area.width, 1), buf);
            },
            height: 3u16
        )
    }
}

fn render_msg(msg: &Msg) -> Elements {
    match msg {
        Msg::Gap => element! {
            Canvas(render_fn: |_area, _buf| {}, height: 1u16)
        },

        Msg::Human(text) => {
            let text1 = text.clone();
            let text2 = text.clone();
            let text_style = Style::default().fg(Color::Gray);
            let bg = Color::Rgb(40, 44, 52);

            element! {
                View(
                    padding_left: Some(Cells(1)),
                    padding_right: Some(Cells(1)),
                    padding_top: Some(Cells(1)),
                    padding_bottom: Some(Cells(1)),
                    style: Style::default().bg(bg),
                ) {
                    Canvas(
                        render_fn: move |area: Rect, buf: &mut Buffer| {
                            let lines: Vec<Line> = text1
                                .lines()
                                .map(|l| Line::styled(l.to_string(), text_style))
                                .collect();
                            Paragraph::new(lines)
                                .style(Style::default().bg(bg))
                                .render(area, buf);
                        },
                        desired_height_fn: move |w: u16| {
                            let w = w.max(1) as u32;
                            let mut h = 0u32;
                            for line in text2.lines() {
                                let chars = line.chars().count() as u32;
                                h += if chars == 0 { 1 } else { chars.div_ceil(w) };
                            }
                            h.max(1).min(u16::MAX as u32) as u16
                        }
                    )
                }
            }
        }

        Msg::Think => element! {
            Spinner(label: "Thinking...")
        },

        Msg::SkillTag(name) => {
            let tag = format!("[skill] {} (ctrl+o to expand)", name);
            let style = Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);
            element! {
                Canvas(
                    render_fn: move |area: Rect, buf: &mut Buffer| {
                        Paragraph::new(Line::styled(tag.clone(), style)).render(area, buf);
                    },
                    height: 1u16
                )
            }
        }

        Msg::ToolRead(path) => {
            let line = format!(" read {}", path);
            let style = Style::default().fg(Color::DarkGray);
            element! {
                Canvas(
                    render_fn: move |area: Rect, buf: &mut Buffer| {
                        Paragraph::new(Line::styled(line.clone(), style)).render(area, buf);
                    },
                    height: 1u16
                )
            }
        }

        Msg::ToolBash(block) => render_bash(block),

        Msg::ToolSubagent(block) => render_subagent(block),

        Msg::ToolWrite { path, bytes } => {
            let line = format!(" write {}  ({} bytes written)", path, bytes);
            let style = Style::default().fg(Color::Green);
            element! {
                Canvas(
                    render_fn: move |area: Rect, buf: &mut Buffer| {
                        Paragraph::new(Line::styled(line.clone(), style)).render(area, buf);
                    },
                    height: 1u16
                )
            }
        }

        Msg::AiMarkdown(src) => {
            let src = src.clone();
            element! {
                View(padding_left: Some(Cells(1))) {
                    Markdown(source: src)
                }
            }
        }
    }
}

fn parse_cmd_line(line: &str) -> Vec<Span<'static>> {
    let prompt_style = Style::default().fg(Color::DarkGray);
    let cmd_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let arg_style = Style::default().fg(Color::Cyan);

    if let Some(rest) = line.strip_prefix("$ ") {
        let mut parts = rest.splitn(2, ' ');
        let cmd = parts.next().unwrap_or(rest);
        let args = parts.next();
        let mut spans = vec![
            Span::styled(" $ ".to_string(), prompt_style),
            Span::styled(cmd.to_string(), cmd_style),
        ];
        if let Some(a) = args {
            spans.push(Span::styled(format!(" {}", a), arg_style));
        }
        spans
    } else {
        vec![Span::styled(
            format!(" {}", line),
            Style::default().fg(Color::DarkGray),
        )]
    }
}

fn render_bash(block: &BashBlock) -> Elements {
    let out_bg = if block.success {
        Color::Rgb(0, 95, 0)
    } else {
        Color::Rgb(110, 0, 0)
    };
    let dim = Style::default().fg(Color::DarkGray);

    let cmd_lines: Vec<String> = block.cmd.lines().map(|l| l.to_string()).collect();
    let trunc = block.truncation.clone();
    let output = block.output.clone();
    let elapsed = block.elapsed.clone();
    let has_content = !output.is_empty() || trunc.is_some();

    let height: u16 = 1
        + cmd_lines.len() as u16
        + 1
        + trunc.as_ref().map_or(0u16, |_| 1)
        + output.len() as u16
        + if has_content { 1 } else { 0 }
        + 1;

    element! {
        Canvas(
            render_fn: move |area: Rect, buf: &mut Buffer| {
                let mut y = area.y + 1;

                for cmd_line in &cmd_lines {
                    if y < area.bottom() {
                        let spans = parse_cmd_line(cmd_line);
                        Paragraph::new(Line::from(spans))
                            .render(Rect::new(area.x, y, area.width, 1), buf);
                        y += 1;
                    }
                }

                y += 1;

                if let Some(ref t) = trunc
                    && y < area.bottom() {
                    Paragraph::new(Line::styled(format!(" {}", t), dim))
                        .render(Rect::new(area.x, y, area.width, 1), buf);
                    y += 1;
                }

                for out_line in &output {
                    if y < area.bottom() {
                        Paragraph::new(Line::from(format!(" {} ", out_line)))
                            .style(Style::default().fg(Color::Black).bg(out_bg))
                            .render(Rect::new(area.x, y, area.width, 1), buf);
                        y += 1;
                    }
                }

                if has_content {
                    y += 1;
                }

                if y < area.bottom() {
                    Paragraph::new(Line::styled(format!(" Took {}", elapsed), dim))
                        .render(Rect::new(area.x, y, area.width, 1), buf);
                }
            },
            height: height
        )
    }
}

fn render_subagent(block: &SubagentBlock) -> Elements {
    let label = block.label.clone();
    let lines = block.lines.clone();
    let success = block.success;

    let label_style = if success {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };
    let dim = Style::default().fg(Color::DarkGray);

    let height: u16 = 1 + lines.len() as u16;

    element! {
        Canvas(
            render_fn: move |area: Rect, buf: &mut Buffer| {
                let prefix = if success { "✓ " } else { "✗ " };
                Paragraph::new(Line::from(vec![
                    Span::styled(prefix, label_style),
                    Span::styled(label.clone(), label_style),
                ]))
                .render(Rect::new(area.x, area.y, area.width, 1), buf);

                for (i, al) in lines.iter().enumerate() {
                    let y = area.y + 1 + i as u16;
                    if y < area.bottom() {
                        let indent = " ".repeat(al.indent + 2);
                        let (pfx, pfx_style, text_style) = if al.success {
                            (
                                "✓ ",
                                Style::default().fg(Color::Green),
                                Style::default().fg(Color::Green),
                            )
                        } else {
                            ("  ", dim, dim)
                        };
                        Paragraph::new(Line::from(vec![
                            Span::raw(indent),
                            Span::styled(pfx, pfx_style),
                            Span::styled(al.text.clone(), text_style),
                        ]))
                        .render(Rect::new(area.x, y, area.width, 1), buf);
                    }
                }
            },
            height: height
        )
    }
}

fn footer_elements() -> Elements {
    let dim = Style::default().fg(Color::DarkGray);
    let branch_style = Style::default().fg(Color::Blue);
    let model_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);

    element! {
        Canvas(
            render_fn: move |area: Rect, buf: &mut Buffer| {
                let sep: String = "─".repeat(area.width as usize);
                Paragraph::new(Line::styled(sep, dim))
                    .render(Rect::new(area.x, area.y, area.width, 1), buf);

                Paragraph::new(Line::from(vec![
                    Span::styled("~/pr/rho", dim),
                    Span::styled(" (main)", branch_style),
                    Span::styled(
                        " ↑20 ↓17k R521k W127k $0.894 6.6%/1.0M (auto) | cache:5m",
                        dim,
                    ),
                    Span::styled("   claude-sonnet-4-6 • medium", model_style),
                ]))
                .render(Rect::new(area.x, area.y + 1, area.width, 1), buf);
            },
            height: 2u16
        )
    }
}

fn editor_elements() -> Elements {
    let placeholder_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);

    element! {
        View(
            border: BorderType::Plain,
            border_style: Style::default().fg(Color::DarkGray),
            padding_left: Some(Cells(1)),
            padding_right: Some(Cells(1)),
        ) {
            Canvas(
                render_fn: move |area: Rect, buf: &mut Buffer| {
                    Paragraph::new(Line::styled("Type a message...", placeholder_style))
                        .render(area, buf);
                },
                height: 1u16
            )
        }
    }
}

fn view(state: &AppState) -> Elements {
    element! {
        #(header_elements())

        #(for msg in &state.messages {
            #(render_msg(msg))
        })

        #(footer_elements())
        #(editor_elements())
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let (mut app, _handle) = Application::builder()
        .state(AppState {
            messages: session(),
        })
        .view(view)
        .build()?;

    app.update(|_| {});
    app.flush(&mut io::stdout())?;

    app.run_interactive(move |event, _state| {
        if let Event::Key(KeyEvent {
            code: KeyCode::Esc | KeyCode::Char('q'),
            kind: KeyEventKind::Press,
            ..
        }) = event
        {
            return ControlFlow::Exit;
        }
        ControlFlow::Continue
    })
    .await
}
