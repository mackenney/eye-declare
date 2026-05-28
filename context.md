# eye-declare Scout Report

## Overview

eye-declare is a **declarative inline TUI library** for Rust, designed specifically for inline rendering (content grows into scrollback, not full-screen). Built on ratatui-core and crossterm, it provides a React-like component model with retained state, automatic dirty tracking, and inline ANSI rendering with line-level diffs.

**Key positioning:** Designed for CLI tools, AI assistants, and interactive prompts where output accumulates and persistence is important.

---

## Files Retrieved

1. `crates/eye_declare/Cargo.toml` – deps: ratatui-core, ratatui-widgets, crossterm, tokio
2. `crates/eye_declare/examples/chat.rs` – interactive chat with streaming, input, markdown
3. `crates/eye_declare/src/app.rs` (lines 1–1482) – Application, Handle, event loop, resize handling
4. `crates/eye_declare/src/component.rs` (lines 1–526) – Component trait, Tracked<S>, VStack/HStack
5. `crates/eye_declare/src/inline.rs` (lines 1–1264) – InlineRenderer, resize, scrollback commit detection
6. `crates/eye_declare/src/renderer.rs` (lines 1–200) – Renderer struct, tree management, layout
7. `crates/eye_declare/src/frame.rs` (lines 1–227) – Frame, Diff, cell-level diffing
8. `crates/eye_declare/src/components/text.rs` (lines 1–100) – Text, Span, word wrap
9. `crates/eye_declare/src/components/markdown.rs` (lines 1–100) – Markdown with styling
10. `README.md` – full architecture, API, examples

---

## Key Code

### 1. Resize Behavior (inline.rs:223–247)

```rust
pub fn resize(&mut self, new_width: u16) -> Vec<u8> {
    let mut output = Vec::new();
    
    // Clear visible screen and home cursor.
    // \x1b[2J = clear entire screen
    // \x1b[H  = cursor to row 1, col 1 (home)
    // This does NOT clear scrollback (\x1b[3J would do that).
    output.extend_from_slice(b"\x1b[2J\x1b[H");
    
    // Reset internal state
    self.renderer.set_width(new_width);
    self.cursor = CursorState::new();
    self.prev_frame = None;
    self.emitted_rows = 0;
    // Update terminal height (resize event gives us width, query for height)
    if let Ok((_, h)) = crossterm::terminal::size() {
        self.terminal_height = h;
    }
    
    // Do a fresh render
    let render_output = self.render();
    output.extend_from_slice(&render_output);
    
    output
}
```

**Called from:** `app.rs` interactive_loop (line 792–796)

```rust
if let Event::Resize(new_width, _) = &evt {
    let output = self.inline.resize(*new_width);
    stdout.write_all(&output)?;
    stdout.flush()?;
    self.dirty = true;
}
```

**Behavior:**
- Clears the visible screen only (`\x1b[2J`, NOT `\x1b[3J` which would clear scrollback)
- Homes cursor (`\x1b[H`)
- Resets internal frame tracking (`prev_frame = None`, `emitted_rows = 0`)
- Updates renderer width via `set_width()`
- Queries terminal height
- Performs a fresh render at the new width

**Critical gap for rho-tui:** Scrollback content from before the resize stays at the old width/wrapping. The visible content re-renders at the new width, but old scrollback is not reflowed. This is a **documented fallback** (README, lines 217–220).

---

### 2. Scrollback Model (inline.rs:422–453, app.rs:902–942)

**How content leaves app control:**

Content stays in app state until explicitly evicted via `on_commit` callback. The framework detects when rows have fully scrolled into terminal scrollback (past terminal height) and fires the callback.

```rust
// From inline.rs:427–453
pub fn detect_committed(
    &self,
    container: NodeId,
    terminal_height: u16,
) -> Vec<(usize, Option<String>)> {
    let scrollback_rows = self.emitted_rows.saturating_sub(terminal_height);
    if scrollback_rows == 0 {
        return Vec::new();
    }
    
    let children = self.renderer.children(container);
    let mut accumulated: u16 = 0;
    let mut committed = Vec::new();
    
    for (i, &child_id) in children.iter().enumerate() {
        let child_height = self.renderer.node_last_height(child_id);
        accumulated = accumulated.saturating_add(child_height);
        if accumulated <= scrollback_rows {
            let key = self.renderer.node_key(child_id).map(|s| s.to_string());
            committed.push((i, key));
        } else {
            break;
        }
    }
    
    committed
}
```

**From app.rs (lines 909–942):**

```rust
fn check_commits_with_height(&mut self, terminal_height: u16) {
    if self.on_commit.is_none() {
        return;
    }
    
    let committed = self.inline.detect_committed(self.container, terminal_height);
    if committed.is_empty() {
        return;
    }
    
    // Calculate total committed height
    let children = self.inline.children(self.container);
    let mut committed_height: u16 = 0;
    for &(i, _) in &committed {
        committed_height += self.inline.node_last_height(children[i]);
    }
    
    // Fire callbacks
    let on_commit = self.on_commit.as_mut().unwrap();
    for (index, key) in &committed {
        let elem = CommittedElement {
            key: key.clone(),
            index: *index,
        };
        on_commit(&elem, &mut self.state);
    }
    self.dirty = true;
    
    // Drop committed nodes and adjust frame tracking
    self.inline.commit(self.container, committed.len(), committed_height);
}
```

**When the callback fires:**
- After every `flush()` (app.rs:591–599)
- After every `render()` in interactive loops (app.rs:716, 844)

**What `on_commit` callback receives:**
- `CommittedElement { key: Option<String>, index: usize }` identifies which element scrolled off
- Mutable reference to app state for eviction

**Example from chat.rs (lines 285–287):**
```rust
.on_commit(|_, state: &mut AppState| {
    state.messages.remove(0);
})
```

---

### 3. Chat Example (examples/chat.rs)

**Structure:**
- `AppState`: messages (Vec<ChatMessage>), input text, cursor position, next_id counter
- `ChatMessage`: id, kind (User(text) or Assistant { content, done })
- `InputBox` component: bordered text input with focus/cursor support
- `StreamingDots` component: animated "...", "..", "." while streaming
- `chat_view()` function: renders all messages + input box

**Key features:**
- Interactive raw-mode event handling (Enter = send, Backspace = delete, Arrow keys = cursor)
- Streaming simulation: spawns async task that updates assistant message word-by-word via `handle.update()`
- Uses `on_commit` to evict old messages
- Markdown rendering for assistant messages (done or streaming with ▌ cursor)
- Demonstrates: events, custom components, streaming via Handle, committed scrollback

**Does it look like a coding agent UI?**
- Yes: chat history, interactive input, streaming responses, markdown support
- But: no code blocks with syntax highlighting beyond markdown, no side-by-side panels

---

### 4. Rendering Pipeline (inline.rs:254–371, frame.rs:48–102)

**Path:** `render()` → Frame diff → escape sequences → stdout

```rust
// From inline.rs:249–371
pub fn render(&mut self) -> Vec<u8> {
    let new_frame = self.renderer.render();  // Produces ratatui Buffer
    let new_height = new_frame.area().height;
    
    // First render: diff against empty frame
    if self.prev_frame.is_none() {
        let empty = Frame::new(Buffer::empty(Rect::new(0, 0, width, 0)));
        let mut diff = new_frame.diff(&empty);
        
        // Stream rows that would scroll off
        self.stream_rows_into_scrollback(&new_frame, 0, stream_until, &mut output);
        
        // Emit newlines to claim rows
        // Filter out cells in scrollback
        diff.retain_visible(scrolled_past);
        
        let escape_bytes = diff.to_escape_sequences(&mut self.cursor);
        output.extend_from_slice(&escape_bytes);
        
        self.append_cursor_position(&mut output);
        self.prev_frame = Some(new_frame);
        return output;
    }
    
    // Subsequent renders: full diff
    let prev = self.prev_frame.as_ref().unwrap();
    let mut diff = new_frame.diff(prev);
    
    // Check for growth, stream scrollback rows if needed
    // Emit newlines if frame grew
    // Filter out cells in scrollback
    let escape_bytes = diff.to_escape_sequences(&mut self.cursor);
    output.extend_from_slice(&escape_bytes);
    
    self.append_cursor_position(&mut output);
    self.prev_frame = Some(new_frame);
    output
}
```

**Frame.diff (frame.rs:48–102):**
- Compares two frames cell-by-cell
- Handles height mismatches by padding with empty cells
- Returns `Diff { cells: Vec<(x, y, cell)>, new_area, prev_area }`

**Output format:**
- ANSI escape sequences: cursor movement, styling (colors, bold), cell content
- DEC synchronized output brackets (`\x1b[?2026h` / `\x1b[?2026l`) to prevent tearing
- Only changed cells are emitted (line-level differential rendering)

**Does it produce ANSI strings?** Yes, but as `Vec<u8>` (bytes). Caller writes to stdout.

---

### 5. Component Model (component.rs:221–399)

**Trait definition:**
```rust
pub trait Component: Send + Sync + 'static {
    type State: Send + Sync + Default + 'static;
    
    fn render(&self, area: Rect, buf: &mut Buffer, state: &Self::State) {}
    fn desired_height(&self, width: u16, state: &Self::State) -> Option<u16> { None }
    fn handle_event_capture(&self, event: &Event, state: &mut Tracked<Self::State>) -> EventResult { Ignored }
    fn handle_event(&self, event: &Event, state: &mut Tracked<Self::State>) -> EventResult { Ignored }
    fn is_focusable(&self, state: &Self::State) -> bool { false }
    fn cursor_position(&self, area: Rect, state: &Self::State) -> Option<(u16, u16)> { None }
    fn initial_state(&self) -> Option<Self::State> { None }
    fn content_inset(&self, state: &Self::State) -> Insets { ZERO }
    fn layout(&self) -> Layout { default() }
    fn width_constraint(&self) -> WidthConstraint { default() }
    fn lifecycle(&self, hooks: &mut Hooks<Self, Self::State>, state: &Self::State) {}
    fn view(&self, state: &Self::State, children: Elements) -> Elements { children }
    
    fn update(
        &self,
        hooks: &mut Hooks<Self, Self::State>,
        state: &Self::State,
        children: Elements,
    ) -> Elements {
        self.lifecycle(hooks, state);
        self.view(state, children)
    }
}
```

**Retained model:**
- State is `Tracked<S>` (automatic dirty detection via `DerefMut`)
- Components define props as struct fields (immutable, set by parent)
- State is mutable, cached across rebuilds
- Reconciliation preserves state when nodes are reused (by key or position)

**React-like aspect:**
- `element!` macro builds virtual trees
- Keyed children survive reordering
- Props + children model
- Lifecycle hooks (mount, unmount, interval, context)
- Dirty tracking triggers re-renders

**#[component] macro (eye_declare_macros):**
- Generates `impl Component` from a function
- Signature: `fn my_component(props: &Props, hooks: &mut Hooks<Props, State>, children: Elements) -> Elements`
- Or with #[props]: `#[props] struct Props { ... }` + `#[component(props = Props)]`

**Example from chat.rs (lines 83–130):**
```rust
#[props]
struct InputBox {
    text: String,
    #[default(0usize)]
    cursor: usize,
    #[default("".to_string())]
    prompt: String,
}

#[component(props = InputBox)]
fn input_box(props: &InputBox, hooks: &mut Hooks<InputBox, ()>) -> Elements {
    hooks.use_autofocus();
    hooks.use_focusable(true);
    
    let cursor_pos = props.cursor;
    hooks.use_cursor(move |area: Rect, _props: &InputBox, _state: &()| {
        let col = 2 + cursor_pos as u16;
        if col < area.width.saturating_sub(1) {
            Some((col, 1))
        } else {
            Some((area.width.saturating_sub(2), 1))
        }
    });
    
    element! {
        View(
            border: BorderType::Plain,
            border_style: Style::default().fg(Color::DarkGray),
            title: format!(" {} ", props.prompt),
            title_style: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            padding_left: Some(Cells(1)),
            padding_right: Some(Cells(1)),
        ) {
            Canvas(render_fn: move |area: Rect, buf: &mut Buffer| {
                if area.width == 0 || area.height == 0 {
                    return;
                }
                let display = if text.is_empty() {
                    Line::from(/* placeholder */)
                } else {
                    Line::from(/* display text */)
                };
                Paragraph::new(display).render(area, buf);
            }, height: 1u16)
        }
    }
}
```

---

### 6. Built-in Components

| Component | Purpose | Notes |
|-----------|---------|-------|
| `Text` | Styled text with word wrap | Accepts Span children + strings |
| `Markdown` | Headings, bold, italic, code, lists | MarkdownState holds styling |
| `Spinner` | Animated Braille spinner + done checkmark | Auto-tick, animates in loop |
| `Canvas` | Raw buffer rendering | User-provided closure, direct Buffer access |
| `View` | Layout container with borders, padding | Unified chrome container |
| `VStack` | Vertical layout (default) | Tallest layout mode |
| `HStack` | Horizontal layout | Requires Column children for width control |
| `Column` | Width constraint wrapper | Use inside HStack with Fixed/Fill/Flex |

---

### 7. Markdown Support (components/markdown.rs)

**Features:**
- `#`, `##`, `###` headings
- `**bold**`, `*italic*`, `` `inline code` ``
- Fenced code blocks (` ``` `)
- Unordered lists (`-`, `*`)

**State:**
- `MarkdownState` holds color/style config (cyan headings, yellow code, green blocks, etc.)
- Customizable via field overwrites

**Example from chat.rs (line 198):**
```rust
MessageKind::Assistant { content, done } => {
    if *done {
        element! { Markdown(key: key, source: content.clone()) }
    } else if content.is_empty() {
        element! { StreamingDots(key: key) }
    } else {
        element! { Markdown(key: key, source: format!("{}▌", content)) }
    }
}
```

---

### 8. ratatui Integration (Cargo.toml)

```toml
[dependencies]
ratatui-core = "0.1"
ratatui-widgets = { version = "0.3", features = ["unstable-rendered-line-info"] }
crossterm = { version = "0.29", features = ["event-stream"] }
```

**What's used:**
- **ratatui-core:** `Buffer` (2D cell grid), `Cell` (char + style), `Rect` (layout region), `Style`, `Text`, `Line`, `Span`
- **ratatui-widgets:** `Paragraph` widget (text rendering with wrapping)
- **crossterm:** Terminal I/O, event streams, raw mode, cursor control

**Note:** eye-declare uses ratatui for *rendering primitives* (Buffer, Cell, styling) but not for full-screen terminal management. The rendering loop is custom (inline, not alternate-screen).

---

## Architecture

### High-level flow

```
Application (app.rs)
  ├─ State: S (user-provided)
  ├─ View fn: &S → Elements
  └─ InlineRenderer
      ├─ Renderer (tree of Components)
      │   ├─ NodeArena (component tree + state)
      │   ├─ Layout engine (width/height measurement)
      │   ├─ Event dispatch (capture + bubble)
      │   └─ Effect management (intervals, mount/unmount)
      ├─ Frame tracking (previous frame, diff)
      ├─ Cursor state (position, column)
      └─ Scrollback tracking (emitted_rows, terminal_height, committed rows)
```

### Render cycle

1. **Rebuild:** Call view fn → produce Elements → reconcile against tree
2. **Layout:** Measure widths/heights, resolve WidthConstraints
3. **Render:** Call `Component::render()` for dirty nodes → produce Buffer
4. **Diff:** Compare current Frame against previous Frame
5. **Escape:** Convert diff to ANSI escape sequences
6. **Output:** Write to stdout (with DEC synchronized output)
7. **Commit check:** Detect rows in scrollback, fire `on_commit` callback, evict from state

### Key data structures

- **Node (renderer.rs):** Component + state + children + layout metadata
- **Element (element.rs):** Virtual element descriptor (component type + props + key + children)
- **Frame (frame.rs):** Rendered output (owns ratatui Buffer)
- **Diff (frame.rs):** Changed cells between frames

---

## Start Here

For understanding eye-declare's core behavior:

1. **`src/app.rs`** (lines 792–796, 909–942) – See resize and commit detection in context of the interactive event loop
2. **`src/inline.rs`** (lines 223–247) – Resize implementation and why scrollback isn't reflowed
3. **`examples/chat.rs`** – Full working app demonstrating interactive input, streaming, markdown, commits
4. **`src/component.rs`** – Component trait and retained state model
5. **`src/frame.rs` + escape.rs** – Frame diffing and ANSI output generation

For building on top:
- **Reconciliation logic:** `renderer.rs` (tree management, key-based matching)
- **Custom components:** Implement `Component` trait or use `#[component]` macro
- **Hooks system:** `hooks.rs` (lifecycle, intervals, context, focus)

---

## Supervisor Coordination

### Questions for the developer (rho-tui use case)

1. **Full history re-render on resize:** The current `resize()` clears the visible screen but doesn't reflow old scrollback. For rho-tui to meet the requirement of "re-render entire history at new width," the app needs to:
   - Track all message data in state (not rely on terminal scrollback)
   - On resize, rebuild the full view from state
   - Clear + re-render (eye-declare can do this, but the app must manage history)

   **Is this acceptable?** Or does rho-tui need to keep scrollback in the terminal itself and avoid re-rendering?

2. **Committed scrollback ergonomics:** The `on_commit` callback is called *after* content has already scrolled. If rho-tui needs to track exactly when content leaves the visible area for other reasons (e.g., logging), the timing might be slightly delayed.

3. **Markdown limitations:** eye-declare's Markdown lacks syntax highlighting for code blocks. For a coding agent UI, custom code block component (with language detection + pygments/syntect) would be needed.

4. **Performance at scale:** At 10k+ messages, reconciliation cost grows linearly. For very long chat histories, consider:
   - Keying every message to preserve component state
   - Using `on_commit` to evict old state from memory
   - Freezing old components (skip re-renders)

---

## Practical Assessment for rho-tui

### What Works

✅ **Inline rendering model** – Content flows to scrollback, not full-screen. Exactly what rho-tui needs.

✅ **Retained component tree** – State survives across rebuilds, animations continue, focus is managed.

✅ **Automatic dirty tracking** – Changes trigger minimal re-renders via `Tracked<S>`.

✅ **Line-level diffs** – Only changed cells are written (efficient on large windows).

✅ **Streaming support** – Handle-based updates from async tasks, perfect for LLM responses.

✅ **Markdown rendering** – Built-in, good for chat output (though lacks code syntax highlighting).

✅ **Interactive input** – Focus, Tab cycling, text input, cursor positioning.

✅ **Event dispatch** – Two-phase (capture + bubble), component-driven.

### Gaps / Challenges

❌ **Resize doesn't reflow old scrollback** – Scrollback stays at old width. To meet rho's requirement of "clear screen + re-render entire history at new width," the app must:
- Keep all message data in state (not rely on terminal scrollback persistence)
- Re-build the full view on resize
- Accept the visual clear

**Workaround:** Track messages in state, rebuild on resize (accept the screen clear as acceptable).

❌ **No syntax highlighting for code blocks** – Markdown component uses basic colors for code blocks, not language-specific highlighting. Would need custom Canvas-based code component.

❌ **No built-in multi-pane layout** – HStack exists but no sidebar/context-pane helpers. Building a sidebar with scrolling context would require custom layout.

❌ **Terminal protocol limitations** – Uses crossterm for events (covers Kitty, WezTerm, but not all features of modern protocols).

### Recommendation

**eye-declare is a good fit for rho-tui** if:
1. You accept clearing the screen on terminal resize (or avoid resize handling)
2. You implement syntax highlighting separately (e.g., Canvas + syntect)
3. You keep all chat data in app state (no reliance on terminal scrollback as a cache)

**Alternative consideration:** If rho-tui needs to preserve scrollback wrapping on resize without clearing, you'd need a custom renderer that:
- Tracks consumed rows at each width
- Reflows wrapped text on width change
- Emits diffs for changed rows only

This is more complex but feasible by extending InlineRenderer's resize logic.

---

## Key Files & Line Ranges

| File | Lines | Purpose |
|------|-------|---------|
| `examples/chat.rs` | 1–353 | Full interactive chat example |
| `src/app.rs` | 792–796 | Resize event handling |
| `src/app.rs` | 909–942 | Commit detection & callback |
| `src/inline.rs` | 210–247 | Resize implementation |
| `src/inline.rs` | 422–453 | Scrollback detection |
| `src/inline.rs` | 460–488 | Commit (node removal & frame adjustment) |
| `src/frame.rs` | 48–102 | Frame diffing algorithm |
| `src/component.rs` | 221–399 | Component trait, Tracked<S> |
| `Cargo.toml` | 1–33 | Dependencies |
| `README.md` | 1–503 | Full architecture & API |
