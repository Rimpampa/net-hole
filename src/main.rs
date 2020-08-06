use pcp::{
    types::ResultCode, Alert, Client, InboundMap, ProtocolNumber, Request, RequestType, State,
};
use std::fmt;
use std::io::{self, BufRead, Write};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::mpsc::RecvError;
use winapi::shared::winerror::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
use winapi::shared::ws2def::{AF_INET, PSOCKADDR_IN, SOCKADDR_IN};
use winapi::shared::{ifdef::IfOperStatusUp, ntdef::NULL};
use winapi::um::iphlpapi::GetAdaptersAddresses;
use winapi::um::iptypes::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
    GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST, PIP_ADAPTER_ADDRESSES,
};

#[allow(clippy::cast_ptr_alignment)] // Can't really do it without
fn retrieve_addresses() -> Option<Vec<(Ipv4Addr, Ipv4Addr)>> {
    // Suggested by microsoft
    const CHUNKS: u32 = 15000;

    // allocate some amount of memory
    let mut buffer: Vec<u8> = Vec::with_capacity(CHUNKS as usize);
    let mut size = CHUNKS;

    let mut result;
    while {
        // Get informations about this computer interfaces
        result = unsafe {
            GetAdaptersAddresses(
                AF_INET as _, // Only IPv4
                GAA_FLAG_INCLUDE_GATEWAYS // Inclue the gateways
					// Exclue informations that aren't needed
                    | GAA_FLAG_SKIP_DNS_SERVER
                    | GAA_FLAG_SKIP_MULTICAST
                    | GAA_FLAG_SKIP_ANYCAST
                    | GAA_FLAG_SKIP_FRIENDLY_NAME,
                NULL, // Reserved
                buffer.as_mut_ptr() as _,
                &mut size as _,
            )
        };
        // The buffer is to small
        result == ERROR_BUFFER_OVERFLOW
    } {
        // Increase the buffer size
        size += CHUNKS;
        buffer = Vec::with_capacity(size as usize);
    }
    // This vector will contain pairs of (address, gateway)
    let mut interfaces = Vec::new();

    // if there is no error, `buffer` contains the linked list
    if result == NO_ERROR {
        // Get the list as a pointer
        let mut list = buffer.as_ptr() as PIP_ADAPTER_ADDRESSES;
        // For every element in the list
        while list != NULL as _ {
            // Get a reference to the current element
            let curr = unsafe { &*list };
            // Move to the next element
            list = unsafe { (*list).Next };
            // Only get the interfaces that are currently active
            if curr.OperStatus == IfOperStatusUp {
                // This vector will contain all the unicast addresses of this interface
                let mut addresses = Vec::new();

                // Get the unicast addresses list (which is another linked list)
                let mut unicast_list = curr.FirstUnicastAddress;
                // For every element in the list
                while unicast_list != NULL as _ {
                    // Get a reference to the current element
                    let curr_unicast = unsafe { &*unicast_list };
                    // Move to the next element
                    unicast_list = unsafe { (*unicast_list).Next };
                    // Get the address field which contains a pointer to the address structure
                    // SOCKADDR and the size of the structure.
                    // If the field sa_familiy of the SOCKADDR strucutre is AF_INET then it's a
                    // SOCKADDR_IN structure containing a port number and an IPv4 address
                    let raw_addr = curr_unicast.Address;
                    if raw_addr.lpSockaddr != NULL as _
						&& unsafe { *raw_addr.lpSockaddr }.sa_family == AF_INET as u16
						// Check also that the length matches
                        && raw_addr.iSockaddrLength as usize == std::mem::size_of::<SOCKADDR_IN>()
                    {
                        // Cast the pointer and get the structure
                        let sock_addr = unsafe { *(raw_addr.lpSockaddr as PSOCKADDR_IN) };
                        // Convert the address form network order
                        let addr = Ipv4Addr::from(u32::from_be(unsafe {
                            *sock_addr.sin_addr.S_un.S_addr()
                        }));
                        addresses.push(addr);
                    }
                }
                // Get the gateway addresses list (which is another linked list)
                let mut gateway_list = curr.FirstGatewayAddress;
                // For every element in the list
                while gateway_list != NULL as _ {
                    // Get a reference to the current element
                    let curr_unicast = unsafe { &*gateway_list };
                    // Move to the next element
                    gateway_list = unsafe { (*gateway_list).Next };
                    // Same for the unicast address
                    let raw_addr = curr_unicast.Address;
                    if raw_addr.lpSockaddr != NULL as _
                        && unsafe { *raw_addr.lpSockaddr }.sa_family == AF_INET as u16
                        && raw_addr.iSockaddrLength as usize == std::mem::size_of::<SOCKADDR_IN>()
                    {
                        let sock_addr = unsafe { *(raw_addr.lpSockaddr as PSOCKADDR_IN) };
                        let addr = Ipv4Addr::from(u32::from_be(unsafe {
                            *sock_addr.sin_addr.S_un.S_addr()
                        }));
                        // Pair the gateway address with every address found for this interface
                        for &address in addresses.iter() {
                            interfaces.push((address, addr));
                        }
                    }
                }
            }
        }
        Some(interfaces)
    } else {
        None
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

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    PCPError(ResultCode),
    RecvError(RecvError),
    Other(&'static str),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<RecvError> for Error {
    fn from(err: RecvError) -> Self {
        Self::RecvError(err)
    }
}

impl From<ResultCode> for Error {
    fn from(err: ResultCode) -> Self {
        Self::PCPError(err)
    }
}

impl From<&'static str> for Error {
    fn from(err: &'static str) -> Self {
        Self::Other(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "An error occurred! ({:?})", self)
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
    COMMANDS.iter().find_map(|(s, c)| match *s == command {
        true => Some(*c),
        false => None,
    })
}

fn main() -> Result<(), Error> {
    let interface = retrieve_addresses().ok_or("Could't retrieve your address")?;
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
        Err("Cannot retrieve your address".into())
    }
}
