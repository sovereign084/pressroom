//! The little records Publisher writes its properties in, in the `Contents` stream and in the text
//! stream's formatting: an id, a kind that says how long the value is, and the value. A container's
//! value is a length followed by more records.

use std::ops::Range;

use crate::bytes;

/// One record.
#[derive(Clone, Debug)]
pub struct Block {
    pub id: u8,
    pub kind: u8,
    /// The number, for a two or four byte value.
    pub value: u32,
    /// A container's contents, or a fixed value's bytes.
    pub body: Range<usize>,
    /// Whether it is a container (or a string).
    pub container: bool,
}

/// How many bytes follow a kind's two byte head, or `None` for a container.
fn fixed_len(kind: u8) -> Option<usize> {
    Some(match kind {
        0x80 | 0x82 | 0x88 | 0x8A | 0x90 | 0x98 | 0xA0 | 0xC0 => return None,
        0x07 | 0x10 | 0x12 | 0x18 | 0x1A => 2,
        0x20 | 0x22 | 0x58 | 0x68 | 0x70 | 0xB8 => 4,
        0x28 => 8,
        0x38 => 16,
        0x48 => 24,
        // Flags: being there is the value.
        _ => 0,
    })
}

/// The records of one level in `data[at..end]`. Reading stops, without a panic, where a record
/// doesn't fit.
pub fn blocks(data: &[u8], at: usize, end: usize) -> Vec<Block> {
    let end = end.min(data.len());
    let mut out = Vec::new();
    let mut at = at;
    while let Some(head_end) = at.checked_add(2).filter(|&h| h <= end) {
        let (Some(&id), Some(&kind)) = (data.get(at), data.get(at + 1)) else { break };
        match fixed_len(kind) {
            Some(n) => {
                let Some(stop) = head_end.checked_add(n).filter(|&s| s <= end) else { break };
                let value = match n {
                    2 => bytes::u16_at(data, head_end).map_or(0, u32::from),
                    4 => bytes::u32_at(data, head_end).unwrap_or(0),
                    _ => 0,
                };
                out.push(Block { id, kind, value, body: head_end..stop, container: false });
                at = stop;
            }
            None => {
                let Some(len) = bytes::u32_at(data, head_end).map(|n| n as usize) else { break };
                let Some(stop) = head_end.checked_add(len).filter(|&s| s <= end && len >= 4) else { break };
                out.push(Block { id, kind, value: 0, body: head_end + 4..stop, container: true });
                at = stop;
            }
        }
    }
    out
}

/// The records inside a container.
pub fn children(data: &[u8], b: &Block) -> Vec<Block> {
    blocks(data, b.body.start, b.body.end)
}

pub fn find(list: &[Block], id: u8) -> Option<&Block> {
    list.iter().find(|b| b.id == id)
}

pub fn value(list: &[Block], id: u8) -> Option<u32> {
    find(list, id).map(|b| b.value)
}

pub fn has(list: &[Block], id: u8) -> bool {
    find(list, id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_containers_come_back_with_their_contents() {
        // A two byte value, a flag, a container holding a four byte value.
        let d = [0x01, 0x12, 0x34, 0x12, 0x02, 0x0A, 0x03, 0x88, 10, 0, 0, 0, 0x05, 0x22, 1, 0, 0, 0];
        let list = blocks(&d, 0, d.len());
        assert_eq!(value(&list, 1), Some(0x1234));
        assert!(has(&list, 2));
        let inner = children(&d, find(&list, 3).unwrap());
        assert_eq!(value(&inner, 5), Some(1));
    }

    #[test]
    fn a_record_that_doesnt_fit_ends_the_list() {
        // A container claiming more than there is, and one claiming less than its own length.
        assert!(blocks(&[0x03, 0x88, 200, 0, 0, 0], 0, 6).is_empty());
        assert!(blocks(&[0x03, 0x88, 2, 0, 0, 0], 0, 6).is_empty());
        assert!(blocks(&[0x03, 0x88, 0xFF, 0xFF, 0xFF, 0xFF], 0, 6).is_empty());
        assert!(blocks(&[0x01, 0x22, 1, 0], 0, 4).is_empty());
        assert!(blocks(&[0x01], 0, 1).is_empty());
        assert!(blocks(&[], 5, 3).is_empty());
    }
}
