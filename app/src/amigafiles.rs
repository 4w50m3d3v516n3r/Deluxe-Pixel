//! The other files of Amiga DeluxePaint (its media folders: BRUSH, COLOURS, MEDIA, MOVE,
//! PATTERNS, TEXTURES), read and written so they can be used here:
//! * palettes (.PAL) and colour sets (.SET): IFF ILBM with BMHD, CMAP, CRNG/DRNG, CAMG and no
//!   BODY;
//! * DRNG colour ranges (DeluxePaint IV/V): cells holding palette registers or fixed colours;
//! * Move settings (.MOVE): IFF form PRSP with a 24-byte MOVE chunk;
//! * media libraries (MEDIA/CHALK, FELTTIP, OIL, WATERCOLOR): Amiga program code, which can't run
//!   here; they are recognised by name and the matching built-in medium is used.

use crate::palette::Range;
use std::path::Path;

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// The chunks of an IFF FORM: (form type, [(id, data)]).
pub fn form_chunks(d: &[u8]) -> Option<([u8; 4], Vec<([u8; 4], &[u8])>)> {
    if d.len() < 12 || &d[0..4] != b"FORM" {
        return None;
    }
    let kind = [d[8], d[9], d[10], d[11]];
    let end = (8 + be32(d, 4) as usize).min(d.len());
    let mut out = Vec::new();
    let mut p = 12;
    while p + 8 <= end {
        let id = [d[p], d[p + 1], d[p + 2], d[p + 3]];
        let n = be32(d, p + 4) as usize;
        let s = (p + 8).min(end);
        out.push((id, &d[s..(s + n).min(end)]));
        p += 8 + n + (n & 1);
    }
    Some((kind, out))
}

/// A CRNG chunk as a range; empty ranges (low = high) are left out.
pub fn parse_crng(c: &[u8]) -> Option<Range> {
    if c.len() < 8 || c[7] <= c[6] {
        return None;
    }
    let mut r = Range::new(c[6], c[7], be16(c, 2).min(16384));
    let flags = be16(c, 4);
    r.active = flags & 1 != 0;
    r.reverse = flags & 2 != 0;
    Some(r)
}

/// A DRNG chunk (DeluxePaint IV/V range): min and max cell, rate, flags, then `ntrue` fixed
/// colours (cell, r, g, b) and `nregs` palette registers (cell, register). The registers become
/// a range from the lowest to the highest one (reversed when they run downwards); a range of
/// fixed colours only has no palette colours to cycle and is left out.
pub fn parse_drng(c: &[u8]) -> Option<Range> {
    if c.len() < 8 {
        return None;
    }
    let rate = be16(c, 2).min(16384);
    let flags = be16(c, 4);
    let (ntrue, nregs) = (c[6] as usize, c[7] as usize);
    let regs_at = 8 + ntrue * 4;
    // (cell = bar position, register = palette colour); fixed true colours are not palette colours.
    let regs: Vec<(u8, u8)> = (0..nregs).filter_map(|i| c.get(regs_at + i * 2..regs_at + i * 2 + 2).map(|b| (b[0], b[1]))).collect();
    if regs.len() < 2 {
        return None;
    }
    let mut r = Range::from_cells(regs, rate);
    if r.high <= r.low {
        return None;
    }
    r.active = flags & 1 != 0;
    Some(r)
}

/// A DRNG chunk for a range with placed colours: min and max position, rate, flags, no fixed
/// colours, then (position, colour) for each placed colour.
pub fn drng_bytes(r: &Range) -> Vec<u8> {
    let cells = r.bar();
    let (min, max) = (cells.first().map(|c| c.0).unwrap_or(0), cells.last().map(|c| c.0).unwrap_or(0));
    let mut v = vec![min, max];
    v.extend_from_slice(&r.rate.to_be_bytes());
    v.extend_from_slice(&((r.active as u16) | 4).to_be_bytes());
    v.push(0);
    v.push(cells.len().min(255) as u8);
    for (p, c) in cells.iter().take(255) {
        v.push(*p);
        v.push(*c);
    }
    v
}

/// Colours and ranges from a palette, colour set or picture file.
pub struct PaletteFile {
    pub colors: Vec<[u8; 3]>,
    pub ranges: Vec<Range>,
    pub camg: Option<u32>,
}

/// Read a palette: IFF (with or without a picture), JASC-PAL text, or any picture's palette.
pub fn read_palette_file(path: &Path) -> Result<PaletteFile, String> {
    let d = std::fs::read(path).map_err(|e| format!("Can't load file: {e}"))?;
    if let Some((kind, chunks)) = form_chunks(&d) {
        if &kind == b"ILBM" || &kind == b"PBM " {
            let mut colors = Vec::new();
            let mut ranges = Vec::new();
            let mut camg = None;
            for (id, c) in chunks {
                match &id {
                    b"CMAP" => colors = c.chunks_exact(3).take(256).map(|v| [v[0], v[1], v[2]]).collect(),
                    b"CRNG" => ranges.extend(parse_crng(c)),
                    b"DRNG" => ranges.extend(parse_drng(c)),
                    b"CAMG" if c.len() >= 4 => camg = Some(be32(c, 0)),
                    _ => {}
                }
            }
            if colors.is_empty() {
                return Err("That file has no colours.".into());
            }
            // Old 4-bit palettes keep their values in the high nibble.
            if colors.iter().all(|c| c.iter().all(|v| v & 15 == 0)) {
                colors.iter_mut().for_each(|c| c.iter_mut().for_each(|v| *v |= *v >> 4));
            }
            return Ok(PaletteFile { colors, ranges, camg });
        }
    }
    if d.starts_with(b"JASC-PAL") {
        let t = String::from_utf8_lossy(&d);
        let colors: Vec<[u8; 3]> = t
            .lines()
            .skip(3)
            .filter_map(|l| {
                let v: Vec<u8> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (v.len() >= 3).then(|| [v[0], v[1], v[2]])
            })
            .take(256)
            .collect();
        return Ok(PaletteFile { colors, ranges: Vec::new(), camg: None });
    }
    match crate::fileio::load(path)? {
        crate::fileio::LoadResult::Indexed(l) => {
            let n = if l.depth > 0 { 1usize << l.depth.min(8) } else { 256 };
            Ok(PaletteFile { colors: l.palette[..n].to_vec(), ranges: l.ranges.unwrap_or_default(), camg: l.camg })
        }
        _ => Err("That picture has no palette.".into()),
    }
}

fn chunk(out: &mut Vec<u8>, id: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(id);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(data);
    if data.len() & 1 == 1 {
        out.push(0);
    }
}

fn form(kind: &[u8; 4], body: Vec<u8>) -> Vec<u8> {
    let mut out = b"FORM".to_vec();
    out.extend_from_slice(&(body.len() as u32 + 4).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend(body);
    out
}

/// Write a palette or colour set as DeluxePaint does: IFF ILBM with an empty BMHD, CMAP,
/// CRNG ranges and the display mode, no picture.
pub fn write_palette_iff(path: &Path, colors: &[[u8; 3]], ranges: &[Range], camg: u32) -> Result<(), String> {
    let mut b = Vec::new();
    let mut bmhd = vec![0u8; 20];
    bmhd[8] = crate::amiga::depth_of(colors.len()) as u8;
    bmhd[14] = 10;
    bmhd[15] = 11;
    chunk(&mut b, b"BMHD", &bmhd);
    let cmap: Vec<u8> = colors.iter().flat_map(|c| c.iter().cloned()).collect();
    chunk(&mut b, b"CMAP", &cmap);
    for r in ranges.iter().take(16) {
        if !r.cells.is_empty() {
            chunk(&mut b, b"DRNG", &drng_bytes(r));
            continue;
        }
        let mut c = vec![0, 0];
        c.extend_from_slice(&r.rate.to_be_bytes());
        c.extend_from_slice(&((r.active as u16) | ((r.reverse as u16) << 1)).to_be_bytes());
        c.push(r.low);
        c.push(r.high);
        chunk(&mut b, b"CRNG", &c);
    }
    if camg != 0 {
        chunk(&mut b, b"CAMG", &camg.to_be_bytes());
    }
    std::fs::write(path, form(b"ILBM", b)).map_err(|e| format!("Can't save file: {e}"))
}

/// Move settings of a DeluxePaint .MOVE file.
#[derive(Clone, Debug, PartialEq)]
pub struct MoveFile {
    pub cyclic: bool,
    /// 1 forward, 0 in place, -1 backward.
    pub record: i32,
    /// Go from here (true) or come to here (false).
    pub go_from: bool,
    pub dist: [i32; 3],
    pub angle: [i32; 3],
    pub count: usize,
    pub slow_in: usize,
    pub slow_out: usize,
}

/// Read a .MOVE file: FORM PRSP holding a MOVE chunk of 12 big-endian words. The distance,
/// angle and count words are clear from the sample files; the first three words (cyclic,
/// record direction, go from / come to) are our reading of them (**estimated**).
pub fn read_move(d: &[u8]) -> Option<MoveFile> {
    let (kind, chunks) = form_chunks(d)?;
    if &kind != b"PRSP" {
        return None;
    }
    let (_, c) = chunks.into_iter().find(|(id, _)| id == b"MOVE")?;
    if c.len() < 24 {
        return None;
    }
    let w = |i: usize| be16(c, i * 2) as i16 as i32;
    Some(MoveFile {
        cyclic: w(0) != 0,
        record: w(1).clamp(-1, 1),
        go_from: w(2) != 0,
        dist: [w(3), w(4), w(5)],
        angle: [w(6), w(7), w(8)],
        count: w(9).max(1) as usize,
        slow_in: w(10).max(0) as usize,
        slow_out: w(11).max(0) as usize,
    })
}

pub fn write_move(m: &MoveFile) -> Vec<u8> {
    let words = [
        m.cyclic as i32,
        m.record,
        m.go_from as i32,
        m.dist[0],
        m.dist[1],
        m.dist[2],
        m.angle[0],
        m.angle[1],
        m.angle[2],
        m.count as i32,
        m.slow_in as i32,
        m.slow_out as i32,
    ];
    let data: Vec<u8> = words.iter().flat_map(|w| (*w as i16).to_be_bytes()).collect();
    let mut b = Vec::new();
    chunk(&mut b, b"MOVE", &data);
    form(b"PRSP", b)
}

/// Which built-in medium a DeluxePaint media library file stands for (by its name).
pub fn media_for_file(path: &Path) -> Option<usize> {
    let name = path.file_stem()?.to_string_lossy().to_lowercase().replace([' ', '-', '_'], "");
    let keys = ["chalk", "felt", "oil", "water"];
    keys.iter().position(|k| name.contains(k))
}

/// Whether a file is Amiga program code (a load file), as media libraries are.
pub fn is_amiga_program(d: &[u8]) -> bool {
    d.len() >= 4 && be32(d, 0) == 0x3F3
}
