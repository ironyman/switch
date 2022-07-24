use windows::{
    core::*,
    Win32::System::Console::*,
};

use std::io::Write;

pub unsafe fn enable_vt_mode() {
    let output = GetStdHandle(STD_OUTPUT_HANDLE); // Don't need to free.

    let mut mode = CONSOLE_MODE(0);
    GetConsoleMode(output, &mut mode);

    mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
    SetConsoleMode(output, mode);
}

pub unsafe fn clear_console() -> Result<()> {
    // We don't want input queued before process starts being read by the new process.
    FlushConsoleInputBuffer(GetStdHandle(STD_INPUT_HANDLE));

    enable_vt_mode();
    print!("\x1b[2J"); // Clear screen
    print!("\x1b[3J"); // Clear scrollback
    print!("\x1b[0;0H"); // Reset cursor position
    // Flush stdout and clear now so that if we panic for some reason the panic is printed after screen is cleared.
    let _ = std::io::stdout().flush();
    // FlushConsoleInputBuffer(GetStdHandle(STD_OUTPUT_HANDLE));
    return Ok(());

    // // If you resize the terminal, the scrollbar will become visible. 
    // let output = GetStdHandle(STD_OUTPUT_HANDLE); // Don't need to free.

    // let mut screen_buffer: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();

    // GetConsoleScreenBufferInfo(output, &mut screen_buffer);

    // let origin  = COORD{ X: 0, Y: 0 };
    // let mut written: u32 = 0;

    // let cells = screen_buffer.dwSize.X as u32 * screen_buffer.dwSize.Y as u32;
    // FillConsoleOutputCharacterA(
    //     output,
    //     CHAR(b' '),
    //     cells,
    //     origin,
    //     &mut written 
    // );
    // assert!(written == cells);

    // FillConsoleOutputAttribute(
    //     output,
    //     (FOREGROUND_GREEN | FOREGROUND_RED | FOREGROUND_BLUE) as u16,
    //     cells,
    //     origin,
    //     &mut written
    // );

    // SetConsoleCursorPosition(output, origin);
    // println!("rows {}", screen_buffer.dwSize.Y);
    // return Ok(());
}