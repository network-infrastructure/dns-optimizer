use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, RootCertStore};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid domain name: {0}")]
    InvalidDomain(String),
    #[error("invalid IP address: {0}")]
    InvalidIp(String),
    #[error("invalid TLS server name: {0}")]
    InvalidServerName(String),
    #[error("query timed out after {0} ms")]
    Timeout(u128),
    #[error("malformed DNS packet: {0}")]
    MalformedPacket(String),
    #[error("network I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Udp,
    Tcp,
    Doh,
    Dot,
}

impl Transport {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "udp" | "udp53" | "udp/53" => Some(Self::Udp),
            "tcp" | "tcp53" | "tcp/53" => Some(Self::Tcp),
            "doh" | "https" => Some(Self::Doh),
            "dot" | "tls" => Some(Self::Dot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordType {
    A = 1,
    AAAA = 28,
    TXT = 16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResponseCode {
    NoError,
    FormErr,
    ServFail,
    NxDomain,
    NotImp,
    Refused,
    Other(u8),
}

impl From<u8> for ResponseCode {
    fn from(code: u8) -> Self {
        match code & 0x0F {
            0 => Self::NoError,
            1 => Self::FormErr,
            2 => Self::ServFail,
            3 => Self::NxDomain,
            4 => Self::NotImp,
            5 => Self::Refused,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsResponse {
    pub rcode: ResponseCode,
    pub latency_ms: f64,
    pub answers: Vec<String>,
    pub transport: Transport,
    pub authenticated_data: bool,
}

/// Build an RFC 1035 wire-format DNS query packet with RD=1 and optional AD=1 (DNSSEC check).
pub fn build_query_packet(
    domain: &str,
    qtype: RecordType,
    tx_id: u16,
    request_dnssec_ad: bool,
) -> Result<Vec<u8>, ProtocolError> {
    let clean = domain.trim().trim_end_matches('.');
    if clean.is_empty() || clean.len() > 253 {
        return Err(ProtocolError::InvalidDomain(domain.to_string()));
    }

    let mut buf = Vec::with_capacity(64);
    buf.extend_from_slice(&tx_id.to_be_bytes());
    // Flags: standard query, RD=1 (0x0100), optional AD=1 (0x0020)
    let flags: u16 = if request_dnssec_ad { 0x0120 } else { 0x0100 };
    buf.extend_from_slice(&flags.to_be_bytes());
    buf.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
    buf.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT = 0
    buf.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT = 0
    buf.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT = 0

    for label in clean.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err(ProtocolError::InvalidDomain(domain.to_string()));
        }
        buf.push(label.len() as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0); // root label

    buf.extend_from_slice(&(qtype as u16).to_be_bytes()); // QTYPE
    buf.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = IN
    Ok(buf)
}

/// Skip a DNS domain name starting at `offset` (handling RFC 1035 compression pointers).
fn skip_name(packet: &[u8], mut offset: usize) -> Result<usize, ProtocolError> {
    let mut jumps = 0;
    loop {
        if offset >= packet.len() {
            return Err(ProtocolError::MalformedPacket("truncated name".into()));
        }
        let len = packet[offset];
        if len == 0 {
            return Ok(offset + 1);
        }
        if (len & 0xC0) == 0xC0 {
            if offset + 1 >= packet.len() {
                return Err(ProtocolError::MalformedPacket("truncated pointer".into()));
            }
            return Ok(offset + 2);
        }
        offset += 1 + (len as usize);
        jumps += 1;
        if jumps > 128 {
            return Err(ProtocolError::MalformedPacket("name label loop".into()));
        }
    }
}

/// Parse an RFC 1035 DNS wire-format response packet.
pub fn parse_response_packet(
    packet: &[u8],
    expected_tx_id: Option<u16>,
    latency_ms: f64,
    transport: Transport,
) -> Result<DnsResponse, ProtocolError> {
    if packet.len() < 12 {
        return Err(ProtocolError::MalformedPacket("header < 12 bytes".into()));
    }
    let tx_id = u16::from_be_bytes([packet[0], packet[1]]);
    if let Some(expected) = expected_tx_id {
        if tx_id != expected {
            return Err(ProtocolError::MalformedPacket(format!(
                "tx_id mismatch: expected {expected}, got {tx_id}"
            )));
        }
    }

    let flags = u16::from_be_bytes([packet[2], packet[3]]);
    let rcode = ResponseCode::from((flags & 0x000F) as u8);
    let authenticated_data = (flags & 0x0020) != 0;
    let qdcount = u16::from_be_bytes([packet[4], packet[5]]);
    let ancount = u16::from_be_bytes([packet[6], packet[7]]);

    let mut offset = 12;
    for _ in 0..qdcount {
        offset = skip_name(packet, offset)?;
        if offset + 4 > packet.len() {
            return Err(ProtocolError::MalformedPacket("truncated question".into()));
        }
        offset += 4; // QTYPE + QCLASS
    }

    let mut answers = Vec::new();
    for _ in 0..ancount {
        offset = skip_name(packet, offset)?;
        if offset + 10 > packet.len() {
            return Err(ProtocolError::MalformedPacket("truncated answer header".into()));
        }
        let rtype = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
        let rdlength = u16::from_be_bytes([packet[offset + 8], packet[offset + 9]]) as usize;
        offset += 10;
        if offset + rdlength > packet.len() {
            return Err(ProtocolError::MalformedPacket("truncated rdata".into()));
        }
        let rdata = &packet[offset..offset + rdlength];
        match rtype {
            1 if rdlength == 4 => {
                let ip = Ipv4Addr::new(rdata[0], rdata[1], rdata[2], rdata[3]);
                answers.push(ip.to_string());
            }
            28 if rdlength == 16 => {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(rdata);
                let ip = Ipv6Addr::from(octets);
                answers.push(ip.to_string());
            }
            16 if rdlength >= 1 => {
                let txt_len = rdata[0] as usize;
                if 1 + txt_len <= rdlength {
                    if let Ok(s) = std::str::from_utf8(&rdata[1..1 + txt_len]) {
                        answers.push(s.to_string());
                    }
                }
            }
            _ => {}
        }
        offset += rdlength;
    }

    Ok(DnsResponse {
        rcode,
        latency_ms,
        answers,
        transport,
        authenticated_data,
    })
}

/// Perform a real UDP/53 DNS query and measure resolution latency in milliseconds.
pub async fn query_udp(
    server_ip: &str,
    domain: &str,
    qtype: RecordType,
    timeout_dur: Duration,
) -> Result<DnsResponse, ProtocolError> {
    let ip: IpAddr = server_ip
        .parse()
        .map_err(|_| ProtocolError::InvalidIp(server_ip.to_string()))?;
    let bind_addr = if ip.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
    let socket = UdpSocket::bind(bind_addr).await?;
    let target = SocketAddr::new(ip, 53);

    let tx_id: u16 = rand::thread_rng().gen();
    let packet = build_query_packet(domain, qtype, tx_id, true)?;
    let mut recv_buf = [0u8; 4096];

    let start = Instant::now();
    socket.send_to(&packet, target).await?;
    let (n, _) = timeout(timeout_dur, socket.recv_from(&mut recv_buf))
        .await
        .map_err(|_| ProtocolError::Timeout(timeout_dur.as_millis()))??;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

    parse_response_packet(&recv_buf[..n], Some(tx_id), latency_ms, Transport::Udp)
}

/// Perform a real TCP/53 DNS query and measure resolution latency.
pub async fn query_tcp(
    server_ip: &str,
    domain: &str,
    qtype: RecordType,
    timeout_dur: Duration,
) -> Result<DnsResponse, ProtocolError> {
    let ip: IpAddr = server_ip
        .parse()
        .map_err(|_| ProtocolError::InvalidIp(server_ip.to_string()))?;
    let target = SocketAddr::new(ip, 53);
    let tx_id: u16 = rand::thread_rng().gen();
    let packet = build_query_packet(domain, qtype, tx_id, true)?;

    let start = Instant::now();
    let res = timeout(timeout_dur, async {
        let mut stream = TcpStream::connect(target).await?;
        let len_prefix = (packet.len() as u16).to_be_bytes();
        stream.write_all(&len_prefix).await?;
        stream.write_all(&packet).await?;
        let mut resp_len_buf = [0u8; 2];
        stream.read_exact(&mut resp_len_buf).await?;
        let resp_len = u16::from_be_bytes(resp_len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        stream.read_exact(&mut resp_buf).await?;
        Ok::<Vec<u8>, std::io::Error>(resp_buf)
    })
    .await
    .map_err(|_| ProtocolError::Timeout(timeout_dur.as_millis()))??;

    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    parse_response_packet(&res, Some(tx_id), latency_ms, Transport::Tcp)
}

/// Perform a real DNS-over-TLS (DoT, RFC 7858) query on port 853.
pub async fn query_dot(
    server_ip: &str,
    tls_hostname: &str,
    domain: &str,
    qtype: RecordType,
    timeout_dur: Duration,
) -> Result<DnsResponse, ProtocolError> {
    let ip: IpAddr = server_ip
        .parse()
        .map_err(|_| ProtocolError::InvalidIp(server_ip.to_string()))?;
    let target = SocketAddr::new(ip, 853);
    let server_name = ServerName::try_from(tls_hostname.to_string())
        .map_err(|_| ProtocolError::InvalidServerName(tls_hostname.to_string()))?;

    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));

    let tx_id: u16 = rand::thread_rng().gen();
    let packet = build_query_packet(domain, qtype, tx_id, true)?;

    let start = Instant::now();
    let res = timeout(timeout_dur, async {
        let tcp = TcpStream::connect(target).await?;
        let mut tls = connector.connect(server_name, tcp).await?;
        let len_prefix = (packet.len() as u16).to_be_bytes();
        tls.write_all(&len_prefix).await?;
        tls.write_all(&packet).await?;
        let mut resp_len_buf = [0u8; 2];
        tls.read_exact(&mut resp_len_buf).await?;
        let resp_len = u16::from_be_bytes(resp_len_buf) as usize;
        let mut resp_buf = vec![0u8; resp_len];
        tls.read_exact(&mut resp_buf).await?;
        Ok::<Vec<u8>, std::io::Error>(resp_buf)
    })
    .await
    .map_err(|_| ProtocolError::Timeout(timeout_dur.as_millis()))??;

    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    parse_response_packet(&res, Some(tx_id), latency_ms, Transport::Dot)
}

/// Perform a real DNS-over-HTTPS (DoH, RFC 8484) binary query.
pub async fn query_doh(
    client: &reqwest::Client,
    doh_url: &str,
    domain: &str,
    qtype: RecordType,
    timeout_dur: Duration,
) -> Result<DnsResponse, ProtocolError> {
    let packet = build_query_packet(domain, qtype, 0, true)?;
    let start = Instant::now();
    let resp = timeout(
        timeout_dur,
        client
            .post(doh_url)
            .header("Content-Type", "application/dns-message")
            .header("Accept", "application/dns-message")
            .body(packet)
            .send(),
    )
    .await
    .map_err(|_| ProtocolError::Timeout(timeout_dur.as_millis()))??;

    let bytes = resp.bytes().await?;
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    parse_response_packet(&bytes, None, latency_ms, Transport::Doh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_rfc1035_query_packet() {
        let pkt = build_query_packet("example.com", RecordType::A, 0x1234, true).unwrap();
        assert_eq!(pkt[0], 0x12);
        assert_eq!(pkt[1], 0x34);
        assert_eq!(pkt[2], 0x01); // RD=1
        assert_eq!(pkt[3], 0x20); // AD=1
        // Labels: 7example3com0
        assert_eq!(pkt[12], 7);
        assert_eq!(&pkt[13..20], b"example");
        assert_eq!(pkt[20], 3);
        assert_eq!(&pkt[21..24], b"com");
        assert_eq!(pkt[24], 0);
    }

    #[test]
    fn parses_synthetic_a_record_response() {
        let mut resp = build_query_packet("example.com", RecordType::A, 0x4242, false).unwrap();
        resp[2] = 0x81; // QR=1, RD=1
        resp[3] = 0x80; // RA=1, NOERROR
        resp[7] = 1; // ANCOUNT=1
        // Compressed pointer to offset 12 (0xC00C)
        resp.extend_from_slice(&[0xC0, 0x0C]);
        resp.extend_from_slice(&1u16.to_be_bytes()); // TYPE A
        resp.extend_from_slice(&1u16.to_be_bytes()); // CLASS IN
        resp.extend_from_slice(&300u32.to_be_bytes()); // TTL
        resp.extend_from_slice(&4u16.to_be_bytes()); // RDLENGTH
        resp.extend_from_slice(&[93, 184, 216, 34]); // 93.184.216.34

        let parsed = parse_response_packet(&resp, Some(0x4242), 11.5, Transport::Udp).unwrap();
        assert_eq!(parsed.rcode, ResponseCode::NoError);
        assert_eq!(parsed.answers, vec!["93.184.216.34"]);
        assert_eq!(parsed.latency_ms, 11.5);
    }
}
