// SPDX-License-Identifier: CC0-1.0

//! An explicitly installed, one-shot fixture transport. The service never opens a socket,
//! resolves a host, reads a proxy setting, follows a link or redirects a request.
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::ops::types::{
    DataSourceSyncJob, DataSourceSyncJobMetrics, DocumentAttribute, DocumentAttributeValue,
};
use crate::state::{term_counts, Index, StoredDataSource, StoredDocument};

#[derive(Debug, Clone)]
pub(crate) struct Page {
    pub uri: String,
    path: String,
    peer: SocketAddr,
    socket: Arc<Mutex<Option<TcpStream>>>,
}

impl Page {
    pub fn matches(&self, source: &StoredDataSource) -> bool {
        let row = &source.row;
        let Some(cfg) = row
            .configuration
            .as_ref()
            .and_then(|cfg| cfg.web_crawler_configuration.as_ref())
        else {
            return false;
        };
        let Some(seeds) = &cfg.urls.seed_url_configuration else {
            return false;
        };
        row.r#type.as_deref() == Some("WEBCRAWLER")
            && row.status.as_deref() == Some("ACTIVE")
            && row.schedule.as_deref().unwrap_or("").is_empty()
            && row.vpc_configuration.is_none()
            && row.custom_document_enrichment_configuration.is_none()
            && cfg.crawl_depth == Some(0)
            && cfg.urls.site_maps_configuration.is_none()
            && cfg.authentication_configuration.is_none()
            && cfg.proxy_configuration.is_none()
            && cfg.url_inclusion_patterns.is_none()
            && cfg.url_exclusion_patterns.is_none()
            && cfg.max_links_per_page.is_none()
            && cfg.max_content_size_per_page_in_mega_bytes.is_none()
            && cfg.max_urls_per_minute_crawl_rate.is_none()
            && seeds.web_crawler_mode.as_deref() == Some("HOST_ONLY")
            && seeds.seed_urls.as_slice() == [self.uri.as_str()]
    }

    pub fn take(&self) -> Option<TcpStream> {
        self.socket.lock().ok()?.take()
    }

    pub fn fetch(&self, mut stream: TcpStream) -> Result<String, &'static str> {
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| "socket unavailable")?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|_| "socket unavailable")?;
        write!(
            stream,
            "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: text/plain\r\nConnection: close\r\n\r\n",
            self.path, self.peer
        )
        .map_err(|_| "origin request failed")?;
        let mut input = BufReader::new(stream.take(68 * 1024));
        let mut line = String::new();
        input
            .read_line(&mut line)
            .map_err(|_| "origin status unavailable")?;
        if !matches!(line.trim_end(), "HTTP/1.0 200 OK" | "HTTP/1.1 200 OK") {
            return Err("origin did not return 200 OK");
        }
        let mut header_bytes = line.len();
        let mut length = None;
        let mut plain = false;
        loop {
            line.clear();
            if input.read_line(&mut line).map_err(|_| "invalid header")? == 0 {
                return Err("incomplete headers");
            }
            header_bytes += line.len();
            if header_bytes > 4096 {
                return Err("headers exceed fixture limit");
            }
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.split_once(':').ok_or("invalid origin header")?;
            match name.to_ascii_lowercase().as_str() {
                "content-length" if length.is_none() => {
                    length = Some(
                        value
                            .trim()
                            .parse::<usize>()
                            .map_err(|_| "invalid content length")?,
                    )
                }
                "content-type" if !plain => {
                    plain = matches!(value.trim(), "text/plain" | "text/plain; charset=utf-8")
                }
                "content-length" | "transfer-encoding" | "content-encoding" | "location" => {
                    return Err("unsupported response framing or redirect")
                }
                _ => (),
            }
        }
        let size = length
            .filter(|n| (1..=65536).contains(n))
            .ok_or("invalid or absent document size")?;
        if !plain {
            return Err("fixture requires plain text");
        }
        let mut body = vec![0; size];
        input
            .read_exact(&mut body)
            .map_err(|_| "incomplete origin body")?;
        String::from_utf8(body).map_err(|_| "invalid UTF-8 body")
    }
}

/// Install an already-connected IPv4 loopback stream for one active, empty, dedicated source/index.
/// The operator supplies the socket out of band. This method has no service API or network endpoint.
pub fn install(
    index: &mut Index,
    source_id: &str,
    stream: TcpStream,
    path: &str,
) -> Result<(), &'static str> {
    let peer = stream.peer_addr().map_err(|_| "unconnected stream")?;
    let suffix = path.strip_prefix("/seed/").ok_or("invalid fixture path")?;
    if peer.ip() != Ipv4Addr::LOCALHOST
        || suffix.len() != 32
        || !suffix.bytes().all(|b| b.is_ascii_hexdigit())
        || index.status != "ACTIVE"
        || !index.documents.is_empty()
        || index.data_sources.len() != 1
    {
        return Err("expected one dedicated empty loopback fixture");
    }
    let source = index
        .data_sources
        .get_mut(source_id)
        .ok_or("unknown source")?;
    let page = Page {
        uri: format!("http://{peer}{path}"),
        path: path.into(),
        peer,
        socket: Arc::new(Mutex::new(Some(stream))),
    };
    if !page.matches(source) || source.page.is_some() {
        return Err("source does not match exclusive socket");
    }
    source.page = Some(page);
    Ok(())
}

/// Finish only the running execution; deleting its source or stopping its job discards the result.
pub(crate) fn finish(
    index: &mut Index,
    source_id: &str,
    job_id: &str,
    page: &Page,
    result: Result<String, &'static str>,
) {
    let now = SystemTime::now();
    let Some(source) = index.data_sources.get_mut(source_id) else {
        return;
    };
    let still_bound = page.matches(source);
    let Some(job) = source.job.as_mut() else {
        return;
    };
    if job.execution_id.as_deref() != Some(job_id) || job.status.as_deref() != Some("SYNCING") {
        return;
    }
    let result = result.and_then(|text| {
        if !still_bound || index.status != "ACTIVE" || !index.documents.is_empty() {
            Err("exclusive fixture changed")
        } else {
            Ok(text)
        }
    });
    job.end_time = Some(now.into());
    let mut metrics = DataSourceSyncJobMetrics::default();
    match result {
        Ok(body) => {
            let id = format!("fixture-{job_id}");
            let doc = StoredDocument {
                id: id.clone(),
                title: Some("Owned page".into()),
                body_term_counts: term_counts(&body),
                body_text: body,
                title_term_counts: term_counts("Owned page"),
                content_type: Some("PLAIN_TEXT".into()),
                attributes: vec![DocumentAttribute {
                    key: "_source_uri".into(),
                    value: DocumentAttributeValue {
                        string_value: Some(page.uri.clone()),
                        ..Default::default()
                    },
                }],
                access_control_list: vec![],
                s3_path: None,
                indexed_at: now,
                replaced_existing: false,
            };
            index.documents.insert(id, doc);
            index.updated_at = now;
            job.status = Some("SUCCEEDED".into());
            metrics.documents_added = Some("1".into());
            metrics.documents_scanned = Some("1".into());
        }
        Err(reason) => {
            job.status = Some("FAILED".into());
            job.error_message = Some(reason.into());
            metrics.documents_added = Some("0".into());
            metrics.documents_failed = Some("1".into());
        }
    }
    job.metrics = Some(metrics);
}

pub(crate) fn running(id: &str) -> DataSourceSyncJob {
    DataSourceSyncJob {
        execution_id: Some(id.into()),
        start_time: Some(SystemTime::now().into()),
        status: Some("SYNCING".into()),
        ..Default::default()
    }
}
