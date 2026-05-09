use crate::scripting::guiwin::GuiWin;
use crate::termwindow::{RepaintStats, TermWindowNotif};
use chrono::prelude::*;
use futures::FutureExt;
use log::Level;
use luahelper::ValuePrinter;
use mlua::Value;
use mux::termwiztermtab::TermWizTerminal;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use termwiz::cell::{AttributeChange, CellAttributes, Intensity};
use termwiz::color::AnsiColor;
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers};
use termwiz::lineedit::*;
use termwiz::surface::{Change, Position};
use termwiz::terminal::Terminal;
use window::WindowOps;

lazy_static::lazy_static! {
    static ref LATEST_LOG_ENTRY: Mutex<Option<DateTime<Local>>> = Mutex::new(None);
}

fn repaint_stats(gui_win: &GuiWin) -> anyhow::Result<RepaintStats> {
    let (tx, rx) = smol::channel::bounded(1);
    gui_win.window.notify(TermWindowNotif::GetRepaintStats(tx));
    Ok(futures::executor::block_on(rx.recv())?)
}

fn format_ms_ago(value: Option<u64>) -> String {
    value
        .map(|value| format!("{value}ms ago"))
        .unwrap_or_else(|| "never".to_string())
}

pub fn show_repaint_debug_overlay(
    mut term: TermWizTerminal,
    gui_win: GuiWin,
) -> anyhow::Result<()> {
    term.no_grab_mouse_in_raw_mode();

    loop {
        let stats = repaint_stats(&gui_win)?;
        term.render(&[
            Change::Title("Repaint Debug".to_string()),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
            Change::ClearScreen(Default::default()),
            Change::Text(format!(
                "Repaint Debug Overlay\r\n\
                 Press ESC or CTRL-D to exit. Refreshes once per second.\r\n\r\n\
                 Window: {}    Workspace: {}    Renderer: {}\r\n\
                 FPS: {:.1}    Last frame: {:.2}ms\r\n\r\n\
                 NeedRepaint count: {}    last: {}\r\n\
                 Paint count:       {}    started: {}    finished: {}\r\n\
                 Present ok/fail:   {}/{}    last present: {}\r\n\r\n\
                 Invalidates since last paint: {}\r\n\
                 resizes_pending: {}    is_repaint_pending: {}\r\n\r\n\
                 If NeedRepaint stops moving while output is expected, look below TermWindow.\r\n\
                 If paint moves but present does not, look at renderer/window presentation.\r\n",
                stats.mux_window_id,
                stats.active_workspace,
                stats.renderer,
                stats.fps,
                stats.last_frame_duration_ms,
                stats.need_repaint_count,
                format_ms_ago(stats.last_need_repaint_ms_ago),
                stats.paint_count,
                format_ms_ago(stats.last_paint_started_ms_ago),
                format_ms_ago(stats.last_paint_finished_ms_ago),
                stats.successful_present_count,
                stats.failed_present_count,
                format_ms_ago(stats.last_present_ms_ago),
                stats.invalidates_since_last_paint,
                stats.resizes_pending,
                stats.is_repaint_pending,
            )),
        ])?;
        term.flush()?;

        match term.poll_input(Some(std::time::Duration::from_secs(1)))? {
            Some(InputEvent::Key(KeyEvent {
                key: KeyCode::Escape,
                ..
            })) => break,
            Some(InputEvent::Key(KeyEvent {
                key: KeyCode::Char('D'),
                modifiers,
            })) if modifiers.contains(Modifiers::CTRL) => break,
            _ => {}
        }
    }

    Ok(())
}

struct LuaReplHost {
    history: BasicHistory,
    lua: mlua::Lua,
}

fn history_file_name() -> PathBuf {
    config::DATA_DIR.join("repl-history")
}

impl LuaReplHost {
    fn new(lua: mlua::Lua) -> Self {
        let mut history = BasicHistory::default();
        if let Ok(data) = std::fs::read_to_string(history_file_name()) {
            for line in data.lines() {
                history.add(line);
            }
        }
        Self { history, lua }
    }

    fn add_history(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }

        if let Some(last) = self.history.last() {
            if self.history.get(last).as_deref() == Some(line) {
                // Don't add duplicate lines
                return;
            }
        }
        self.history.add(line);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(history_file_name())
        {
            writeln!(file, "{}", line).ok();
        }
    }
}

fn format_lua_err(err: mlua::Error) -> String {
    match err {
        mlua::Error::SyntaxError {
            incomplete_input: true,
            ..
        } => "...".to_string(),
        _ => format!("{:#}", err),
    }
}

fn fragment_to_expr_or_statement(lua: &mlua::Lua, text: &str) -> Result<String, String> {
    let expr = format!("return {};", text);

    let chunk = lua.load(&expr).set_name("=repl");
    match chunk.into_function() {
        Ok(_) => {
            // It's an expression
            Ok(text.to_string())
        }
        Err(_) => {
            // Try instead as a statement
            let chunk = lua.load(text).set_name("=repl");
            match chunk.into_function() {
                Ok(_) => Ok(text.to_string()),
                Err(err) => Err(format_lua_err(err)),
            }
        }
    }
}

impl LineEditorHost for LuaReplHost {
    fn history(&mut self) -> &mut dyn History {
        &mut self.history
    }

    fn resolve_action(
        &mut self,
        event: &InputEvent,
        editor: &mut LineEditor<'_>,
    ) -> Option<Action> {
        let (line, _cursor) = editor.get_line_and_cursor();
        if line.is_empty()
            && matches!(
                event,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                })
            )
        {
            Some(Action::Cancel)
        } else {
            None
        }
    }

    fn render_preview(&self, line: &str) -> Vec<OutputElement> {
        let mut preview = vec![];

        if let Err(err) = fragment_to_expr_or_statement(&self.lua, line) {
            preview.push(OutputElement::Text(err))
        }

        preview
    }
}

pub fn show_debug_overlay(
    mut term: TermWizTerminal,
    gui_win: GuiWin,
    opengl_info: String,
    connection_info: String,
) -> anyhow::Result<()> {
    term.no_grab_mouse_in_raw_mode();

    let config::LoadedConfig { lua, .. } = config::Config::load();
    // Try hard to fall back to some kind of working lua context even
    // if the user's config file is temporarily out of whack
    let lua = match lua {
        Some(lua) => lua,
        None => match config::Config::try_default() {
            Ok(config::LoadedConfig { lua: Some(lua), .. }) => lua,
            _ => config::lua::make_lua_context(std::path::Path::new(""))?,
        },
    };

    lua.load("wezterm = require 'wezterm'").exec()?;
    lua.globals().set("window", gui_win.clone())?;
    let lua_version: String = lua.globals().get("_VERSION")?;

    let mut host = Some(LuaReplHost::new(lua));

    term.render(&[Change::Title("Debug".to_string())])?;

    fn print_new_log_entries(term: &mut TermWizTerminal) -> termwiz::Result<()> {
        let entries = env_bootstrap::ringlog::get_entries();
        let mut changes = vec![];
        for entry in entries {
            if let Some(latest) = LATEST_LOG_ENTRY.lock().unwrap().as_ref() {
                if entry.then <= *latest {
                    // already seen this one
                    continue;
                }
            }
            LATEST_LOG_ENTRY.lock().unwrap().replace(entry.then);

            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text(entry.then.format("%H:%M:%S%.3f ").to_string()));

            changes.push(
                AttributeChange::Foreground(match entry.level {
                    Level::Error => AnsiColor::Maroon.into(),
                    Level::Warn => AnsiColor::Red.into(),
                    Level::Info => AnsiColor::Green.into(),
                    Level::Debug => AnsiColor::Blue.into(),
                    Level::Trace => AnsiColor::Fuchsia.into(),
                })
                .into(),
            );
            changes.push(Change::Text(entry.level.as_str().to_string()));
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(AttributeChange::Intensity(Intensity::Bold).into());
            changes.push(Change::Text(format!(" {}", entry.target)));
            changes.push(Change::AllAttributes(CellAttributes::default()));
            changes.push(Change::Text(format!(
                " > {}\r\n",
                entry.msg.replace("\n", "\r\n")
            )));
        }
        term.render(&changes)
    }

    let version = config::wezterm_version();
    let triple = config::wezterm_target_triple();
    let stats = repaint_stats(&gui_win)?;

    term.render(&[Change::Text(format!(
        "Debug Overlay\r\n\
         wezterm version: {version} {triple}\r\n\
         Window Environment: {connection_info}\r\n\
         Lua Version: {lua_version}\r\n\
         {opengl_info}\r\n\
         Repaint: fps={:.1}, last_frame={:.2}ms, NeedRepaint={} ({}), paint={} ({}), present={}/{} ({})\r\n\
         Repaint details: invalidates_since_last_paint={}, resizes_pending={}, is_repaint_pending={}\r\n\
         Evaluate window:repaint_stats() to refresh repaint diagnostics.\r\n\
         Enter lua statements or expressions and hit Enter.\r\n\
         Press ESC or CTRL-D to exit\r\n",
        stats.fps,
        stats.last_frame_duration_ms,
        stats.need_repaint_count,
        format_ms_ago(stats.last_need_repaint_ms_ago),
        stats.paint_count,
        format_ms_ago(stats.last_paint_finished_ms_ago),
        stats.successful_present_count,
        stats.failed_present_count,
        format_ms_ago(stats.last_present_ms_ago),
        stats.invalidates_since_last_paint,
        stats.resizes_pending,
        stats.is_repaint_pending,
    ))])?;

    loop {
        print_new_log_entries(&mut term)?;
        let mut editor = LineEditor::new(&mut term);
        editor.set_prompt("> ");
        if let Some(line) = editor.read_line(host.as_mut().unwrap())? {
            if line.is_empty() {
                continue;
            }
            host.as_mut().unwrap().add_history(&line);

            let passed_host = host.take().unwrap();

            let (host_res, text) =
                smol::block_on(promise::spawn::spawn_into_main_thread(async move {
                    evaluate_trampoline(passed_host, line)
                        .recv()
                        .await
                        .map_err(|e| mlua::Error::external(format!("{:#}", e)))
                        .expect("returning result not to fail")
                }));

            host.replace(host_res);

            if text != "nil" {
                term.render(&[Change::Text(format!("{}\r\n", text.replace("\n", "\r\n")))])?;
            }
        } else {
            return Ok(());
        }
    }
}

// A bit of indirection because spawn_into_main_thread wants the
// overall future to be Send but mlua::Value, mlua::Chunk are not
// Send.  We need to split off the actual evaluation future to
// run separately, so we spawn it and use a channel to funnel
// the result back to the caller without blocking the gui thread.
fn evaluate_trampoline(
    host: LuaReplHost,
    expr: String,
) -> smol::channel::Receiver<(LuaReplHost, String)> {
    let (tx, rx) = smol::channel::bounded(1);
    promise::spawn::spawn(async move {
        let _ = tx.send(evaluate(host, expr).await).await;
    })
    .detach();
    rx
}

async fn evaluate(host: LuaReplHost, expr: String) -> (LuaReplHost, String) {
    async fn do_it(host: &LuaReplHost, expr: &str) -> String {
        let code = match fragment_to_expr_or_statement(&host.lua, expr) {
            Ok(code) => code,
            Err(err) => return err,
        };
        let chunk = host.lua.load(&code).set_name("repl");

        let result = chunk
            .eval_async::<Value>()
            .map(|result| match result {
                Ok(result) => {
                    let value = ValuePrinter(result);
                    format!("{:#?}", value)
                }
                Err(err) => format_lua_err(err),
            })
            .await;

        result
    }

    let result = do_it(&host, &expr).await;
    (host, result)
}
