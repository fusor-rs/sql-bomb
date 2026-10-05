use crate::client::{CLIENTS, Startup};

pub(crate) const COMMAND: &str = env!("CARGO_BIN_NAME");

pub(crate) enum Command {
    Help,
    Version,
    Upgrade,
    Start(Startup),
}

pub(crate) fn parse(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Command, Box<dyn std::error::Error>> {
    let Some(first) = arguments.next() else {
        return Ok(Command::Start(Startup::Choose(None)));
    };
    if matches!(first.as_str(), "--help" | "-h") {
        return Ok(Command::Help);
    }
    let command = match first.as_str() {
        "--version" | "-V" => Some(Command::Version),
        "upgrade" => Some(Command::Upgrade),
        _ => None,
    };
    if let Some(command) = command {
        if arguments.next().is_some() {
            return Err(format!("Unexpected argument; run {COMMAND} --help for usage").into());
        }
        return Ok(command);
    }
    let client = CLIENTS.iter().find(|client| client.profile().flag == first);
    let address = if client.is_some() {
        arguments.next()
    } else {
        Some(first)
    };
    if arguments.next().is_some() || address.as_ref().is_some_and(|value| value.starts_with('-')) {
        return Err(
            format!("Unknown option or extra argument; run {COMMAND} --help for usage").into(),
        );
    }
    let startup = match (client, address) {
        (Some(client), Some(address)) => Startup::Connected(
            client
                .connect(address, String::new())
                .map_err(|error| error.to_string())?,
        ),
        (Some(client), None) => Startup::Configure(*client),
        (None, address) => Startup::Choose(address),
    };
    Ok(Command::Start(startup))
}

pub(crate) fn help() {
    println!(
        "Usage: {COMMAND} [CLIENT [CONNECTION]]
       {COMMAND} [CONNECTION]
       {COMMAND} upgrade

Without arguments, open saved connections or choose a client.
A client flag opens its connection form; adding a connection opens the workspace.

Available clients:"
    );
    for client in CLIENTS {
        let profile = client.profile();
        println!("  {} [CONNECTION]  {}", profile.flag, profile.name);
    }
    println!(
        "
  upgrade        Upgrade to the latest stable release
  -h, --help     Show this help
  -V, --version  Show the installed version

Ctrl+R runs SQL · Ctrl+E edits · Ctrl+L browses results · Ctrl+C quits."
    );
}
