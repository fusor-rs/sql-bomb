use fusor::{FromInputs, OwnerHandle, Signal, signal};
use hypercmd::{Error, Services};
use std::time::Duration;

const REVEAL_INTERVAL: Duration = Duration::from_millis(166);
const LOGO_UPPER: &str = include_str!("../../assets/brand/sql-bomb-terminal-upper.txt");
const LOGO_LOWER: &str = include_str!("../../assets/brand/sql-bomb-terminal-lower.txt");

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Phase {
    Outline,
    Fuse,
    Database,
    Title,
}

pub(super) struct Welcome {
    phase: Signal<Phase>,
}

pub(super) struct WelcomeInputs {}

impl FromInputs for Welcome {
    type Inputs = WelcomeInputs;
    type Error = Error;

    fn from_inputs(_: Self::Inputs, owner: OwnerHandle) -> Result<Self, Error> {
        let phase = signal(Phase::Outline);
        let animated_phase = phase.clone();
        Services::from_owner(&owner)?.spawn(&owner, async move {
            for next_phase in [Phase::Fuse, Phase::Database, Phase::Title] {
                tokio::time::sleep(REVEAL_INTERVAL).await;
                animated_phase.set(next_phase);
            }
        })?;
        Ok(Self { phase })
    }
}

fusor::template!(backend = "hypercmd", "ui/welcome.html");
