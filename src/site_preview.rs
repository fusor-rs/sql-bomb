mod cli;
mod client;
mod library;
mod results;
#[path = "../examples/site-preview/server.rs"]
mod server;
#[path = "../examples/site-preview/snapshot.rs"]
mod snapshot;
mod terminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .ok_or("Pass a temporary library directory or --svg/--text")?;
    match directory.as_str() {
        "--svg" => return snapshot::write(snapshot::Format::Svg),
        "--text" => return snapshot::write(snapshot::Format::Text),
        _ => {}
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _runtime = runtime.enter();
    let address = runtime.block_on(server::start())?;
    let cli::Command::Start(startup) =
        cli::parse([client::Client::FlightSql.profile().flag.to_owned(), address].into_iter())?
    else {
        cli::help();
        return Ok(());
    };
    let library =
        library::Library::open(&std::path::Path::new(&directory).join(library::DIRECTORY))?;
    hypercmd::native::run(hypercmd::mount::<terminal::Terminal>((startup, library))?)?;
    Ok(())
}
