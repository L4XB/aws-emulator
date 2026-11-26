// SPDX-License-Identifier: CC0-1.0
//! Bounded single-key Memcached text commands on a dedicated local cache node.
use emulator_server::{ListenerConn, NativeHandler};
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

struct Item {
    flags: u32,
    expires: Option<SystemTime>,
    bytes: Vec<u8>,
}

#[derive(Default, Clone)]
pub(super) struct Wire(Arc<Mutex<HashMap<String, Item>>>);

impl NativeHandler for Wire {
    fn serve(&self, conn: ListenerConn) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let cache = self.clone();
        Box::pin(async move {
            let _ = cache.session(conn).await;
        })
    }
}

impl Wire {
    async fn session(&self, conn: ListenerConn) -> std::io::Result<()> {
        let mut io = BufReader::new(conn);
        loop {
            let mut line = Vec::new();
            (&mut io).take(1025).read_until(b'\n', &mut line).await?;
            if line.len() > 1024 || !line.ends_with(b"\r\n") {
                return Ok(());
            }
            let Ok(line) = std::str::from_utf8(&line[..line.len() - 2]) else {
                return Ok(());
            };
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            if let Some(key) = fields.get(1) {
                if key.len() > 250 || !key.bytes().all(|b| b.is_ascii_graphic()) {
                    return Ok(());
                }
            }
            let mut body = Vec::new();
            let result: &[u8] = match fields.as_slice() {
                ["version"] => b"VERSION emulator-memcached-1\r\n",
                ["quit"] => return Ok(()),
                [cmd @ ("set" | "add"), key, flags, ttl, size] => {
                    let (Ok(flags), Ok(ttl), Ok(size)) = (
                        flags.parse::<u32>(),
                        ttl.parse::<u32>(),
                        size.parse::<usize>(),
                    ) else {
                        return Ok(());
                    };
                    if size > 65536 {
                        return Ok(());
                    }
                    let mut bytes = vec![0; size];
                    io.read_exact(&mut bytes).await?;
                    let mut tail = [0; 2];
                    io.read_exact(&mut tail).await?;
                    if tail != *b"\r\n" {
                        return Ok(());
                    }
                    let now = SystemTime::now();
                    let expires = match ttl {
                        0 => None,
                        1..=2592000 => Some(now + Duration::from_secs(ttl.into())),
                        _ => Some(UNIX_EPOCH + Duration::from_secs(ttl.into())),
                    };
                    let mut items = self.0.lock().unwrap();
                    items.retain(|_, v| v.expires.is_none_or(|end| end > now));
                    if *cmd == "add" && items.contains_key(*key) {
                        b"NOT_STORED\r\n"
                    } else if items.len() >= 128 && !items.contains_key(*key) {
                        b"SERVER_ERROR cache full\r\n"
                    } else {
                        items.insert(
                            (*key).to_owned(),
                            Item {
                                flags,
                                expires,
                                bytes,
                            },
                        );
                        b"STORED\r\n"
                    }
                }
                ["get", key] => {
                    let items = self.0.lock().unwrap();
                    if let Some(item) = items
                        .get(*key)
                        .filter(|v| v.expires.is_none_or(|end| end > SystemTime::now()))
                    {
                        body.extend(
                            format!("VALUE {key} {} {}\r\n", item.flags, item.bytes.len()).bytes(),
                        );
                        body.extend(&item.bytes);
                        body.extend(b"\r\n");
                    }
                    body.extend(b"END\r\n");
                    &body
                }
                ["delete", key] => {
                    let item = self.0.lock().unwrap().remove(*key);
                    if item.is_some_and(|v| v.expires.is_none_or(|end| end > SystemTime::now())) {
                        b"DELETED\r\n"
                    } else {
                        b"NOT_FOUND\r\n"
                    }
                }
                _ => {
                    io.get_mut().write_all(b"ERROR\r\n").await?;
                    return Ok(());
                }
            };
            io.get_mut().write_all(result).await?;
        }
    }
}
