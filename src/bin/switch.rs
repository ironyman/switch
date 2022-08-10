#![allow(dead_code)]
use crossterm::{
    event::{self, EnableMouseCapture, Event, KeyCode, KeyModifiers, DisableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    style::{Color, Modifier, Style},
    text::{Spans},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame, Terminal,
};
use clap::{Arg, Command};

use switch::{
    ListContentProvider,
    WindowProvider,
    StartAppsProvider,
    console,
};

#[allow(unused_imports)]
use switch::log::*;

const INPUT_PROMPT: &str = "> ";
const WORD_BOUNDARIES: &str = " ;:\\/-=";

struct InputLine {
    buffer: Vec<char>,
    pos: isize, // maintain 0 <= self.pos <= self.len()
}

impl Default for InputLine {
    fn default() -> Self {
        return InputLine { buffer: vec![], pos: 0 };
    }
}

impl From<&InputLine> for String {
    fn from(il: &InputLine) -> Self {
        return il.buffer.iter().cloned().collect::<String>();
    }
}

impl InputLine {
    fn clear(&mut self) {
        self.buffer.clear();
        self.pos = 0;
    }

    fn push(&mut self, ch: char) {
        self.buffer.push(ch);
    }

    fn insert(&mut self, ch: char) {
        self._insert_at(self.pos as usize, ch);
        self.pos += 1;
    }

    fn backward_remove_cursor(&mut self) {
        // precondition: 0 <= self.pos <= self.len()
        if self.pos > 0 {
            self.pos -= 1;
            self.buffer.remove(self.pos as usize);
        }
    }

    fn forward_remove_cursor(&mut self) {
        // precondition: 0 <= self.pos <= self.len()
        if self.pos < self.len() as isize {
            self.buffer.remove(self.pos as usize);
        }
    }

    fn backward_find_word(&self) -> usize {
        let mut past_boundary = 0;
        
        for i in (0 .. self.pos).rev() {
            if !WORD_BOUNDARIES.contains(self.buffer[i as usize]) {
                past_boundary = i;
                break;
            }
        }

        for i in (0 .. past_boundary).rev() {
            if WORD_BOUNDARIES.contains(self.buffer[i as usize]) {
                return (i + 1) as usize;
            }
        }

        return 0usize;
    }

    fn backward_kill_word(&mut self) {
        let i = self.backward_find_word();
        self.buffer.splice(i .. self.cursor_pos(), "".chars());
        self.pos = i as isize;
    }

    fn forward_find_word(&self) -> usize {
        let mut past_boundary = self.len() as isize;
        
        for i in self.pos + 1 .. self.len() as isize {
            if !WORD_BOUNDARIES.contains(self.buffer[i as usize]) {
                past_boundary = i;
                break;
            }
        }

        for i in past_boundary .. self.len() as isize {
            if WORD_BOUNDARIES.contains(self.buffer[i as usize]) {
                return i as usize;
            }
        }

        return self.len();
    }

    fn forward_kill_word(&mut self) {
        let i = self.forward_find_word();
        self.buffer.splice(self.cursor_pos() .. i, "".chars());
    }

    fn backward_kill_line(&mut self) {
        self.buffer.splice(0 .. self.cursor_pos(), "".chars());
        self.pos = 0;
    }

    fn forward_kill_line(&mut self) {
        self.buffer.splice(self.cursor_pos() .. self.len(), "".chars());
    }

    fn backward_word(&mut self) {
        self.pos = self.backward_find_word() as isize;
    }

    fn forward_word(&mut self) {
        self.pos = self.forward_find_word() as isize;
    }

    fn cursor_end(&mut self) {
        self.pos = self.buffer.len() as isize;
    }

    fn cursor_begin(&mut self) {
        self.pos = 0;
    }

    fn cursor_move(&mut self, delta: isize) {
        let result = self.pos + delta;
        if result >= 0 && result <= self.buffer.len() as isize {
            self.pos = result;
        }
    }

    fn cursor_pos(&self) -> usize {
        return self.pos as usize;
    }

    fn len(&self) -> usize {
        return self.buffer.len();
    }

    fn _insert_at(&mut self, index: usize, element: char) {
        self.buffer.insert(index, element);
    }

    fn reset_buffer<IntoString: Into<String>>(&mut self, s: IntoString) {
        let s = s.into() as String;
        self.buffer = s.chars().collect();
        self.pos = self.buffer.len() as isize;
    }

    fn insert_string<IntoString: Into<String>>(&mut self, s: IntoString) {
        let s = s.into() as String;
        if self.cursor_pos() == self.len() {
            self.buffer.extend(s.chars());

        } else {
            self.buffer.splice(self.cursor_pos() .. self.cursor_pos(), s.chars());
        }
        self.pos += s.len() as isize;
    }
}

/// This struct holds the current state of the app. In particular, it has the `items` field which is a wrapper
/// around `ListState`. Keeping track of the items state let us render the associated widget with its state
/// and have access to features such as natural scrolling.
///
/// Check the event handling at the bottom to see how to change the state on incoming events.
/// Check the drawing logic for items on how to specify the highlighting style for selected items.
struct SearchableListApp {
    input_line: InputLine,
    list_state: ListState,
    providers: Vec<Box<dyn ListContentProvider>>,
    selected_provider: usize,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> SearchableListApp {
    fn new(providers: Vec<Box<dyn ListContentProvider>>, screen_width: u16, screen_height: u16) -> SearchableListApp {
        SearchableListApp {
            input_line: InputLine::default(),
            list_state: ListState::default(),
            providers,
            selected_provider: 0,
            screen_width,
            screen_height,
        }
    }

    fn current_provider(&self) -> &dyn ListContentProvider {
        assert!(self.selected_provider < self.providers.len());
        return self.providers[self.selected_provider].as_ref()
    }

    fn current_provider_mut(&mut self) -> &mut dyn ListContentProvider {
        assert!(self.selected_provider < self.providers.len());
        return self.providers[self.selected_provider].as_mut()
    }

    fn next_provider(&mut self) {
        self.list_state = ListState::default();
        self.input_line.clear();
        self.set_query((&self.input_line).into());
        self.selected_provider = if self.selected_provider >= self.providers.len() - 1 {
            0
        } else {
            self.selected_provider + 1
        };
    }

    fn set_query(&mut self, filter: String) {
        self.current_provider_mut().set_query(filter);
    }

    fn list_next(&mut self) {
        let list_len = self.current_provider_mut().query_for_items().len();
        if list_len == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= list_len - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn list_previous(&mut self) {
        let list_len = self.current_provider_mut().query_for_items().len();
        if list_len == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    list_len - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn list_page_next(&mut self) {
        let list_len = self.current_provider_mut().query_for_items().len();
        if list_len == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                // -1 for input prompt
                if i + self.screen_height as usize - 1 >= list_len {
                    0
                } else {
                    i + self.screen_height as usize - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn list_page_prev(&mut self) {
        let list_len = self.current_provider_mut().query_for_items().len();
        if list_len == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                // -1 for input prompt
                if i as isize - (self.screen_height as isize - 1) < 0 {
                    list_len - 1
                } else {
                    i - (self.screen_height as usize - 1)
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn list_unselect(&mut self) {
        self.list_state.select(None);
    }

    fn on_tick(&mut self) {
        // let mut message = MSG::default();
        // unsafe {
        //     while PeekMessageA(&mut message, HWND(0), 0, 0, PM_REMOVE).into() {
        //         DispatchMessageA(&message);
        //     }
        // }
    }
}

impl Drop for SearchableListApp {
    fn drop(&mut self) {
        // unsafe {
            // DestroyWindow(self.input_window);
        // }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("switch")
        .arg(Arg::new("mode")
            .short('m')
            .long("mode")
            .help("Start in mode window or startapps")
            .value_name("MODE")
            .takes_value(true))
        .get_matches();

    switch::log::initialize_log(log::Level::Debug, &["init", "start"], switch::path::get_app_data_path("switch.log")?)?;

    // setup terminal
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout,
        EnterAlternateScreen, 
        EnableMouseCapture,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
    )?;

    // If window is resized then redrawing will draw more stuff and cause terminal to scroll.
    // Disable scrolling by clearing terminal buffer. By the way crossterm::terminal::Clear doesn't work,
    // so we do it ourselves.
    // Note this also clears crash messages, so comment this when debugging.
    unsafe {
        console::enable_vt_mode();
        console::clear_console()?;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // terminal.show_cursor()?;

    let screen_width = terminal.get_frame().size().width;
    let screen_height = terminal.get_frame().size().height;

    // create app and run it
    let tick_rate = Duration::from_millis(1000);
    let mut app = SearchableListApp::new(vec![
        WindowProvider::new(),
        StartAppsProvider::new(),
    ], screen_width, screen_height);

    let selected_mode = matches.value_of("mode").unwrap_or("window");
    if selected_mode == "window" {
        app.list_next();
        if app.current_provider_mut().query_for_items().len() > 1 {
            app.list_next();
        }
    } else if selected_mode == "startapps" {
        app.next_provider();
    }

    let res = run_app(&mut terminal, app, tick_rate);

    // Clear terminal and restore to original mode.
    unsafe {
        console::clear_console()?;
    }
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    // terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: SearchableListApp,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        // This causes flicker. Figure out how to double buffer.
        // terminal.clear()?;

        terminal.draw(|f| ui(f, &mut app))?;
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                            return Ok(())
                        } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u' {
                            app.input_line.backward_kill_line();
                        } else if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'k' {
                            app.input_line.forward_kill_line();
                        } else {
                            app.input_line.insert(c);
                        }

                        app.set_query((&app.input_line).into());
                        app.list_state.select(Some(0));
                        // Hide cursor before redrawing input line to prevent flickering cursor.
                        let _ = terminal.hide_cursor();
                    },
                    KeyCode::Home => {
                        app.input_line.cursor_begin();
                    },
                    KeyCode::End => {
                        app.input_line.cursor_end();
                    },
                    KeyCode::Left => {
                        // app.list_unselect()
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            app.input_line.backward_word();
                        } else {
                            app.input_line.cursor_move(-1);
                        }
                    },
                    KeyCode::Right => {
                        // app.list_unselect()
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            app.input_line.forward_word();
                        } else {
                            app.input_line.cursor_move(1);
                        }
                    },
                    KeyCode::Down => app.list_next(),
                    KeyCode::Up => app.list_previous(),
                    KeyCode::Backspace => {
                        if app.input_line.len() == 0 {
                            continue;
                        }
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            // let kill_back_word_pos = app.input_buffer.iter().rev().position(|&x| x == ' ').unwrap_or(app.input_buffer.len() - 1);
                            // app.input_buffer.truncate(app.input_buffer.len() - kill_back_word_pos - 1);
                            app.input_line.backward_kill_word();
                        } else {
                            app.input_line.backward_remove_cursor();
                        }
                        app.set_query((&app.input_line).into());
                        app.list_state.select(Some(0));
                    },
                    KeyCode::Esc => {
                        return Ok(())
                    },
                    KeyCode::Enter => {
                        let selected = app.list_state.selected().unwrap_or(std::usize::MAX);
                        // unsafe { 
                            // There are rules for who can set foreground window, and if you fail
                            // it just flashes that window in task bar and nothing else
                            // SetForegroundWindow(app.list[selected].windowh);


                            // This brings the other window to foreground, but doesn't focus it.
                            // SetWindowPos(app.list[selected].windowh, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
                            // SetWindowPos(app.list[selected].windowh, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE);
                        // }

                        //set_foreground_window_in_foreground(app.list[selected].windowh);
                        // std::assert!(set_foreground_window(app.list[selected].windowh).is_ok());
                        // set_foreground_window_ex(app.query_for_items()[selected].windowh);
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            switch::trace!("start", log::Level::Info, "Start app: KeyModifiers::CONTROL");
                            app.current_provider_mut().start(selected, true);
                        } else {
                            switch::trace!("start", log::Level::Info, "Start app`");
                            app.current_provider_mut().start(selected, false);
                        }

                        return Ok(())
                    },
                    KeyCode::Delete => {
                        let selected = app.list_state.selected().unwrap_or(std::usize::MAX);
                        app.current_provider_mut().remove(selected);
                    },
                    KeyCode::F(1) => {
                        app.next_provider();
                    },
                    KeyCode::Tab => {
                        if let Some(selected) = app.list_state.selected() {
                            if selected >= app.current_provider_mut().query_for_items().len() {
                                continue;
                            }
                            let s = (&app.current_provider_mut().query_for_items()[selected]).as_matchable_string();
                            app.input_line.reset_buffer(&s);
                            app.current_provider_mut().set_query(s);
                            app.list_state.select(Some(0));
                        }
                    },
                    KeyCode::PageDown => {
                        app.list_page_next();
                    },
                    KeyCode::PageUp => {
                        app.list_page_prev();
                    },
                    _ => {},
                },
                Event::Mouse(key) => match key.kind {
                    crossterm::event::MouseEventKind::Down(button) => match button {
                        crossterm::event::MouseButton::Right => {
                            app.input_line.insert_string(switch::clipboard::get_text());
                            let line = String::from(&app.input_line);
                            app.current_provider_mut().set_query(line);
                            app.list_state.select(Some(0));
                        },
                        _ => {},
                    },
                    _ => {},
                },
                Event::Resize(width, height) => {
                    app.screen_width = width;
                    app.screen_height = height;
                    unsafe {
                        terminal.clear()?;
                        console::clear_console()?;
                    }
                },
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // app.on_tick();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut SearchableListApp) {
    // Iterate through all elements in the `items` app and append some debug text to it.
    let items: Vec<ListItem> = app.current_provider_mut()
        .query_for_names()
        .iter()
        .map(|i| {
            // let mut lines = vec![Spans::from(i.0)];
            // for _ in 0..i.1 {
            //     lines.push(Spans::from(Span::styled(
            //         "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
            //         Style::default().add_modifier(Modifier::ITALIC),
            //     )));
            // }
            // ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
            ListItem::new(Spans::from(String::from(i)))
        })
        .collect();

    let rendered_input_line = INPUT_PROMPT.to_string() + &(String::from(&app.input_line));
    let rendered_input_line = if rendered_input_line.len() > app.screen_width as usize {
        rendered_input_line[0..app.screen_width as usize].to_string()
    } else {
        rendered_input_line
    };

    let cursor_col = INPUT_PROMPT.len() + app.input_line.cursor_pos();
    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::NONE).title(Spans::from(rendered_input_line)))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );

    // We can now render the item list
    //f.render_stateful_widget(items, chunks[0], &mut app.list.state);
    f.render_stateful_widget(items, f.size(), &mut app.list_state);
    // Show cursor after drawing finishes to prevent flickering cursor.
    f.set_cursor(cursor_col as u16, 0);
}