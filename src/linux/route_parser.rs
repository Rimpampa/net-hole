use crate::error;
use std::convert::{TryFrom, TryInto};
use std::net::Ipv4Addr;
use std::process::Command;

#[derive(Default, Debug)]
pub struct Flags {
    /// U (route is up)
    pub up: bool,
    /// H (target is a host)
    pub host: bool,
    /// G (use gateway)
    pub gateway: bool,
    /// R (reinstate route for dynamic routing)
    pub reinstate: bool,
    /// D (dynamically installed by daemon or redirect)
    pub dynamically: bool,
    /// M (modified from routing daemon or redirect)
    pub modified: bool,
    /// A (installed by addrconf)
    pub addrconf: bool,
    /// C (cache entry)
    pub cache: bool,
    /// !  (reject route)
    pub reject: bool,
}

impl TryFrom<&str> for Flags {
    type Error = error::Error;

    fn try_from(value: &str) -> error::Result<Self> {
        let mut flags = Self::default();
        for char in value.chars() {
            match char {
                'U' => flags.up = true,
                'H' => flags.host = true,
                'G' => flags.gateway = true,
                'R' => flags.reinstate = true,
                'D' => flags.dynamically = true,
                'M' => flags.modified = true,
                'A' => flags.addrconf = true,
                'C' => flags.cache = true,
                '!' => flags.reject = true,
                _ => return Err("Flag parsing failed".into()),
            }
        }
        Ok(flags)
    }
}

#[derive(Debug)]
pub struct Route {
    pub iface: String,
    pub gateway: Ipv4Addr,
    pub destination: Ipv4Addr,
    pub flags: Flags,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            iface: String::new(),
            destination: Ipv4Addr::UNSPECIFIED,
            gateway: Ipv4Addr::UNSPECIFIED,
            flags: <_>::default(),
        }
    }
}

pub fn parse() -> error::Result<Vec<Route>> {
    let output = Command::new("route").arg("-n4").output()?;
    let string: String = output.stdout.into_iter().map(|u| u as char).collect();

    let first = 1 + string
        .chars()
        .position(|c| c == '\n')
        .ok_or("Parsing failed")?;

    let second = string
        .get(first..)
        .unwrap()
        .chars()
        .position(|c| c == '\n')
        .ok_or("Parsing failed")?
        + first;
    // For both those two cases unwrap is assured to not panic as there are at
    // least `first` elements in the first case and `second` elements in this
    // last one
    let string = string.get(second + 1..).unwrap();

    let mut route = Route::default();
    let mut routes = Vec::new();

    let mut field_start = None;
    let mut field_idx = 0;
    for (i, char) in string.char_indices() {
        match field_start {
            None if !char.is_whitespace() => field_start = Some(i),
            Some(start) if char.is_whitespace() => {
                // Can unwrap here as those indices can't be invalid
                let field = string.get(start..i).unwrap();
                match field_idx {
                    0 => route.destination = field.parse()?,
                    1 => route.gateway = field.parse()?,
                    3 => route.flags = field.try_into()?,
                    7 => route.iface = field.into(),
                    2 | 4..=6 => (), // TODO: maybe add those later
                    _ => unreachable!(),
                }
                field_start = None;
                field_idx += 1;
            }
            _ => (),
        }
        if char == '\n' {
            routes.push(route);
            route = <_>::default();
            field_idx = 0;
            field_start = None;
        }
    }
    Ok(routes)
}
