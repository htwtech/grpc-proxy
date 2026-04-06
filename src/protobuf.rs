// Minimal protobuf wire format parser for Yellowstone SubscribeRequest.
// Does not depend on prost or yellowstone-grpc-proto.

use std::collections::HashSet;

pub const MAX_BODY_SIZE: usize = 65536; // 64KB max for SubscribeRequest
const MAX_VARINT_BYTES: usize = 10;

/// Read a varint, return (value, bytes_consumed).
pub fn read_varint(buf: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    let mut shift = 0u32;
    let limit = buf.len().min(MAX_VARINT_BYTES);
    for (i, &byte) in buf[..limit].iter().enumerate() {
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
    }
    None
}

/// Skip a field based on wire type.
pub fn skip_field(wire_type: u8, buf: &[u8]) -> Option<usize> {
    match wire_type {
        0 => read_varint(buf).map(|(_, n)| n),
        1 => (buf.len() >= 8).then_some(8),
        2 => {
            let (len, n) = read_varint(buf)?;
            let len = usize::try_from(len).ok()?;
            let total = n.checked_add(len)?;
            (buf.len() >= total).then_some(total)
        }
        5 => (buf.len() >= 4).then_some(4),
        _ => None,
    }
}

/// Extract all length-delimited field values for a given field number.
pub fn extract_len_fields(buf: &[u8], target_field: u32) -> Vec<&[u8]> {
    let mut results = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let Some((tag, tag_len)) = read_varint(&buf[pos..]) else {
            break;
        };
        pos += tag_len;
        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        if field_number == target_field && wire_type == 2 {
            let Some((len, len_bytes)) = read_varint(&buf[pos..]) else {
                break;
            };
            let Some(len) = usize::try_from(len).ok() else { break };
            pos += len_bytes;
            let end = pos.saturating_add(len);
            if end > buf.len() {
                break;
            }
            results.push(&buf[pos..end]);
            pos = end;
        } else {
            let Some(skip) = skip_field(wire_type, &buf[pos..]) else {
                break;
            };
            pos += skip;
        }
    }
    results
}

/// Count repeated string fields with a given field number.
pub fn count_repeated_strings(buf: &[u8], target_field: u32) -> Vec<String> {
    extract_len_fields(buf, target_field)
        .into_iter()
        .filter_map(|b| std::str::from_utf8(b).ok().map(String::from))
        .collect()
}

/// Extract map<string, T> values: protobuf map entries have field 2 = value.
pub fn extract_map_values(buf: &[u8], map_field: u32) -> Vec<Vec<u8>> {
    extract_len_fields(buf, map_field)
        .into_iter()
        .filter_map(|entry| {
            extract_len_fields(entry, 2)
                .into_iter()
                .next()
                .map(|v| v.to_vec())
        })
        .collect()
}

/// Find a string in the reject set.
pub fn has_rejected(strings: &[String], reject: &HashSet<String>) -> Option<String> {
    strings
        .iter()
        .find(|s| reject.contains(s.as_str()))
        .cloned()
}

/// Read a bool varint field with a given field number.
pub fn read_bool_field(buf: &[u8], target_field: u32) -> Option<bool> {
    let mut pos = 0;
    while pos < buf.len() {
        let Some((tag, tag_len)) = read_varint(&buf[pos..]) else {
            break;
        };
        pos += tag_len;
        let field_number = (tag >> 3) as u32;
        let wire_type = (tag & 0x07) as u8;

        if field_number == target_field && wire_type == 0 {
            return read_varint(&buf[pos..]).map(|(val, _)| val != 0);
        }
        let Some(skip) = skip_field(wire_type, &buf[pos..]) else {
            break;
        };
        pos += skip;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_varint() {
        assert_eq!(Some((1, 1)), read_varint(&[0x01]));
        assert_eq!(Some((300, 2)), read_varint(&[0xAC, 0x02]));
        assert_eq!(None, read_varint(&[]));
        assert_eq!(None, read_varint(&[0x80; 11]));
    }
}
