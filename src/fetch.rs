//! Downloads assets from public http(s) URLs. Blocks `file://`, localhost and
//! private networks, and re-checks every redirect hop. A `data:` URL is
//! decoded in place.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Url;

/// Largest asset accepted from a URL or base64, bytes.
pub const MAX_ASSET_BYTES: usize = 50 * 1024 * 1024;
const MAX_REDIRECTS: usize = 5;

/// Downloads a public http(s) URL, re-checking every redirect hop, or
/// decodes a `data:` URL.
///
/// # Errors
/// Non-http(s) or non-public URLs, HTTP errors, oversized bodies, too many redirects.
pub async fn fetch(url: &str) -> Result<Vec<u8>> {
    if let Some(data) = url.strip_prefix("data:") {
        return data_url(data);
    }
    let mut url = Url::parse(url).context("bad URL")?;
    for _ in 0..=MAX_REDIRECTS {
        let addr = public_addr(&url).await?;
        let host = url.host_str().unwrap_or_default().to_owned();
        // Pin the checked address so DNS can't change between check and connect.
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .resolve(&host, addr)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()?;
        let mut resp = client.get(url.clone()).send().await?;
        if resp.status().is_redirection() {
            let loc = resp
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| anyhow!("redirect without Location"))?;
            url = url.join(loc)?;
            continue;
        }
        if !resp.status().is_success() {
            bail!("HTTP {}", resp.status());
        }
        if resp
            .content_length()
            .is_some_and(|n| n > MAX_ASSET_BYTES as u64)
        {
            bail!("asset larger than {} MB", MAX_ASSET_BYTES >> 20);
        }
        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await? {
            body.extend_from_slice(&chunk);
            if body.len() > MAX_ASSET_BYTES {
                bail!("asset larger than {} MB", MAX_ASSET_BYTES >> 20);
            }
        }
        return Ok(body);
    }
    bail!("too many redirects")
}

/// The bytes of a `data:` URL (after `data:`): base64, or percent-encoded
/// text such as an inline SVG.
fn data_url(data: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    let (meta, body) = data
        .split_once(',')
        .ok_or_else(|| anyhow!("data: URL needs a comma before its data"))?;
    let bytes = if meta.ends_with(";base64") {
        base64::engine::general_purpose::STANDARD
            .decode(body.trim())
            .context("bad base64 in data: URL")?
    } else {
        let mut out = Vec::with_capacity(body.len());
        let mut rest = body.as_bytes();
        while let Some((&b, tail)) = rest.split_first() {
            let hex = tail
                .get(..2)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            match (b, hex) {
                (b'%', Some(v)) => {
                    out.push(v);
                    rest = &tail[2..];
                }
                _ => {
                    out.push(b);
                    rest = tail;
                }
            }
        }
        out
    };
    if bytes.len() > MAX_ASSET_BYTES {
        bail!("asset larger than {} MB", MAX_ASSET_BYTES >> 20);
    }
    Ok(bytes)
}

/// Resolves the URL's host and returns an address only if it's public.
async fn public_addr(url: &Url) -> Result<SocketAddr> {
    if !matches!(url.scheme(), "http" | "https") {
        bail!("only http(s) URLs are allowed");
    }
    let host = url.host_str().ok_or_else(|| anyhow!("URL has no host"))?;
    let port = url.port_or_known_default().unwrap_or(443);
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.trim_matches(['[', ']']), port))
        .await
        .with_context(|| format!("can't resolve {host}"))?
        .collect();
    if addrs.is_empty() || addrs.iter().any(|a| !is_public(a.ip())) {
        bail!("{host} is not a public address");
    }
    Ok(addrs[0])
}

fn is_public(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || a == 0
                || (a == 100 && (64..128).contains(&b)) // carrier-grade NAT
                || (a == 198 && (18..20).contains(&b))) // benchmarking
        }
        IpAddr::V6(v6) => {
            // Ranges that carry an IPv4 address are judged by that address.
            let o = v6.octets();
            let embedded = match v6.segments() {
                // NAT64, and IPv4-compatible (which covers :: and ::1).
                [0x64, 0xff9b, 0, 0, 0, 0, _, _] | [0, 0, 0, 0, 0, 0, _, _] => {
                    Some([o[12], o[13], o[14], o[15]])
                }
                [0x64, 0xff9b, ..] => return false, // local-use NAT64
                [0x2002, ..] => Some([o[2], o[3], o[4], o[5]]), // 6to4
                _ => None,
            };
            if let Some(v4) = embedded {
                return is_public(IpAddr::from(v4));
            }
            let first = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xfe00) == 0xfc00 // unique local
                || (first & 0xffc0) == 0xfe80) // link-local
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_urls_decode_in_place() {
        assert_eq!(
            data_url("image/svg+xml;utf8,<svg fill=\"%23fff\"/>").unwrap(),
            b"<svg fill=\"#fff\"/>"
        );
        assert_eq!(data_url("text/plain;base64,aGk=").unwrap(), b"hi");
        assert_eq!(data_url("text/plain,100%").unwrap(), b"100%");
        assert!(
            data_url("image/png")
                .unwrap_err()
                .to_string()
                .contains("comma")
        );
    }

    #[test]
    fn private_and_local_addresses_are_blocked() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "::1",
            "fc00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "::",
            "::10.0.0.1",
            "64:ff9b::169.254.169.254",
            "64:ff9b:1::1",
            "2002:a9fe:a9fe::1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip} should be blocked");
        }
        for ip in [
            "8.8.8.8",
            "151.101.1.69",
            "2606:4700::1111",
            "64:ff9b::8.8.8.8",
            "2002:808:808::1",
        ] {
            assert!(is_public(ip.parse().unwrap()), "{ip} should be allowed");
        }
    }

    #[tokio::test]
    async fn non_http_and_localhost_urls_are_rejected() {
        for url in [
            "file:///etc/passwd",
            "http://localhost/x.png",
            "http://127.0.0.1/x.png",
            "http://[::1]/x.png",
        ] {
            assert!(fetch(url).await.is_err(), "{url} should be rejected");
        }
    }
}
