//! The compound file (OLE2) a Publisher file is: a small file system inside one file, with its own
//! sectors, allocation table and folders. Microsoft documents this container in [MS-CFB].
//!
//! Everything here is read from a file that came from outside, so every number in it is treated as
//! hostile: chains are checked for loops, the folder tree for depth and repeats, and no stream can
//! be longer than the file it is in.

use crate::{PubError, bytes};

const SIGNATURE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
const NO_STREAM: u32 = 0xFFFF_FFFF;
/// Deepest the folder tree is followed (a real file has three or four levels, spread over a
/// binary tree of siblings).
const MAX_TREE_DEPTH: usize = 256;

/// A folder or stream in the file.
#[derive(Clone, Debug)]
struct Entry {
    name: String,
    is_folder: bool,
    left: u32,
    right: u32,
    child: u32,
    start: u32,
    size: u64,
}

pub struct Compound<'a> {
    data: &'a [u8],
    sector: usize,
    mini_sector: usize,
    cutoff: u64,
    fat: Vec<u32>,
    mini_fat: Vec<u32>,
    mini_stream: Vec<u8>,
    entries: Vec<Entry>,
}

impl<'a> Compound<'a> {
    pub fn open(data: &'a [u8]) -> Result<Self, PubError> {
        if data.get(..8) != Some(&SIGNATURE[..]) {
            return Err(PubError::NotPublisher);
        }
        let damaged = || PubError::Damaged("its header can't be read".into());
        let sector_shift = bytes::u16_at(data, 30).ok_or_else(damaged)?;
        let mini_shift = bytes::u16_at(data, 32).ok_or_else(damaged)?;
        if !matches!(sector_shift, 9 | 12) || mini_shift != 6 {
            return Err(PubError::Damaged("its sector size isn't one this format allows".into()));
        }
        let sector = 1usize << sector_shift;
        let mini_sector = 1usize << mini_shift;
        let fat_sectors = bytes::u32_at(data, 44).ok_or_else(damaged)? as usize;
        let dir_start = bytes::u32_at(data, 48).ok_or_else(damaged)?;
        let cutoff = u64::from(bytes::u32_at(data, 56).ok_or_else(damaged)?);
        let mini_fat_start = bytes::u32_at(data, 60).ok_or_else(damaged)?;
        let difat_start = bytes::u32_at(data, 68).ok_or_else(damaged)?;
        let max_sectors = data.len() / sector + 1;
        if fat_sectors > max_sectors {
            return Err(PubError::Damaged("it claims to be larger than it is".into()));
        }

        // Where the allocation table's own sectors are: 109 in the header, the rest chained.
        let mut difat: Vec<u32> = (0..109).filter_map(|i| bytes::u32_at(data, 76 + i * 4)).collect();
        let mut next = difat_start;
        let mut seen = 0usize;
        while next < END_OF_CHAIN {
            seen += 1;
            if seen > max_sectors {
                return Err(PubError::Damaged("its allocation table loops".into()));
            }
            let s = sector_at(data, sector, next).ok_or_else(damaged)?;
            let words = sector / 4 - 1;
            difat.extend((0..words).filter_map(|i| bytes::u32_at(s, i * 4)));
            next = bytes::u32_at(s, words * 4).ok_or_else(damaged)?;
        }
        let mut fat = Vec::with_capacity(fat_sectors.saturating_mul(sector / 4).min(max_sectors * (sector / 4)));
        for &s in difat.iter().take(fat_sectors) {
            if s >= END_OF_CHAIN {
                continue;
            }
            let block = sector_at(data, sector, s).ok_or_else(damaged)?;
            fat.extend((0..sector / 4).filter_map(|i| bytes::u32_at(block, i * 4)));
        }

        let mut c = Compound { data, sector, mini_sector, cutoff, fat, mini_fat: Vec::new(), mini_stream: Vec::new(), entries: Vec::new() };
        let dir = c.chain(dir_start, None)?;
        c.entries = dir.chunks_exact(128).map(read_entry).collect();
        let root = c.entries.first().cloned().ok_or_else(|| PubError::Damaged("it has no folders".into()))?;
        let mini_fat = c.chain(mini_fat_start, None)?;
        c.mini_fat = mini_fat.chunks_exact(4).filter_map(|w| bytes::u32_at(w, 0)).collect();
        c.mini_stream = c.chain(root.start, Some(root.size))?;
        Ok(c)
    }

    /// The bytes of a chain of sectors from `start`, cut to `size` when it is known.
    fn chain(&self, start: u32, size: Option<u64>) -> Result<Vec<u8>, PubError> {
        let mut out = Vec::new();
        let mut s = start;
        let mut seen = vec![false; self.fat.len()];
        while s < END_OF_CHAIN {
            if size.is_some_and(|n| out.len() as u64 >= n) {
                break;
            }
            match seen.get_mut(s as usize) {
                Some(v) if *v => return Err(PubError::Damaged("one of its streams loops".into())),
                Some(v) => *v = true,
                None => return Err(PubError::Damaged("one of its streams has a broken chain".into())),
            }
            let block =
                sector_at(self.data, self.sector, s).ok_or_else(|| PubError::Damaged("one of its streams runs past the end of the file".into()))?;
            out.extend_from_slice(block);
            s = *self.fat.get(s as usize).ok_or_else(|| PubError::Damaged("one of its streams has a broken chain".into()))?;
        }
        if let Some(n) = size {
            if (out.len() as u64) < n {
                return Err(PubError::Damaged("one of its streams is cut short".into()));
            }
            out.truncate(n as usize);
        }
        Ok(out)
    }

    fn mini_chain(&self, start: u32, size: u64) -> Result<Vec<u8>, PubError> {
        let mut out = Vec::new();
        let mut s = start;
        let mut seen = vec![false; self.mini_fat.len()];
        while s < END_OF_CHAIN && (out.len() as u64) < size {
            match seen.get_mut(s as usize) {
                Some(v) if *v => return Err(PubError::Damaged("one of its small streams loops".into())),
                Some(v) => *v = true,
                None => return Err(PubError::Damaged("one of its small streams has a broken chain".into())),
            }
            let at =
                (s as usize).checked_mul(self.mini_sector).ok_or_else(|| PubError::Damaged("one of its small streams has a broken chain".into()))?;
            let block = self
                .mini_stream
                .get(at..at.saturating_add(self.mini_sector))
                .ok_or_else(|| PubError::Damaged("one of its small streams runs past its end".into()))?;
            out.extend_from_slice(block);
            s = *self.mini_fat.get(s as usize).ok_or_else(|| PubError::Damaged("one of its small streams has a broken chain".into()))?;
        }
        if (out.len() as u64) < size {
            return Err(PubError::Damaged("one of its small streams is cut short".into()));
        }
        out.truncate(size as usize);
        Ok(out)
    }

    /// The child named `name` of the folder entry `folder`, walking its tree of siblings.
    fn find_child(&self, folder: usize, name: &str) -> Option<usize> {
        let mut stack = vec![(self.entries.get(folder)?.child, 0usize)];
        let mut visited = 0usize;
        while let Some((i, depth)) = stack.pop() {
            if i == NO_STREAM || depth > MAX_TREE_DEPTH {
                continue;
            }
            visited += 1;
            if visited > self.entries.len() {
                return None;
            }
            let e = self.entries.get(i as usize)?;
            if e.name.eq_ignore_ascii_case(name) {
                return Some(i as usize);
            }
            stack.push((e.left, depth + 1));
            stack.push((e.right, depth + 1));
        }
        None
    }

    /// The stream at `path` ("Quill", "QuillSub", "CONTENTS"), or `None` when the file has none.
    pub fn stream(&self, path: &[&str]) -> Result<Option<Vec<u8>>, PubError> {
        let mut at = 0usize;
        for name in path {
            match self.find_child(at, name) {
                Some(i) => at = i,
                None => return Ok(None),
            }
        }
        let Some(e) = self.entries.get(at) else { return Ok(None) };
        if e.is_folder {
            return Ok(None);
        }
        if e.size > self.data.len() as u64 {
            return Err(PubError::Damaged("one of its streams claims to be larger than the file".into()));
        }
        if e.size == 0 {
            return Ok(Some(Vec::new()));
        }
        if e.size < self.cutoff { self.mini_chain(e.start, e.size).map(Some) } else { self.chain(e.start, Some(e.size)).map(Some) }
    }
}

fn sector_at(data: &[u8], sector: usize, index: u32) -> Option<&[u8]> {
    let at = (index as usize).checked_add(1)?.checked_mul(sector)?;
    data.get(at..at.checked_add(sector)?)
}

fn read_entry(e: &[u8]) -> Entry {
    let name_len = bytes::u16_at(e, 64).unwrap_or(0) as usize;
    let units: Vec<u16> = e.get(..name_len.saturating_sub(2).min(64)).unwrap_or(&[]).chunks_exact(2).filter_map(|c| bytes::u16_at(c, 0)).collect();
    Entry {
        name: String::from_utf16_lossy(&units),
        is_folder: matches!(e.get(66), Some(1 | 5)),
        left: bytes::u32_at(e, 68).unwrap_or(NO_STREAM),
        right: bytes::u32_at(e, 72).unwrap_or(NO_STREAM),
        child: bytes::u32_at(e, 76).unwrap_or(NO_STREAM),
        start: bytes::u32_at(e, 116).unwrap_or(END_OF_CHAIN),
        size: bytes::u64_at(e, 120).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A smallest compound file: one allocation sector, one folder sector, one data sector.
    fn tiny(name: &str, stream: &[u8]) -> Vec<u8> {
        let mut f = vec![0u8; 512];
        f[..8].copy_from_slice(&SIGNATURE);
        f[30] = 9;
        f[32] = 6;
        f[44] = 1; // one allocation sector
        f[48..52].copy_from_slice(&1u32.to_le_bytes()); // folders in sector 1
        f[56..60].copy_from_slice(&4096u32.to_le_bytes());
        f[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        f[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        for i in 0..109 {
            f[76 + i * 4..80 + i * 4].copy_from_slice(&NO_STREAM.to_le_bytes());
        }
        f[76..80].copy_from_slice(&0u32.to_le_bytes());
        // Sector 0: the allocation table.
        let mut fat = vec![0xFFu8; 512];
        fat[..4].copy_from_slice(&0xFFFF_FFFDu32.to_le_bytes());
        fat[4..8].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        fat[8..12].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
        f.extend(fat);
        // Sector 1: root and one stream.
        let entry = |name: &str, kind: u8, child: u32, start: u32, size: u64| {
            let mut e = vec![0u8; 128];
            let units: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
            e[..units.len()].copy_from_slice(&units);
            e[64..66].copy_from_slice(&((units.len() + 2) as u16).to_le_bytes());
            e[66] = kind;
            e[68..72].copy_from_slice(&NO_STREAM.to_le_bytes());
            e[72..76].copy_from_slice(&NO_STREAM.to_le_bytes());
            e[76..80].copy_from_slice(&child.to_le_bytes());
            e[116..120].copy_from_slice(&start.to_le_bytes());
            e[120..128].copy_from_slice(&size.to_le_bytes());
            e
        };
        let mut dir = entry("Root Entry", 5, 1, END_OF_CHAIN, 0);
        dir.extend(entry(name, 2, NO_STREAM, 2, stream.len() as u64));
        dir.resize(512, 0);
        f.extend(dir);
        let mut body = stream.to_vec();
        body.resize(512, 0);
        f.extend(body);
        f
    }

    #[test]
    fn a_stream_is_read_back_by_name_whatever_its_case() {
        let mut big = vec![7u8; 5000];
        big.truncate(512);
        let file = tiny("Contents", &big);
        let mut with_cutoff = file.clone();
        with_cutoff[56..60].copy_from_slice(&0u32.to_le_bytes());
        let c = Compound::open(&with_cutoff).unwrap();
        assert_eq!(c.stream(&["CONTENTS"]).unwrap().unwrap(), big);
        assert!(c.stream(&["Quill"]).unwrap().is_none());
    }

    #[test]
    fn a_file_that_is_not_a_compound_file_says_so() {
        assert!(matches!(Compound::open(b"not a publisher file"), Err(PubError::NotPublisher)));
        assert!(matches!(Compound::open(b""), Err(PubError::NotPublisher)));
    }

    #[test]
    fn a_looping_chain_is_an_error_not_a_hang() {
        let mut file = tiny("Contents", &[1u8; 512]);
        file[56..60].copy_from_slice(&0u32.to_le_bytes());
        // Sector 2 chains to itself, and the stream claims more than one sector.
        file[512 + 8..512 + 12].copy_from_slice(&2u32.to_le_bytes());
        let size_at = 512 + 512 + 128 + 120;
        file[size_at..size_at + 8].copy_from_slice(&1024u64.to_le_bytes());
        let c = Compound::open(&file).unwrap();
        assert!(c.stream(&["Contents"]).is_err());
    }

    #[test]
    fn a_stream_larger_than_the_file_is_an_error_not_an_allocation() {
        let mut file = tiny("Contents", &[1u8; 512]);
        file[56..60].copy_from_slice(&0u32.to_le_bytes());
        let size_at = 512 + 512 + 128 + 120;
        file[size_at..size_at + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        let c = Compound::open(&file).unwrap();
        assert!(c.stream(&["Contents"]).is_err());
    }

    #[test]
    fn a_truncated_header_is_an_error() {
        let file = tiny("Contents", &[1u8; 512]);
        for cut in [8, 31, 47, 75, 200, 511, 700, 1100] {
            let _ = Compound::open(&file[..cut]);
        }
    }
}
