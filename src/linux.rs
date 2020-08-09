use nix::ifaddrs::getifaddrs;
use nix::sys::socket::{InetAddr, SockAddr};
use std::convert::{TryFrom, TryInto};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr};

macro_rules! zip {
    ($a:ident, $b:ident) => {
        match ($a, $b) {
            (Some(a), Some(b)) => Some((a, b)),
            (None, _) | (_, None) => None,
        }
    };
}

pub fn retrieve() -> Option<Vec<(Ipv4Addr, Ipv4Addr)>> {
    let addrs = getifaddrs().ok();
    let routef = File::open("/proc/net/route").ok();
    if let Some((addrs, routef)) = zip!(addrs, routef) {
        let interfaces: Vec<(String, Ipv4Addr)> = addrs
            .filter_map(|f| match f.address {
                Some(SockAddr::Inet(InetAddr::V4(ip))) => Some((f.interface_name, ip)),
                _ => None,
            })
            .map(|(name, ip)| {
                let ip = Ipv4Addr::from(u32::from_be(ip.sin_addr.s_addr));
                (name, ip)
            })
            .collect();

        let mut out = Vec::new();

        let reader = BufReader::new(routef);
        for line in reader.lines().skip(1).map(|s| s.unwrap()) {
            let (name, line) = line.split_at(line.find('\t').unwrap());
            let (_, line) = line.split_at(1);
            for ip in interfaces.iter().filter_map(|(n, ip)| opt!(n == name, ip)) {
                let (dest, line) = line.split_at(line.find('\t').unwrap());
                let (_, line) = line.split_at(1);
                if dest == "00000000" {
                    let (gateway, _) = line.split_at(line.find('\t').unwrap());
                    let vec: [u8; 4] = hex::decode(gateway).unwrap()[..].try_into().unwrap();
                    let gateway = Ipv4Addr::from(u32::from_le_bytes(vec));
                    out.push((*ip, gateway))
                }
            }
        }

        Some(out)
    } else {
        None
    }
}
