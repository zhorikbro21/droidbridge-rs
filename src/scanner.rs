//! Chunked async TCP scanner for the Android ephemeral port range.
//!
//! Same shape as the PS original: chunks of 1000 concurrent connects,
//! ~700 ms per-connect budget; a port counts as open only if the TCP
//! connect succeeds. If a full sweep finds nothing, it retries once —
//! live-stand testing showed that a 28k-SYN burst over a VPN/overlay
//! path occasionally drops the one SYN that mattered.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;

const CHUNK: usize = 1000;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(700);

/// Scan `min..=max` on `host` and return open ports in ascending order.
pub async fn scan_open_ports(host: &str, min: u16, max: u16) -> Vec<u16> {
    let first = sweep(host, min, max).await;
    if !first.is_empty() {
        return first;
    }
    sweep(host, min, max).await
}

async fn sweep(host: &str, min: u16, max: u16) -> Vec<u16> {
    let addrs: Vec<SocketAddr> = (min..=max)
        .filter_map(|p| format!("{host}:{p}").parse().ok())
        .collect();
    let mut open = Vec::new();

    for chunk in addrs.chunks(CHUNK) {
        let mut set = JoinSet::new();
        for &addr in chunk {
            set.spawn(async move {
                match timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
                    Ok(Ok(_)) => Some(addr.port()),
                    _ => None,
                }
            });
        }
        while let Some(joined) = set.join_next().await {
            if let Some(port) = joined.unwrap_or(None) {
                open.push(port);
            }
        }
    }
    open.sort_unstable();
    open
}

/// Blocking wrapper for use from synchronous code paths.
pub fn scan_open_ports_blocking(host: &str, min: u16, max: u16) -> Vec<u16> {
    crate::RUNTIME.block_on(scan_open_ports(host, min, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn finds_open_listener() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let open = scan_open_ports("127.0.0.1", port.saturating_sub(3), port).await;
        assert!(open.contains(&port), "expected {port} in {open:?}");
    }

    #[tokio::test]
    async fn closed_ports_do_not_hang() {
        // 127.0.0.1 refuses instantly; a small closed range must return
        // fast regardless of what it finds.
        let start = std::time::Instant::now();
        let _open = scan_open_ports("127.0.0.1", 1, 5).await;
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    #[tokio::test]
    #[ignore = "live stand: phone at 192.0.2.10"]
    async fn live_scan_full_range() {
        let start = std::time::Instant::now();
        let open = scan_open_ports("192.0.2.10", 32768, 60999).await;
        eprintln!("total: {:?}, open ports: {open:?}", start.elapsed());
        assert!(!open.is_empty(), "scan found nothing on live phone");
    }
}
