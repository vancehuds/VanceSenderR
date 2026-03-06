//! Network utilities — LAN IPv4 discovery.

pub fn get_lan_ipv4_addresses() -> Vec<String> {
    // Use platform-specific network interface enumeration
    let mut addrs = Vec::new();

    #[cfg(windows)]
    {
        // Use std::net to get local addresses
        if let Ok(hostname) = hostname::get() {
            if let Ok(name) = hostname.into_string() {
                if let Ok(ips) = dns_lookup_fallback(&name) {
                    addrs = ips;
                }
            }
        }
    }

    // Fallback: try to bind a UDP socket and read local addr
    if addrs.is_empty() {
        if let Some(ip) = get_local_ip_via_udp() {
            addrs.push(ip);
        }
    }

    addrs
}

fn get_local_ip_via_udp() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    let ip = addr.ip().to_string();
    if ip != "0.0.0.0" && ip != "127.0.0.1" {
        Some(ip)
    } else {
        None
    }
}

#[cfg(windows)]
fn dns_lookup_fallback(hostname: &str) -> Result<Vec<String>, ()> {
    use std::net::ToSocketAddrs;
    let addrs: Vec<String> = format!("{hostname}:0")
        .to_socket_addrs()
        .map_err(|_| ())?
        .filter_map(|a| {
            if a.ip().is_ipv4() && !a.ip().is_loopback() {
                Some(a.ip().to_string())
            } else {
                None
            }
        })
        .collect();
    Ok(addrs)
}
