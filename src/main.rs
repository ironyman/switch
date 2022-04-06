#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
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
    layout::{Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame, Terminal,
};

use crate::windows::{
    WindowInfo,
    enum_window,
    set_foreground_window_ex,
};

mod windows;

/// This struct holds the current state of the app. In particular, it has the `items` field which is a wrapper
/// around `ListState`. Keeping track of the items state let us render the associated widget with its state
/// and have access to features such as natural scrolling.
///
/// Check the event handling at the bottom to see how to change the state on incoming events.
/// Check the drawing logic for items on how to specify the highlighting style for selected items.
struct App {
    input_buffer: Vec<char>,
    list_state: ListState,
    list: Vec<WindowInfo>,
    // input_window: HWND,
}

impl<'a> App {
    fn new() -> App {
        let windows = enum_window().unwrap();

        App {
            list_state: ListState::default(),
            input_buffer: Vec::new(),
            list: windows,
            // input_window: create_window().unwrap(),
        }
    }

    fn next(&mut self) {
        // let list = self.list.len();
        let list = self.get_filtered_list();
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

    fn previous(&mut self) {
        // let list = self.list.len();
        let list = self.get_filtered_list();

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

    fn unselect(&mut self) {
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

    fn get_filtered_list(&'a self) -> Vec<&'a WindowInfo> {
        // Skip the first one which is the host of this app, wt or conhost.
        self.list.iter().skip(1).filter(|&w| {
            if w.image_name.to_lowercase().contains(&self.input_buffer.iter().cloned().collect::<String>()) {
                return true;
            }
            if w.window_text.to_lowercase().contains(&self.input_buffer.iter().cloned().collect::<String>()) {
                return true;
            }
            return false;
        }).collect()
    }
}

impl Drop for App {
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let tick_rate = Duration::from_millis(250);
    let mut app = App::new();
    app.next();

    let res = run_app(&mut terminal, app, tick_rate);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App,
    tick_rate: Duration,
) -> io::Result<()> {
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char(c) => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                            return Ok(())
                        }
                        app.input_buffer.push(c);
                        app.list_state.select(Some(0));
                    },
                    KeyCode::Left => app.unselect(),
                    KeyCode::Down => app.next(),
                    KeyCode::Up => app.previous(),
                    KeyCode::Backspace => {
                        if key.modifiers.contains(KeyModifiers::CONTROL) {
                            let kill_back_word_pos = app.input_buffer.iter().rev().position(|&x| x == ' ').unwrap_or(app.input_buffer.len() - 1);
                            app.input_buffer.truncate(app.input_buffer.len() - kill_back_word_pos - 1);
                        } else {
                            app.input_buffer.pop();
                        }
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
                        set_foreground_window_ex(app.get_filtered_list()[selected].windowh);
                        return Ok(())
                    }
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    // Create two chunks with equal horizontal screen space
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    // Iterate through all elements in the `items` app and append some debug text to it.
    let items: Vec<ListItem> = app
        .get_filtered_list()
        .iter()
        .map(|&i| {
            // let mut lines = vec![Spans::from(i.0)];
            // for _ in 0..i.1 {
            //     lines.push(Spans::from(Span::styled(
            //         "Lorem ipsum dolor sit amet, consectetur adipiscing elit.",
            //         Style::default().add_modifier(Modifier::ITALIC),
            //     )));
            // }
            // ListItem::new(lines).style(Style::default().fg(Color::Black).bg(Color::White))
            ListItem::new(Spans::from(format!("{}: {} ({}, {}, {})", i.image_name, i.window_text, i.process_id, i.style.0, i.ex_style.0)))
        })
        .collect();

    // Create a List from all list items and highlight the currently selected one
    let items = List::new(items)
        .block(Block::default().borders(Borders::NONE).title(Spans::from(app.input_buffer.iter().cloned().collect::<String>())))
        .highlight_style(
            Style::default()
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        );

    // We can now render the item list
    //f.render_stateful_widget(items, chunks[0], &mut app.list.state);
    f.render_stateful_widget(items, f.size(), &mut app.list_state);
}