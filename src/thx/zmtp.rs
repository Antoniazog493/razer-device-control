//! Just enough ZeroMQ to talk to the THX service like Synapse does: a `REQ`
//! socket over ZMTP 3.1 with the `NULL` mechanism (no security), one peer,
//! one request at a time (ADR 0005). Generic over the stream so tests can
//! play the service.

use std::io::{Read, Write};

/// Frame flags (ZMTP 3.1, RFC 37).
const MORE: u8 = 0x01;
const LONG: u8 = 0x02;
const COMMAND: u8 = 0x04;

/// Greeting: signature, version 3.1, mechanism "NULL", as-server = 0.
fn greeting() -> [u8; 64] {
    let mut g = [0u8; 64];
    g[0] = 0xff;
    g[8] = 0x01; // libzmq's padding, kept for older peers
    g[9] = 0x7f;
    g[10] = 3;
    g[11] = 1;
    g[12..16].copy_from_slice(b"NULL");
    g
}

/// One frame: flags, size (1 byte or 8 big-endian) and body.
fn frame(flags: u8, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 9);
    if body.len() > 255 {
        out.push(flags | LONG);
        out.extend_from_slice(&(body.len() as u64).to_be_bytes());
    } else {
        out.push(flags);
        out.push(body.len() as u8);
    }
    out.extend_from_slice(body);
    out
}

/// The `READY` command of the `NULL` mechanism, with our socket type. An
/// empty `Identity`, as libzmq's REQ sends it.
fn ready_command() -> Vec<u8> {
    let mut body = vec![5];
    body.extend_from_slice(b"READY");
    for (name, value) in [("Socket-Type", &b"REQ"[..]), ("Identity", &b""[..])] {
        body.push(name.len() as u8);
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(&(value.len() as u32).to_be_bytes());
        body.extend_from_slice(value);
    }
    frame(COMMAND, &body)
}

/// A REQ connection to one peer.
pub struct Req<S> {
    stream: S,
}

impl<S: Read + Write> Req<S> {
    /// Do the handshake on a freshly connected stream.
    pub fn handshake(mut stream: S) -> Result<Self, String> {
        let io = |e: std::io::Error| format!("saludo ZeroMQ: {e}");
        stream.write_all(&greeting()).map_err(io)?;
        let mut peer = [0u8; 64];
        stream.read_exact(&mut peer).map_err(io)?;
        if peer[0] != 0xff || peer[9] != 0x7f || peer[10] < 3 {
            return Err("el servicio no habla ZMTP 3".into());
        }
        if !peer[12..32].starts_with(b"NULL\0") {
            return Err("el servicio pide un mecanismo de seguridad desconocido".into());
        }
        stream.write_all(&ready_command()).map_err(io)?;
        let mut req = Self { stream };
        let (flags, body) = req.read_frame()?;
        let name = body.get(1..1 + usize::from(*body.first().unwrap_or(&0))).unwrap_or_default();
        if flags & COMMAND == 0 || name != b"READY" {
            let mut text = "el servicio rechazó la conexión ZeroMQ".to_string();
            if name == b"ERROR" {
                // ERROR: a 1-byte length and the reason.
                text += &format!(": {}", String::from_utf8_lossy(body.get(7..).unwrap_or_default()));
            }
            return Err(text);
        }
        Ok(req)
    }

    fn read_frame(&mut self) -> Result<(u8, Vec<u8>), String> {
        let io = |e: std::io::Error| format!("lectura ZeroMQ: {e}");
        let mut head = [0u8; 1];
        self.stream.read_exact(&mut head).map_err(io)?;
        let size = if head[0] & LONG != 0 {
            let mut b = [0u8; 8];
            self.stream.read_exact(&mut b).map_err(io)?;
            u64::from_be_bytes(b)
        } else {
            let mut b = [0u8; 1];
            self.stream.read_exact(&mut b).map_err(io)?;
            u64::from(b[0])
        };
        // The service's messages are a few KB; anything huge is a broken stream.
        if size > 1 << 20 {
            return Err("trama ZeroMQ demasiado grande".into());
        }
        let mut body = vec![0u8; size as usize];
        self.stream.read_exact(&mut body).map_err(io)?;
        Ok((head[0], body))
    }

    /// Send a multipart request and wait for the multipart reply.
    pub fn request(&mut self, parts: &[&[u8]]) -> Result<Vec<Vec<u8>>, String> {
        // REQ puts an empty delimiter frame before the parts.
        let mut out = frame(if parts.is_empty() { 0 } else { MORE }, &[]);
        for (i, part) in parts.iter().enumerate() {
            out.extend(frame(if i + 1 < parts.len() { MORE } else { 0 }, part));
        }
        self.stream.write_all(&out).map_err(|e| format!("envío ZeroMQ: {e}"))?;

        let mut reply = Vec::new();
        loop {
            let (flags, body) = self.read_frame()?;
            // Commands between messages (heartbeats) aren't part of the reply.
            if flags & COMMAND != 0 {
                continue;
            }
            reply.push(body);
            if flags & MORE == 0 {
                break;
            }
        }
        // Drop the delimiter (and anything a ROUTER put before it).
        match reply.iter().position(Vec::is_empty) {
            Some(i) => Ok(reply.split_off(i + 1)),
            None => Err("respuesta ZeroMQ sin delimitador".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// A stream that returns scripted bytes and records what is written.
    struct Fake {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.input.read(buf)
        }
    }

    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.output.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// What libzmq's REP sends in its handshake (NULL, READY with Socket-Type).
    fn service_hello() -> Vec<u8> {
        let mut out = greeting().to_vec();
        let mut ready = vec![5];
        ready.extend_from_slice(b"READY\x0bSocket-Type\x00\x00\x00\x03REP");
        out.extend(frame(COMMAND, &ready));
        out
    }

    fn fake(input: Vec<u8>) -> Fake {
        Fake { input: Cursor::new(input), output: Vec::new() }
    }

    #[test]
    fn greeting_and_ready_bytes() {
        let g = greeting();
        assert_eq!(&g[..12], b"\xff\0\0\0\0\0\0\0\x01\x7f\x03\x01");
        assert_eq!(&g[12..32], b"NULL\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0");
        assert!(g[32..].iter().all(|&b| b == 0));
        assert_eq!(
            ready_command(),
            b"\x04\x26\x05READY\x0bSocket-Type\x00\x00\x00\x03REQ\x08Identity\x00\x00\x00\x00".to_vec()
        );
    }

    #[test]
    fn request_and_reply() {
        let mut input = service_hello();
        input.extend(frame(COMMAND, b"\x04PING\x00\x00")); // ignored
        input.extend(frame(MORE, b""));
        input.extend(frame(0, &[b'x'; 300])); // long frame
        let mut req = Req::handshake(fake(input)).unwrap();
        let reply = req.request(&[b"x-address:a", b"x-payload:b"]).unwrap();
        assert_eq!(reply, vec![vec![b'x'; 300]]);

        let mut want = greeting().to_vec();
        want.extend(ready_command());
        want.extend(b"\x01\x00\x01\x0bx-address:a\x00\x0bx-payload:b");
        assert_eq!(req.stream.output, want);
    }

    #[test]
    fn long_frames_are_sent_long() {
        let f = frame(0, &[7u8; 256]);
        assert_eq!(&f[..9], b"\x02\x00\x00\x00\x00\x00\x00\x01\x00");
        assert_eq!(f.len(), 9 + 256);
    }

    #[test]
    fn refuses_bad_peers() {
        let err = |input: Vec<u8>| Req::handshake(fake(input)).err().unwrap();
        assert!(err(b"HTTP/1.1 400 Bad Request\r\n".to_vec()).contains("saludo"));
        let mut curve = greeting();
        curve[12..17].copy_from_slice(b"CURVE");
        assert!(err(curve.to_vec()).contains("mecanismo"));
        let mut refused = greeting().to_vec();
        refused.extend(frame(COMMAND, b"\x05ERROR\x04nope"));
        assert!(err(refused).ends_with("nope"));

        let mut no_delimiter = service_hello();
        no_delimiter.extend(frame(0, b"x"));
        let mut req = Req::handshake(fake(no_delimiter)).unwrap();
        assert!(req.request(&[b"a"]).is_err());
    }
}
