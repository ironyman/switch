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

use switch::{
    ListContentProvider,
    WindowProvider,
    StartAppsProvider,
    console,
};

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
}

impl<'a> SearchableListApp {
    fn new(providers: Vec<Box<dyn ListContentProvider>>) -> SearchableListApp {
        SearchableListApp {
            list_state: ListState::default(),
            input_buffer: Vec::new(),
            providers,
            selected_provider: 0,
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
        self.selected_provider = if self.selected_provider >= self.providers.len() {
            0
        } else {
            self.selected_provider + 1
        };
    }

    fn set_filter(&mut self, filter: String) {
        self.current_provider_mut().set_filter(filter);
    }

    fn list_next(&mut self) {
        // let list = self.list.len();
        let list = self.current_provider().get_filtered_list();
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
        let list = self.current_provider().get_filtered_list();

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
    // setup terminal
    enable_raw_mode()?;

    let mut stdout = io::stdout();
    execute!(stdout,
        EnterAlternateScreen, 
        EnableMouseCapture,
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
    )?;
    // If window is resized then redrawing will draw more stuff and cause terminal to scroll.
    // Disable scrolling by clearing terminal buffer. By the way crossterm::terminal::Clear
    // So we do it ourselves.
    unsafe {
        console::clear_console()?;
    }

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(250);
    let mut app = SearchableListApp::new(vec![
        WindowProvider::new(),
        StartAppsProvider::new(),
    ]);
    
    app.list_next();
    if app.current_provider().get_filtered_list().len() > 1 {
        app.list_next();
    }

    let res = run_app(&mut terminal, app, tick_rate);

    // Clear terminal and restore to cooked mode.
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
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                                return Ok(())
                            }
                            app.input_buffer.push(c);
                            app.set_filter(app.input_buffer.iter().cloned().collect::<String>());
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
                            app.set_filter(app.input_buffer.iter().cloned().collect::<String>());
                            app.list_state.select(Some(0));
                        },
                        KeyCode::Esc => {
                            return Ok(())
                        },
                        KeyCode::Enter => {
                            let selected = app.list_state.selected().unwrap_or(0);

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
                            // set_foreground_window_ex(app.get_filtered_list()[selected].windowh);
                            app.current_provider().activate(selected);
                            return Ok(())
                        }
                        _ => {}
                    }
                },
                Event::Resize(_width, _height) => {
                    unsafe {
                        console::clear_console()?;
                    }
                },
                _ => {

                }
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
        .get_filtered_list()
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

    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::NONE).title(Spans::from("> ".to_string() + &app.input_buffer.iter().cloned().collect::<String>())))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );

    // We can now render the item list
    //f.render_stateful_widget(items, chunks[0], &mut app.list.state);
    f.render_stateful_widget(items, f.size(), &mut app.list_state);
}