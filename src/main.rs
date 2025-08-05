use memmap2::Mmap;
use std::env;
use std::fs::File;
use xmz::stats::print_stats;
use xmz::tui::run_tui;

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let use_tui = args.iter().any(|a| a == "--tui");

    if use_tui {
        run_tui()?;
    } else {
        let file = File::open("psd7003.xml")?;
        let mmap = unsafe { Mmap::map(&file)? };
        let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");
        print_stats(xml);
    }

    Ok(())
}