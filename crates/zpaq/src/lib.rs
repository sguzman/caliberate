//! ZPAQ archive inspection and unmodeled extraction.

use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::Path;
use tracing::{debug, trace};

const START_TAG_13: [u8; 13] = [
    0x37, 0x6b, 0x53, 0x74, 0xa0, 0x31, 0x83, 0xd3, 0x8c, 0xb2, 0x28, 0xb0, 0xd3,
];
const MAGIC_16: [u8; 16] = [
    0x37, 0x6b, 0x53, 0x74, 0xa0, 0x31, 0x83, 0xd3, 0x8c, 0xb2, 0x28, 0xb0, 0xd3, b'z', b'P', b'Q',
];
const COMP_SIZE: [u8; 10] = [0, 2, 3, 2, 3, 4, 6, 6, 3, 5];

#[derive(Debug, Clone)]
pub struct ZpaqBlockHeader {
    pub start_offset: usize,
    pub level: u8,
    pub zpaql_type: u8,
    pub hsize: u16,
    pub hh: u8,
    pub hm: u8,
    pub ph: u8,
    pub pm: u8,
    pub n_components: u8,
    pub comp_bytes: usize,
    pub hcomp_bytes: usize,
    pub segment_offset: usize,
}

#[derive(Debug, Clone)]
pub struct ZpaqExtractedSegment {
    pub block_index: usize,
    pub filename: String,
    pub comment: String,
    pub data: Vec<u8>,
    pub sha1: Option<[u8; 20]>,
}

pub fn inspect_file(path: &Path) -> CoreResult<Vec<ZpaqBlockHeader>> {
    let data = fs::read(path).map_err(|err| CoreError::Io("read zpaq".to_string(), err))?;
    inspect_bytes(&data)
}

pub fn inspect_bytes(data: &[u8]) -> CoreResult<Vec<ZpaqBlockHeader>> {
    let mut out = Vec::new();
    let mut i = 0usize;

    while i + MAGIC_16.len() + 2 < data.len() {
        let Some(rel) = find_magic(&data[i..]) else {
            break;
        };
        let at = i + rel;

        let Some((block, consumed)) = parse_block_header(data, at)? else {
            i = at + 1;
            continue;
        };

        out.push(block);
        i = at + consumed;
    }

    Ok(out)
}

pub fn archive_is_fully_unmodeled_file(path: &Path) -> CoreResult<bool> {
    let blocks = inspect_file(path)?;
    Ok(!blocks.is_empty() && blocks.iter().all(|b| b.n_components == 0))
}

pub fn extract_unmodeled_file(path: &Path) -> CoreResult<Vec<ZpaqExtractedSegment>> {
    let data = fs::read(path).map_err(|err| CoreError::Io("read zpaq".to_string(), err))?;
    extract_unmodeled_bytes(&data)
}

pub fn extract_unmodeled_bytes(data: &[u8]) -> CoreResult<Vec<ZpaqExtractedSegment>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut block_index = 0usize;

    while i + MAGIC_16.len() + 2 < data.len() {
        let Some(rel) = find_magic(&data[i..]) else {
            break;
        };
        let at = i + rel;

        let Some((header, consumed)) = parse_block_header(data, at)? else {
            i = at + 1;
            continue;
        };

        if header.n_components != 0 {
            return Err(CoreError::ConfigValidate(
                "modeled blocks are not supported yet".to_string(),
            ));
        }

        debug!(
            block = block_index,
            offset = header.start_offset,
            segment_offset = header.segment_offset,
            "extracting unmodeled zpaq block"
        );

        let mut pos = header.segment_offset;
        let mut dec_curr = 0u32;
        let mut pp = PassOrProgramPostProcessor::new(header.ph, header.pm);
        let mut first_segment = true;

        loop {
            let marker = get_required(data, &mut pos, "segment marker")?;
            if marker == 255 {
                break;
            }
            if marker != 1 {
                return Err(corrupt("missing segment or end-of-block marker"));
            }

            let filename = read_cstr(data, &mut pos)?;
            let comment = read_cstr(data, &mut pos)?;
            if get_required(data, &mut pos, "reserved byte")? != 0 {
                return Err(corrupt("missing reserved byte after comment"));
            }

            let mut segment_data = Vec::new();

            if first_segment {
                first_segment = false;
                while (pp.state() & 3) != 1 {
                    let c = decompress_unmodeled_byte(data, &mut pos, &mut dec_curr)?;
                    pp.write(c, &mut segment_data)?;
                }
            }

            loop {
                let c = decompress_unmodeled_byte(data, &mut pos, &mut dec_curr)?;
                pp.write(c, &mut segment_data)?;
                if c < 0 {
                    break;
                }
            }

            let seg_end = get_required(data, &mut pos, "segment end marker")?;
            let sha1 = if seg_end == 254 {
                None
            } else if seg_end == 253 {
                let mut sum = [0u8; 20];
                for b in &mut sum {
                    *b = get_required(data, &mut pos, "sha1 byte")?;
                }
                Some(sum)
            } else {
                return Err(corrupt("missing end-of-segment marker"));
            };

            trace!(
                block = block_index,
                file = filename,
                bytes = segment_data.len(),
                "decoded segment"
            );

            out.push(ZpaqExtractedSegment {
                block_index,
                filename,
                comment,
                data: segment_data,
                sha1,
            });
        }

        block_index += 1;
        i = pos.max(at + consumed);
    }

    Ok(out)
}

fn corrupt(msg: &str) -> CoreError {
    CoreError::ConfigValidate(msg.to_string())
}

fn decompress_unmodeled_byte(data: &[u8], pos: &mut usize, curr: &mut u32) -> CoreResult<i32> {
    if *curr == 0 {
        *curr = read_u32_be(data, pos)?;
        if *curr == 0 {
            return Ok(-1);
        }
    }

    *curr -= 1;
    let b = get_required(data, pos, "compressed payload")?;
    Ok(i32::from(b))
}

fn read_u32_be(data: &[u8], pos: &mut usize) -> CoreResult<u32> {
    let mut x = 0u32;
    for _ in 0..4 {
        x = (x << 8) | u32::from(get_required(data, pos, "u32")?);
    }
    Ok(x)
}

fn read_cstr(data: &[u8], pos: &mut usize) -> CoreResult<String> {
    let mut out = Vec::new();
    loop {
        let c = get_required(data, pos, "cstr")?;
        if c == 0 {
            break;
        }
        out.push(c);
    }
    Ok(String::from_utf8_lossy(&out).into_owned())
}

fn get_required(data: &[u8], pos: &mut usize, what: &'static str) -> CoreResult<u8> {
    if *pos >= data.len() {
        return Err(corrupt(what));
    }
    let b = data[*pos];
    *pos += 1;
    Ok(b)
}

fn find_magic(haystack: &[u8]) -> Option<usize> {
    haystack
        .windows(MAGIC_16.len())
        .position(|w| w == MAGIC_16.as_slice())
}

fn parse_block_header(data: &[u8], at: usize) -> CoreResult<Option<(ZpaqBlockHeader, usize)>> {
    if at + MAGIC_16.len() + 2 > data.len() {
        return Ok(None);
    }

    if data[at..at + START_TAG_13.len()] != START_TAG_13 {
        return Ok(None);
    }

    let mut p = at + MAGIC_16.len();
    let level = data[p];
    p += 1;
    if level != 1 && level != 2 {
        return Ok(None);
    }

    let zpaql_type = data[p];
    p += 1;
    if zpaql_type != 1 {
        return Ok(None);
    }

    if p + 7 > data.len() {
        return Err(corrupt("truncated ZPAQL header prefix"));
    }

    let hsize = u16::from_le_bytes([data[p], data[p + 1]]);
    let hh = data[p + 2];
    let hm = data[p + 3];
    let ph = data[p + 4];
    let pm = data[p + 5];
    let n_components = data[p + 6];

    let header_start = p;
    let header_total = hsize as usize + 2;
    if header_start + header_total > data.len() {
        return Err(corrupt("truncated ZPAQL header"));
    }

    let mut cp = header_start + 7;
    for _ in 0..n_components {
        if cp >= header_start + header_total {
            return Err(corrupt("COMP overflows header"));
        }
        let t = data[cp] as usize;
        if t >= COMP_SIZE.len() || COMP_SIZE[t] == 0 {
            return Err(corrupt("invalid component type"));
        }
        let sz = COMP_SIZE[t] as usize;
        if cp + sz > header_start + header_total {
            return Err(corrupt("component overflows header"));
        }
        cp += sz;
    }

    if cp >= header_start + header_total || data[cp] != 0 {
        return Err(corrupt("missing COMP END"));
    }
    cp += 1;

    let comp_bytes = cp - (header_start + 2);
    if comp_bytes > hsize as usize {
        return Err(corrupt("invalid hsize/COMP layout"));
    }

    let hcomp_bytes = hsize as usize - comp_bytes;
    if hcomp_bytes == 0 {
        return Err(corrupt("missing HCOMP"));
    }

    if data[header_start + header_total - 1] != 0 {
        return Err(corrupt("missing HCOMP END"));
    }

    let segment_offset = header_start + header_total;
    let consumed = (segment_offset - at).max(1);

    Ok(Some((
        ZpaqBlockHeader {
            start_offset: at,
            level,
            zpaql_type,
            hsize,
            hh,
            hm,
            ph,
            pm,
            n_components,
            comp_bytes,
            hcomp_bytes,
            segment_offset,
        },
        consumed,
    )))
}

#[derive(Debug, Clone)]
struct PassOrProgramPostProcessor {
    state: u8,
    program_remaining: usize,
    program_mode: bool,
    _ph: u8,
    _pm: u8,
}

impl PassOrProgramPostProcessor {
    fn new(ph: u8, pm: u8) -> Self {
        Self {
            state: 0,
            program_remaining: 0,
            program_mode: false,
            _ph: ph,
            _pm: pm,
        }
    }

    fn state(&self) -> u8 {
        self.state
    }

    fn write(&mut self, c: i32, out: &mut Vec<u8>) -> CoreResult<()> {
        match self.state {
            0 => {
                if c < 0 {
                    return Err(corrupt("unexpected EOS in postprocessor header"));
                }
                self.state = c as u8 + 1;
                if self.state == 1 || self.state == 2 {
                    Ok(())
                } else {
                    self.program_mode = true;
                    self.program_remaining = (self.state as usize - 2) * 128;
                    self.state = 1;
                    Ok(())
                }
            }
            1 => {
                if self.program_mode {
                    if c < 0 {
                        return Err(corrupt("unexpected EOS in postprocessor"));
                    }
                    out.push(c as u8);
                    self.program_remaining = self.program_remaining.saturating_sub(1);
                    if self.program_remaining == 0 {
                        self.program_mode = false;
                        self.state = 2;
                    }
                    Ok(())
                } else {
                    if c >= 0 {
                        out.push(c as u8);
                    } else {
                        self.state = 2;
                    }
                    Ok(())
                }
            }
            _ => {
                if c >= 0 {
                    out.push(c as u8);
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_unmodeled_archive(filename: &str, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC_16);
        buf.push(2);
        buf.push(1);
        buf.extend_from_slice(&7u16.to_le_bytes());
        buf.extend_from_slice(&[0, 0, 0, 0, 0]);
        buf.push(0);
        buf.push(0);

        buf.push(1);
        buf.extend_from_slice(filename.as_bytes());
        buf.push(0);
        buf.push(0);
        buf.push(0);

        let payload_len = (data.len() + 1) as u32;
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf.push(0);
        buf.extend_from_slice(data);
        buf.extend_from_slice(&0u32.to_be_bytes());

        buf.push(254);
        buf.push(255);

        buf
    }

    #[test]
    fn rejects_non_magic_input() {
        let blocks = inspect_bytes(b"hello world").expect("inspect");
        assert!(blocks.is_empty());
    }

    #[test]
    fn parses_minimal_header() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&MAGIC_16);
        buf.push(2);
        buf.push(1);
        buf.extend_from_slice(&7u16.to_le_bytes());
        buf.extend_from_slice(&[0, 0, 0, 0, 0]);
        buf.push(0);
        buf.push(0);
        buf.push(255);

        let blocks = inspect_bytes(&buf).expect("inspect");
        assert_eq!(blocks.len(), 1);
        let b = &blocks[0];
        assert_eq!(b.level, 2);
        assert_eq!(b.zpaql_type, 1);
        assert_eq!(b.hsize, 7);
        assert_eq!(b.n_components, 0);

        let segments = extract_unmodeled_bytes(&buf).expect("extract");
        assert!(segments.is_empty());
    }

    #[test]
    fn extracts_unmodeled_segment() {
        let buf = build_unmodeled_archive("book.txt", b"hello");
        let segments = extract_unmodeled_bytes(&buf).expect("extract");
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].filename, "book.txt");
        assert_eq!(segments[0].data, b"hello");
    }
}
