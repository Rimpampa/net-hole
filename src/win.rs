use winapi::shared::winerror::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
use winapi::shared::ws2def::{AF_INET, PSOCKADDR_IN, SOCKADDR_IN};
use winapi::shared::{ifdef::IfOperStatusUp, ntdef::NULL};
use winapi::um::iphlpapi::GetAdaptersAddresses;
use winapi::um::iptypes::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER,
    GAA_FLAG_SKIP_FRIENDLY_NAME, GAA_FLAG_SKIP_MULTICAST, PIP_ADAPTER_ADDRESSES,
};

#[allow(clippy::cast_ptr_alignment)] // Can't really do it without
pub fn retrieve() -> Option<Vec<(Ipv4Addr, Ipv4Addr)>> {
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