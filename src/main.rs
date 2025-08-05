use memmap2::Mmap;
use std::fs::File;
use xmz::stats::print_stats;
use xmz::tui::run_tui;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Path to the XML file
    file_path: String,

    /// Run in TUI mode
    #[arg(long)]
    tui: bool,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let file = File::open(&cli.file_path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");

    if cli.tui {
        run_tui(xml)?;
    } else {
        print_stats(xml);
    }

    Ok(())
}
