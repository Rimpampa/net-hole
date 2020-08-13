macro_rules! opt {
    ($cond:expr, $val:expr) => {
        if $cond {
            Some($val)
        } else {
            None
        }
    };
}

// macro_rules! zip {
//     ($a:ident, $b:ident) => {
//         match ($a, $b) {
//             (Some(a), Some(b)) => Some((a, b)),
//             (None, _) | (_, None) => None,
//         }
//     };
// }

mod error;

use pcp::{Alert, Client, InboundMap, ProtocolNumber, Request, RequestType, State};
use std::fmt;
use std::io::{self, BufRead, Write};
use std::net::{IpAddr, Ipv4Addr};

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        mod win;
        use win as address;

    } else if #[cfg(target_os = "linux")] {
        mod linux;
        use linux as address;
    }
}

fn get_port() -> io::Result<u16> {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    loop {
        let mut port_str = String::new();
        print!("Insert the port number: ");
        io::stdout().flush()?;
        handle.read_line(&mut port_str)?;
        match port_str.trim_end().parse::<u16>() {
            Ok(port) => return Ok(port),
            Err(_) => println!("Not a valid port!"),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Command {
    State,
    Close,
    Help,
}

impl Command {
    pub fn list() {
        println!(
            "- {}\n- {}\n- {}",
            Command::Help,
            Command::Close,
            Command::State
        )
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Command::State => "state",
            Command::Close => "close",
            Command::Help => "help",
        };
        write!(fmt, "{}", s)
    }
}

impl fmt::Display for Command {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let explain = match self {
            Self::State => "prints the state of the mapping",
            Self::Close => "covers the hole",
            Self::Help => "shows this list",
        };
        write!(fmt, "{:?} - {}", self, explain)
    }
}

const COMMANDS: &[(&str, Command)] = &[
    ("state", Command::State),
    ("close", Command::Close),
    ("help", Command::Help),
];

fn parse(command: &str) -> Option<Command> {
    COMMANDS.iter().find_map(|(s, c)| opt!(*s == command, *c))
}

fn main() -> error::Result<()> {
    let interface = address::retrieve()?;
    if !interface.is_empty() {
        let pcp = Client::<Ipv4Addr>::start(interface[0].0, interface[0].1).unwrap();

        let port = get_port()?;

        let map = InboundMap::new(port, 900)
            .protocol(ProtocolNumber::Tcp)
            .external_port(port);

        let handle = pcp.request(map, RequestType::KeepAlive).unwrap();

        println!("Drilling the hole...");
        loop {
            match handle.wait_alert()? {
                Alert::Assigned(addr, port, _) => {
                    let addr = match addr {
                        IpAddr::V4(addr) => addr,
                        _ => return Err("Unexpected response!".into()),
                    };
                    println!("Done! Exited on {}:{}", addr, port);
                    break;
                }
                Alert::StateChange => {
                    if let State::Error(err) = handle.state() {
                        return Err(err.into());
                    }
                }
            }
        }
        println!("\nCommands available:");
        Command::list();
        let stdin = io::stdin();
        let cin = stdin.lock();
        for command in cin.lines() {
            let command = &command?;
            match parse(command) {
                Some(command) => match command {
                    Command::State => println!("Current state: {:?}", handle.state()),
                    Command::Close => {
                        handle.revoke();
                        println!("Goodbye!");
                        break;
                    }
                    Command::Help => Command::list(),
                },
                None => println!("`{}` is not recognized as a command...", command),
            }
        }
        Ok(())
    } else {
        Err(error::Error::AddressNotFound)
    }
}
