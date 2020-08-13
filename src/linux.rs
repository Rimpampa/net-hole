mod route_parser;
use crate::error;
use nix::ifaddrs::getifaddrs;
use nix::sys::socket::{InetAddr, SockAddr};
use std::net::Ipv4Addr;

pub fn retrieve() -> error::Result<Vec<(Ipv4Addr, Ipv4Addr)>> {
    let mut out = Vec::new();
    let routes = route_parser::parse()?;
    let ifaces = getifaddrs()?
        // Take only the IPv4 addresses
        .filter_map(|f| match f.address {
            Some(SockAddr::Inet(InetAddr::V4(ip))) => Some((f.interface_name, ip)),
            _ => None,
        })
        // Take the raw IPv4 address and make it into the proper struct, `Ipv4Addr`
        .map(|(name, ip)| {
            let ip = Ipv4Addr::from(u32::from_be(ip.sin_addr.s_addr));
            (name, ip)
        });
    for (iface, addr) in ifaces {
        let routes = routes
            .iter()
            .filter(|route| route.iface == iface && route.flags.up && route.flags.gateway);
        for route in routes {
            out.push((addr, route.gateway))
        }
    }

    Ok(out)
}
