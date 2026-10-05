mod cli;
mod client;
mod library;
mod results;
mod terminal;
#[cfg(test)]
mod tests;
mod upgrade;

use terminal::Terminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let startup = match cli::parse(std::env::args().skip(1))? {
        cli::Command::Help => {
            cli::help();
            return Ok(());
        }
        cli::Command::Version => {
            println!("{} {}", cli::COMMAND, env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        cli::Command::Upgrade => return upgrade::run(),
        cli::Command::Start(startup) => startup,
    };
    let home =
        std::env::home_dir().ok_or("Cannot locate your home directory for the query library")?;
    let library = library::Library::open(&home.join(library::DIRECTORY))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _runtime = runtime.enter();
    let terminal = hypercmd::mount::<Terminal>((startup, library))?;
    hypercmd::native::run(terminal)?;
    Ok(())
}
