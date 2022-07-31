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

/// This struct holds the current state of the app. In particular, it has the `items` field which is a wrapper
/// around `ListState`. Keeping track of the items state let us render the associated widget with its state
/// and have access to features such as natural scrolling.
///
/// Check the event handling at the bottom to see how to change the state on incoming events.
/// Check the drawing logic for items on how to specify the highlighting style for selected items.
struct SearchableListApp {
    input_buffer: Vec<char>,
    list_state: ListState,
    providers: Vec<Box<dyn ListContentProvider>>,
    selected_provider: usize,
    screen_width: u16,
    screen_height: u16,
}

impl<'a> SearchableListApp {
    fn new(providers: Vec<Box<dyn ListContentProvider>>, screen_width: u16, screen_height: u16) -> SearchableListApp {
        SearchableListApp {
            list_state: ListState::default(),
            input_buffer: Vec::new(),
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
        self.input_buffer.clear();
        self.set_query("".into());
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
        // let list = self.list.len();
        let list = self.current_provider().query_for_items();
        if list.len() == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i >= list.len() - 1 {
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
        // let list = self.list.len();
        let list = self.current_provider().query_for_items();
        if list.len() == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    list.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn list_page_next(&mut self) {
        // let list = self.list.len();
        let list = self.current_provider().query_for_items();
        if list.len() == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                // -1 for input prompt
                if i + self.screen_height as usize - 1 >= list.len() {
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
        // let list = self.list.len();
        let list = self.current_provider().query_for_items();
        if list.len() == 0 {
            return;
        }

        let i = match self.list_state.selected() {
            Some(i) => {
                // -1 for input prompt
                if i as isize - (self.screen_height as isize - 1) < 0 {
                    list.len() - 1
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
    let tick_rate = Duration::from_millis(250);
    let mut app = SearchableListApp::new(vec![
        WindowProvider::new(),
        StartAppsProvider::new(),
    ], screen_width, screen_height);

    let selected_mode = matches.value_of("mode").unwrap_or("window");
    if selected_mode == "window" {
        app.list_next();
        if app.current_provider().query_for_items().len() > 1 {
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
                        }
                        app.input_buffer.push(c);
                        app.set_query(app.input_buffer.iter().cloned().collect::<String>());
                        app.list_state.select(Some(0));
                    },
                    KeyCode::Left => app.list_unselect(),
                    KeyCode::Down => app.list_next(),
                    KeyCode::Up => app.list_previous(),
                    KeyCode::Backspace => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            let kill_back_word_pos = app.input_buffer.iter().rev().position(|&x| x == ' ').unwrap_or(app.input_buffer.len() - 1);
                            app.input_buffer.truncate(app.input_buffer.len() - kill_back_word_pos - 1);
                        } else {
                            app.input_buffer.pop();
                        }
                        app.set_query(app.input_buffer.iter().cloned().collect::<String>());
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
                            app.current_provider_mut().start_elevated(selected);
                        } else {
                            switch::trace!("start", log::Level::Info, "Start app: KeyModifiers::CONTROL no");
                            app.current_provider_mut().start(selected);
                        }

                        return Ok(())
                    },
                    KeyCode::Delete => {
                        let selected = app.list_state.selected().unwrap_or(std::usize::MAX);
                        app.current_provider_mut().remove(selected);
                    }
                    KeyCode::Tab => {
                        app.next_provider();
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
            app.on_tick();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut SearchableListApp) {
    // Iterate through all elements in the `items` app and append some debug text to it.
    let items: Vec<ListItem> = app.current_provider()
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

    let rendered_line_buffer = INPUT_PROMPT.to_string() + &app.input_buffer.iter().cloned().collect::<String>();
    let rendered_line_buffer = if rendered_line_buffer.len() > app.screen_width as usize {
        rendered_line_buffer[0..app.screen_width as usize].to_string()
    } else {
        rendered_line_buffer
    };

    let cursor_col = INPUT_PROMPT.len() + app.input_buffer.len();
    f.set_cursor(cursor_col as u16, 0);

    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::NONE).title(Spans::from(rendered_line_buffer)))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );

    // We can now render the item list
    //f.render_stateful_widget(items, chunks[0], &mut app.list.state);
    f.render_stateful_widget(items, f.size(), &mut app.list_state);
}