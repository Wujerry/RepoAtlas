//! Oversized Codex JSONL records may contain images or tool payloads that are
//! irrelevant to the text index. Stream those fields through Serde's IgnoredAny
//! without allocating them, while still bounding retained JSON and total work.
use crate::{Error, Result};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::{
    cell::Cell,
    fmt,
    io::{self, Read},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) const TEXT_LIMIT: usize = 8 * 1024 * 1024;
const RECORD_LIMIT: usize = 512 * 1024 * 1024;

#[derive(Default)]
struct Budget {
    ignored: Cell<bool>,
    retained: Cell<usize>,
    limit_hit: Cell<bool>,
    newline: Cell<bool>,
}

/// Stop at the physical line boundary, including when JSON is incomplete.
/// Do not read ahead: the projection controls which bytes count toward memory.
struct RecordReader<'a, R> {
    inner: R,
    budget: &'a Budget,
    cancel: &'a AtomicBool,
    consumed: usize,
}

impl<R: Read> Read for RecordReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.budget.newline.get() {
            return Ok(0);
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::other("canceled"));
        }
        let n = self.inner.read(&mut buf[..1])?;
        if n == 0 {
            return Ok(0);
        }
        self.consumed += n;
        if !self.budget.ignored.get() {
            self.budget.retained.set(self.budget.retained.get() + n);
        }
        if self.consumed > RECORD_LIMIT || self.budget.retained.get() > TEXT_LIMIT {
            self.budget.limit_hit.set(true);
            return Err(io::Error::other("session_line_limit"));
        }
        self.budget.newline.set(buf[0] == b'\n');
        Ok(n)
    }
}

#[derive(Clone, Copy)]
enum Location {
    Root,
    Payload,
    Content,
    Other,
}

#[derive(Clone, Copy)]
struct Projection<'a> {
    budget: &'a Budget,
    location: Location,
}

impl<'de> DeserializeSeed<'de> for Projection<'_> {
    type Value = Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        de: D,
    ) -> std::result::Result<Value, D::Error> {
        de.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Projection<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a Codex JSON record")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> std::result::Result<Value, M::Error> {
        let mut result = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            let ignored = match self.location {
                Location::Payload => matches!(
                    key.as_str(),
                    "images" | "encrypted_content" | "arguments" | "output"
                ),
                Location::Content => key == "image_url",
                _ => false,
            };
            if ignored {
                self.budget.ignored.set(true);
                let value = map.next_value::<IgnoredAny>();
                self.budget.ignored.set(false);
                value?;
            } else {
                let location = match (self.location, key.as_str()) {
                    (Location::Root, "payload") => Location::Payload,
                    (Location::Payload, "content") => Location::Content,
                    _ => Location::Other,
                };
                result.insert(key, map.next_value_seed(Projection { location, ..self })?);
            }
        }
        Ok(Value::Object(result))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> std::result::Result<Value, S::Error> {
        let mut result = Vec::new();
        while let Some(value) = seq.next_element_seed(self)? {
            result.push(value);
        }
        Ok(Value::Array(result))
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> std::result::Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> std::result::Result<Value, E> {
        Ok(value.into())
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> std::result::Result<Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> std::result::Result<Value, E> {
        Ok(value.into())
    }

    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Value, E> {
        Ok(Value::Null)
    }
}

/// Replay the bounded prefix already read by the fast path, then stream the rest
/// of this line. A growing file's unfinished final record remains retryable.
pub(super) fn read_large(
    prefix: &[u8],
    remainder: impl Read,
    cancel: &AtomicBool,
) -> Result<Option<Value>> {
    let budget = Budget::default();
    let reader = RecordReader {
        inner: prefix.chain(remainder),
        budget: &budget,
        cancel,
        consumed: 0,
    };
    let mut de = serde_json::Deserializer::from_reader(reader);
    let value = Projection {
        budget: &budget,
        location: Location::Root,
    }
    .deserialize(&mut de)
    .and_then(|value| de.end().map(|()| value));
    match value {
        Ok(value) => Ok(Some(value)),
        Err(_) if cancel.load(Ordering::Relaxed) => Err(Error::msg("canceled")),
        Err(_) if budget.limit_hit.get() => Err(Error::msg("session_line_limit")),
        Err(e) if e.is_eof() && !budget.newline.get() => Ok(None),
        Err(e) if e.is_io() => Err(Error::msg("session_read_failed")),
        Err(_) => Err(Error::msg("session_malformed_records")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn read(text: &str) -> Result<Option<Value>> {
        read_large(text.as_bytes(), io::empty(), &AtomicBool::new(false))
    }

    #[test]
    fn projection_preserves_text_metadata_and_numeric_usage_fields() {
        let record = json!({"type":"response_item","timestamp":"2026-10-09T00:00:00Z","payload":{
            "type":"message","role":"user","content":[
                {"type":"input_image","image_url":"data:image/png;base64,abcdef"},
                {"type":"input_text","text":"中文 \\\" images output"}
            ],"images":[{"url":"escaped\\\"image"}],"info":{"output":42,"ratio":1.5,"ok":true,"none":null}
        }});
        let mut expected = record.clone();
        expected["payload"]
            .as_object_mut()
            .unwrap()
            .remove("images");
        expected["payload"]["content"][0]
            .as_object_mut()
            .unwrap()
            .remove("image_url");
        assert_eq!(read(&record.to_string()).unwrap(), Some(expected));
    }

    #[test]
    fn retained_text_still_has_a_memory_limit() {
        let text = json!({"payload":{"content":"x".repeat(TEXT_LIMIT)}}).to_string();
        assert_eq!(read(&text).unwrap_err().to_string(), "session_line_limit");
    }

    #[test]
    fn physical_lines_and_unfinished_records_do_not_consume_the_next_record() {
        let mut remainder =
            io::Cursor::new(b"\"image\"},\"type\":\"event_msg\"}\r\n{\"next\":true}\n");
        let value = read_large(
            b"{\"payload\":{\"images\":",
            &mut remainder,
            &AtomicBool::new(false),
        )
        .unwrap()
        .unwrap();
        assert_eq!(value, json!({"payload":{},"type":"event_msg"}));
        let mut tail = String::new();
        remainder.read_to_string(&mut tail).unwrap();
        assert_eq!(tail, "{\"next\":true}\n");
        assert!(read("{\"payload\":{\"images\":\"unfinished")
            .unwrap()
            .is_none());
        assert_eq!(
            read("{\"payload\":{\"images\":\"unfinished\n")
                .unwrap_err()
                .to_string(),
            "session_malformed_records"
        );
        assert!(read("{} trailing\n").is_err());
        assert!(read("{\"payload\":{\"images\":invalid}}\n").is_err());
    }

    #[test]
    fn cancel_interrupts_ignored_payloads_and_raw_bytes_remain_bounded() {
        struct CancelOnRead<'a>(&'a AtomicBool);
        impl Read for CancelOnRead<'_> {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                self.0.store(true, Ordering::Relaxed);
                buf[0] = b'x';
                Ok(1)
            }
        }
        let cancel = AtomicBool::new(false);
        assert_eq!(
            read_large(
                b"{\"payload\":{\"images\":\"",
                CancelOnRead(&cancel),
                &cancel
            )
            .unwrap_err()
            .to_string(),
            "canceled"
        );
        let budget = Budget::default();
        budget.ignored.set(true);
        let mut reader = RecordReader {
            inner: &b"x"[..],
            budget: &budget,
            cancel: &AtomicBool::new(false),
            consumed: RECORD_LIMIT,
        };
        assert!(reader.read(&mut [0]).is_err());
        assert!(budget.limit_hit.get());
    }
}
