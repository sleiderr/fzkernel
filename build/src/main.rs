#![feature(async_fn_in_trait)]
#![feature(exit_status_error)]

use std::error::Error;

use std::io::Stdout;
use std::panic;
use std::{io, sync::Arc};

use conquer_once::spin::OnceCell;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use parking_lot::Mutex;
use ratatui::{prelude::CrosstermBackend, Terminal, TerminalOptions, Viewport};

use crate::{
    cli::app::{run_app, App},
    components::build::{BootloaderBuild, BootloaderBuildConfig},
    ui::build::BuildUI,
};

pub mod cli;
pub mod components;
pub mod errors;
pub mod ui;

pub static BOOTLOADER_BUILD: OnceCell<Arc<Mutex<BootloaderBuild>>> = OnceCell::uninit();
pub static TERMINAL: OnceCell<Arc<Mutex<Terminal<CrosstermBackend<Stdout>>>>> = OnceCell::uninit();
pub static APP: OnceCell<Arc<Mutex<App>>> = OnceCell::uninit();

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app: App = argh::from_env();
    APP.init_once(|| Arc::new(Mutex::new(app)));

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let term = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(8),
        },
    )?;

    TERMINAL.init_once(|| Arc::new(Mutex::new(term)));
    panic::set_hook(Box::new(|panic| {
        unsafe { TERMINAL.get_unchecked().force_unlock() };

        let mut term = unsafe { TERMINAL.get_unchecked().lock() };
        disable_raw_mode();
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        println!("{panic}");
    }));

    let mut app = APP.get().unwrap().lock();
    if app.standalone && app.fast {
        drop(app);
        let boot_img = String::from("boot.img");
        let parts = vec!["main"];
        let cfg = BootloaderBuildConfig::new(
            boot_img,
            String::from("src/fzboot/$name"),
            String::from("target/$name/x86_64-fbios/release/$name.bin"),
            parts,
        );
        let build = BootloaderBuild::new(cfg);

        BOOTLOADER_BUILD.init_once(|| Arc::new(Mutex::new(build)));

        let ui = BuildUI::default();
        ui.run();

        disable_raw_mode()?;
        let mut term_guard = TERMINAL.get().expect("Failed to load terminal").lock();
        let term = &mut *term_guard;

        execute!(term.backend_mut(), DisableMouseCapture)?;
        term.show_cursor()?;

        return Ok(());
    }
    drop(app);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_app(&mut terminal)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
