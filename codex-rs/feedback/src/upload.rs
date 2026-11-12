// Local archive serialization only. This module performs no network I/O.
use crate::MAX_DECODED_UPLOAD_BYTES;
use anyhow::Result;
use flate2::Compression;
use flate2::write::GzEncoder;
use sentry::protocol::Attachment;
use sentry::protocol::Envelope;
use sentry::protocol::EnvelopeHeaders;
use sentry::protocol::EnvelopeItem;
use std::io;
use std::io::Write;

pub(super) fn encode_envelope(envelope: &Envelope) -> io::Result<(Vec<u8>, usize)> {
    let mut writer = CountingWriter {
        inner: GzEncoder::new(Vec::new(), Compression::fast()),
        bytes: 0,
    };
    envelope.to_writer(&mut writer)?;
    let decoded_bytes = writer.bytes;
    let mut body = writer.inner.finish()?;
    // Already-compressed attachments can grow beyond Sentry's 200 MiB wire limit.
    // Reuse the buffer for the raw envelope when gzip does not make it smaller.
    if body.len() >= decoded_bytes {
        body.clear();
        envelope.to_writer(&mut body)?;
    }
    Ok((body, decoded_bytes))
}

pub(super) fn encode_attachment_envelope(
    headers: &EnvelopeHeaders,
    attachment: Attachment,
) -> Result<Vec<u8>> {
    anyhow::ensure!(
        attachment.buffer.len() <= MAX_DECODED_UPLOAD_BYTES,
        "feedback attachment exceeds the size limit"
    );
    let mut envelope = Envelope::new().with_headers(headers.clone());
    envelope.add_item(EnvelopeItem::Attachment(attachment));
    let (body, decoded_bytes) = encode_envelope(&envelope)?;
    anyhow::ensure!(
        decoded_bytes <= MAX_DECODED_UPLOAD_BYTES,
        "feedback attachment envelope exceeds the size limit"
    );
    Ok(body)
}

struct CountingWriter<W> {
    inner: W,
    bytes: usize,
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(buf)?;
        self.bytes += written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
