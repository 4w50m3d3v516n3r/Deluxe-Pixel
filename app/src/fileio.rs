//! Picture files: IFF ILBM / PBM (".lbm", ".bbm"), IFF ANIM (".anim"), PCX, BMP, PNG, and
//! any other common picture format through true-colour conversion.

use crate::convert::Rgba;
use crate::palette::{Palette, Range, DEFAULT_PALETTE};
use crate::pix::Image;
use std::io::Cursor;
use std::path::Path;

pub struct Loaded {
    pub frames: Vec<Image>,
    pub palette: Palette,
    pub ranges: Option<Vec<Range>>,
    pub fps: Option<f32>,
    /// Frames of an animation that bring their own palette.
    pub frame_palettes: Vec<Option<Palette>>,
    /// AnimBrush hotspot and transparent colour, when the file is an AnimBrush.
    pub grab: Option<(i16, i16)>,
    pub transparent: Option<u8>,
    pub frame_delays: Vec<u32>,
    /// Bitplanes of the file (2^depth colours); 0 when the file doesn't say (256 colours).
    pub depth: u32,
    /// Amiga display mode id (CAMG chunk), when the file has one.
    pub camg: Option<u32>,
}

/// How an IFF picture is written: bitplanes and Amiga display mode id (0 = choose from the pixels).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IffFormat {
    pub planes: u32,
    pub camg: u32,
}

pub enum LoadResult {
    Indexed(Loaded),
    TrueColor(Rgba),
}

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

pub fn load(path: &Path) -> Result<LoadResult, String> {
    let data = std::fs::read(path).map_err(|e| format!("Can't load file: {e}"))?;
    if crate::anim::is_anim(&data) {
        let a = crate::anim::decode(&data)?;
        return Ok(LoadResult::Indexed(Loaded {
            frames: a.frames,
            palette: a.palette,
            ranges: if a.ranges.is_empty() { None } else { Some(a.ranges) },
            fps: Some(a.fps),
            frame_palettes: a.frame_palettes,
            grab: a.grab,
            transparent: a.transparent,
            frame_delays: a.frame_delays,
            depth: a.depth,
            camg: a.camg,
        }));
    }
    if data.len() >= 12 && &data[0..4] == b"FORM" {
        return load_iff(&data).map(LoadResult::Indexed);
    }
    if data.len() > 128 && data[0] == 0x0A && data[2] == 1 {
        if let Ok(l) = load_pcx(&data) {
            return Ok(LoadResult::Indexed(l));
        }
    }
    if data.starts_with(b"BM") {
        if let Ok(l) = load_bmp8(&data) {
            return Ok(LoadResult::Indexed(l));
        }
    }
    if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        if let Ok(Some(l)) = load_png_indexed(&data) {
            return Ok(LoadResult::Indexed(l));
        }
    }
    let img = image::load_from_memory(&data).map_err(|e| format!("Not a picture this program can read: {e}"))?;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as i32, rgb.height() as i32);
    let px = rgb.pixels().map(|p| [p[0], p[1], p[2]]).collect();
    Ok(LoadResult::TrueColor(Rgba { w, h, px }))
}

// ---------------------------------------------------------------- IFF ILBM / PBM

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}
fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn unpack_byterun1(src: &[u8], pos: &mut usize, out: &mut Vec<u8>, want: usize) -> Result<(), String> {
    let start = out.len();
    while out.len() - start < want {
        if *pos >= src.len() {
            return Err("Short IFF chunk.".into());
        }
        let n = src[*pos] as i8;
        *pos += 1;
        if n >= 0 {
            let cnt = n as usize + 1;
            if *pos + cnt > src.len() {
                return Err("Mangled IFF file.".into());
            }
            out.extend_from_slice(&src[*pos..*pos + cnt]);
            *pos += cnt;
        } else if n != -128 {
            let cnt = (-(n as i32)) as usize + 1;
            if *pos >= src.len() {
                return Err("Mangled IFF file.".into());
            }
            let b = src[*pos];
            *pos += 1;
            out.extend(std::iter::repeat(b).take(cnt));
        }
    }
    out.truncate(start + want);
    Ok(())
}

pub fn load_iff(d: &[u8]) -> Result<Loaded, String> {
    if d.len() < 12 || &d[0..4] != b"FORM" {
        return Err("Not an IFF file.".into());
    }
    let kind = &d[8..12];
    let pbm = match kind {
        b"ILBM" => false,
        b"PBM " => true,
        _ => return Err("Bad IFF Form.".into()),
    };
    let end = (8 + be32(d, 4) as usize).min(d.len());
    let mut pos = 12;
    let (mut w, mut h, mut planes, mut masking, mut comp) = (0usize, 0usize, 0usize, 0u8, 0u8);
    let mut pal = DEFAULT_PALETTE;
    let mut ranges = Vec::new();
    let mut body: Option<&[u8]> = None;
    let mut camg: Option<u32> = None;
    let mut ncmap = 256usize;
    let mut grab: Option<(i16, i16)> = None;
    let mut transparent_colour = 0u8;
    while pos + 8 <= end {
        let id = &d[pos..pos + 4];
        let len = be32(d, pos + 4) as usize;
        let c = &d[pos + 8..(pos + 8 + len).min(end)];
        match id {
            b"BMHD" if c.len() >= 20 => {
                w = be16(c, 0) as usize;
                h = be16(c, 2) as usize;
                planes = c[8] as usize;
                masking = c[9];
                comp = c[10];
                transparent_colour = be16(c, 12) as u8;
            }
            b"CMAP" => {
                ncmap = c.len() / 3;
                for i in 0..(c.len() / 3).min(256) {
                    pal[i] = [c[i * 3], c[i * 3 + 1], c[i * 3 + 2]];
                }
            }
            // Empty CRNG ranges (low = high) are skipped; DRNG is DeluxePaint IV/V's range.
            b"CRNG" => ranges.extend(crate::amigafiles::parse_crng(c)),
            b"DRNG" => ranges.extend(crate::amigafiles::parse_drng(c)),
            // The brush handle.
            b"GRAB" if c.len() >= 4 => grab = Some((be16(c, 0) as i16, be16(c, 2) as i16)),
            b"BODY" => body = Some(c),
            b"CAMG" if c.len() >= 4 => camg = Some(be32(c, 0)),
            _ => {}
        }
        pos += 8 + len + (len & 1);
    }
    let body = match body {
        Some(b) => b,
        None if w == 0 || h == 0 => {
            return Err("This file holds only colours (a palette or colour set). Load it with Colour > Palette > Load or Colour Set > Load.".into())
        }
        None => return Err("Short IFF chunk.".into()),
    };
    if w == 0 || h == 0 {
        return Err("Mangled IFF file.".into());
    }
    if !pbm && planes > 8 {
        return Err("Only pictures with up to 256 colours can be loaded.".into());
    }
    let mut img = Image::new(w as i32, h as i32, 0);
    let mut bp = 0usize;
    if pbm {
        let row = w + (w & 1);
        for y in 0..h {
            let mut line = Vec::with_capacity(row);
            if comp == 1 {
                unpack_byterun1(body, &mut bp, &mut line, row)?;
            } else {
                line.extend_from_slice(body.get(bp..bp + row).ok_or("Short IFF chunk.")?);
                bp += row;
            }
            img.px[y * w..y * w + w].copy_from_slice(&line[..w]);
        }
    } else {
        let rowbytes = ((w + 15) / 16) * 2;
        let nplanes = planes + if masking == 1 { 1 } else { 0 };
        for y in 0..h {
            let mut line = Vec::with_capacity(rowbytes * nplanes);
            if comp == 1 {
                unpack_byterun1(body, &mut bp, &mut line, rowbytes * nplanes)?;
            } else {
                line.extend_from_slice(body.get(bp..bp + rowbytes * nplanes).ok_or("Short IFF chunk.")?);
                bp += rowbytes * nplanes;
            }
            for x in 0..w {
                let mut v = 0u8;
                for p in 0..planes {
                    let byte = line[p * rowbytes + x / 8];
                    if byte & (0x80 >> (x % 8)) != 0 {
                        v |= 1 << p;
                    }
                }
                img.px[y * w + x] = v;
            }
        }
    }
    let ham = !pbm && camg.map(|c| c & 0x800 != 0).unwrap_or(false) && (planes == 6 || planes == 8);
    let ehb = !pbm && !ham && camg.map(|c| c & 0x80 != 0).unwrap_or(false) && planes == 6;
    // Old 4-bit palettes (values only in the high nibble) are stretched to 8 bits.
    let n = ncmap.min(256);
    if n > 0 && pal[..n].iter().all(|c| c.iter().all(|v| v & 15 == 0)) && pal[..n].iter().any(|c| c.iter().any(|v| *v != 0)) {
        for c in pal.iter_mut().take(n) {
            for v in c.iter_mut() {
                *v |= *v >> 4;
            }
        }
    }
    if ehb {
        let half = crate::anim::ehb_palette(&pal[..32]);
        pal[..64].copy_from_slice(&half[..64]);
    }
    let out_depth = if pbm { 8 } else if ham { 8 } else { planes as u32 };
    if ham {
        // Hold-And-Modify is true colour on the Amiga: reduce it to a 256-colour picture.
        let p = crate::anim::Planar::from_image(&img, planes);
        let rgb = crate::anim::ham_to_rgb(&p, &pal[..1 << (planes - 2)]);
        let shared = crate::convert::median_cut(&rgb.iter().step_by(3).cloned().collect::<Vec<_>>(), 256);
        let mut np = DEFAULT_PALETTE;
        for (i, c) in shared.iter().take(256).enumerate() {
            np[i] = *c;
        }
        pal = np;
        let r = crate::convert::Rgba { w: w as i32, h: h as i32, px: rgb };
        let opts = crate::convert::ConvertOptions { palette: crate::convert::PaletteChoice::Current, method: crate::convert::Method::Closest, ..Default::default() };
        img = crate::convert::reduce(&r, &opts, &pal).0;
    }
    Ok(Loaded { frames: vec![img], palette: pal, ranges: if ranges.is_empty() { None } else { Some(ranges) }, fps: None, frame_palettes: Vec::new(), grab, transparent: if masking == 2 { Some(transparent_colour) } else { None }, frame_delays: Vec::new(), depth: out_depth, camg })
}

fn pack_byterun1(src: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < src.len() {
        // Run of identical bytes?
        let mut run = 1;
        while i + run < src.len() && run < 128 && src[i + run] == src[i] {
            run += 1;
        }
        if run >= 3 {
            out.push((-(run as i32 - 1)) as i8 as u8);
            out.push(src[i]);
            i += run;
            continue;
        }
        // Literal run until the next run of 3.
        let start = i;
        let mut lit = 0;
        while i < src.len() && lit < 128 {
            if i + 2 < src.len() && src[i] == src[i + 1] && src[i] == src[i + 2] {
                break;
            }
            i += 1;
            lit += 1;
        }
        out.push((lit - 1) as u8);
        out.extend_from_slice(&src[start..start + lit]);
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

/// Write an IFF ILBM picture (planar, ByteRun1), readable by Amiga and PC paint programs.
pub fn save_ilbm(path: &Path, img: &Image, pal: &Palette, ranges: &[Range], transparent: Option<u8>) -> Result<(), String> {
    save_ilbm_fmt(path, img, pal, ranges, transparent, IffFormat::default())
}

/// Bitplanes the pixels need.
pub fn planes_for(img: &Image) -> u32 {
    let maxc = img.px.iter().cloned().max().unwrap_or(0) as usize;
    crate::amiga::depth_of(maxc + 1)
}

/// Write an IFF ILBM with a given number of bitplanes and Amiga display mode (CAMG).
pub fn save_ilbm_fmt(path: &Path, img: &Image, pal: &Palette, ranges: &[Range], transparent: Option<u8>, fmt: IffFormat) -> Result<(), String> {
    let w = img.w as usize;
    let h = img.h as usize;
    let planes = fmt.planes.max(planes_for(img)).clamp(1, 8) as usize;
    let mut bmhd = Vec::new();
    bmhd.extend_from_slice(&(w as u16).to_be_bytes());
    bmhd.extend_from_slice(&(h as u16).to_be_bytes());
    bmhd.extend_from_slice(&0i16.to_be_bytes());
    bmhd.extend_from_slice(&0i16.to_be_bytes());
    bmhd.push(planes as u8);
    bmhd.push(if transparent.is_some() { 2 } else { 0 });
    bmhd.push(1);
    bmhd.push(0);
    bmhd.extend_from_slice(&(transparent.unwrap_or(0) as u16).to_be_bytes());
    bmhd.push(1);
    bmhd.push(1);
    bmhd.extend_from_slice(&(w as i16).to_be_bytes());
    bmhd.extend_from_slice(&(h as i16).to_be_bytes());
    let ncol = 1usize << planes;
    let cmap: Vec<u8> = pal.iter().take(ncol).flat_map(|c| c.iter().cloned()).collect();
    let rowbytes = ((w + 15) / 16) * 2;
    let mut body = Vec::new();
    let mut line = vec![0u8; rowbytes];
    for y in 0..h {
        for p in 0..planes {
            line.iter_mut().for_each(|b| *b = 0);
            for x in 0..w {
                if img.px[y * w + x] & (1 << p) != 0 {
                    line[x / 8] |= 0x80 >> (x % 8);
                }
            }
            pack_byterun1(&line, &mut body);
        }
    }
    let mut form = Vec::new();
    form.extend_from_slice(b"ILBM");
    chunk(&mut form, b"BMHD", &bmhd);
    chunk(&mut form, b"CMAP", &cmap);
    if fmt.camg != 0 {
        chunk(&mut form, b"CAMG", &fmt.camg.to_be_bytes());
    }
    for r in ranges.iter().take(16) {
        // Ranges with colours placed on the bar are written as DRNG, as DeluxePaint IV/V does.
        if !r.cells.is_empty() {
            chunk(&mut form, b"DRNG", &crate::amigafiles::drng_bytes(r));
            continue;
        }
        let mut c = Vec::new();
        c.extend_from_slice(&0u16.to_be_bytes());
        c.extend_from_slice(&r.rate.to_be_bytes());
        let flags: u16 = (r.active as u16) | ((r.reverse as u16) << 1);
        c.extend_from_slice(&flags.to_be_bytes());
        c.push(r.low);
        c.push(r.high);
        chunk(&mut form, b"CRNG", &c);
    }
    chunk(&mut form, b"BODY", &body);
    let mut out = Vec::new();
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(form.len() as u32).to_be_bytes());
    out.extend_from_slice(&form);
    std::fs::write(path, out).map_err(|e| format!("Can't save file: {e}"))
}

// ---------------------------------------------------------------- PCX

pub fn load_pcx(d: &[u8]) -> Result<Loaded, String> {
    let bpp = d[3];
    let xmin = u16::from_le_bytes([d[4], d[5]]) as usize;
    let ymin = u16::from_le_bytes([d[6], d[7]]) as usize;
    let xmax = u16::from_le_bytes([d[8], d[9]]) as usize;
    let ymax = u16::from_le_bytes([d[10], d[11]]) as usize;
    let nplanes = d[65] as usize;
    let bpl = u16::from_le_bytes([d[66], d[67]]) as usize;
    let w = xmax - xmin + 1;
    let h = ymax - ymin + 1;
    let mut pal = DEFAULT_PALETTE;
    for i in 0..16 {
        pal[i] = [d[16 + i * 3], d[17 + i * 3], d[18 + i * 3]];
    }
    let total = bpl * nplanes;
    let mut pos = 128;
    let mut img = Image::new(w as i32, h as i32, 0);
    for y in 0..h {
        let mut line = Vec::with_capacity(total);
        while line.len() < total {
            if pos >= d.len() {
                return Err("Short PCX file".into());
            }
            let b = d[pos];
            pos += 1;
            if b & 0xC0 == 0xC0 {
                let n = (b & 0x3F) as usize;
                let v = *d.get(pos).ok_or("Short PCX file")?;
                pos += 1;
                line.extend(std::iter::repeat(v).take(n));
            } else {
                line.push(b);
            }
        }
        for x in 0..w {
            let v = if bpp == 8 && nplanes == 1 {
                line[x]
            } else if bpp == 1 {
                let mut v = 0u8;
                for p in 0..nplanes {
                    if line[p * bpl + x / 8] & (0x80 >> (x % 8)) != 0 {
                        v |= 1 << p;
                    }
                }
                v
            } else {
                return Err("Unsupported PCX layout".into());
            };
            img.px[y * w + x] = v;
        }
    }
    if bpp == 8 && d.len() >= 769 && d[d.len() - 769] == 0x0C {
        let p = &d[d.len() - 768..];
        for i in 0..256 {
            pal[i] = [p[i * 3], p[i * 3 + 1], p[i * 3 + 2]];
        }
    }
    Ok(Loaded { frames: vec![img], palette: pal, ranges: None, fps: None, frame_palettes: Vec::new(), grab: None, transparent: None, frame_delays: Vec::new(), depth: 0, camg: None })
}

pub fn save_pcx(path: &Path, img: &Image, pal: &Palette) -> Result<(), String> {
    let w = img.w as usize;
    let h = img.h as usize;
    let bpl = w + (w & 1);
    let mut out = vec![0u8; 128];
    out[0] = 0x0A;
    out[1] = 5;
    out[2] = 1;
    out[3] = 8;
    out[8..10].copy_from_slice(&((w - 1) as u16).to_le_bytes());
    out[10..12].copy_from_slice(&((h - 1) as u16).to_le_bytes());
    out[12..14].copy_from_slice(&72u16.to_le_bytes());
    out[14..16].copy_from_slice(&72u16.to_le_bytes());
    out[65] = 1;
    out[66..68].copy_from_slice(&(bpl as u16).to_le_bytes());
    out[68] = 1;
    for y in 0..h {
        let mut x = 0;
        let row: Vec<u8> = (0..bpl).map(|i| if i < w { img.px[y * w + i] } else { 0 }).collect();
        while x < bpl {
            let v = row[x];
            let mut n = 1;
            while x + n < bpl && n < 63 && row[x + n] == v {
                n += 1;
            }
            if n > 1 || v & 0xC0 == 0xC0 {
                out.push(0xC0 | n as u8);
            }
            out.push(v);
            x += n;
        }
    }
    out.push(0x0C);
    for c in pal.iter() {
        out.extend_from_slice(c);
    }
    std::fs::write(path, out).map_err(|e| format!("Can't save file: {e}"))
}

// ---------------------------------------------------------------- BMP (8-bit)

pub fn load_bmp8(d: &[u8]) -> Result<Loaded, String> {
    let le32 = |o: usize| u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
    let le16 = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    if d.len() < 54 {
        return Err("short".into());
    }
    let off = le32(10) as usize;
    let hsz = le32(14) as usize;
    let w = le32(18) as i32;
    let hraw = le32(22) as i32;
    let bpp = le16(28);
    let comp = le32(30);
    if (bpp != 8 && bpp != 4) || comp != 0 {
        return Err("not 8-bit".into());
    }
    let h = hraw.abs();
    let mut ncol = le32(46) as usize;
    if ncol == 0 {
        ncol = 1 << bpp;
    }
    let mut pal = DEFAULT_PALETTE;
    let po = 14 + hsz;
    for i in 0..ncol.min(256) {
        let o = po + i * 4;
        if o + 3 > d.len() {
            break;
        }
        pal[i] = [d[o + 2], d[o + 1], d[o]];
    }
    let stride = ((w as usize * bpp as usize + 31) / 32) * 4;
    let mut img = Image::new(w, h, 0);
    for row in 0..h as usize {
        let y = if hraw > 0 { h as usize - 1 - row } else { row };
        let o = off + row * stride;
        for x in 0..w as usize {
            let v = if bpp == 8 {
                *d.get(o + x).ok_or("short")?
            } else {
                let b = *d.get(o + x / 2).ok_or("short")?;
                if x % 2 == 0 { b >> 4 } else { b & 15 }
            };
            img.px[y * w as usize + x] = v;
        }
    }
    Ok(Loaded { frames: vec![img], palette: pal, ranges: None, fps: None, frame_palettes: Vec::new(), grab: None, transparent: None, frame_delays: Vec::new(), depth: 0, camg: None })
}

pub fn save_bmp8(path: &Path, img: &Image, pal: &Palette) -> Result<(), String> {
    let w = img.w as usize;
    let h = img.h as usize;
    let stride = (w + 3) & !3;
    let off = 14 + 40 + 1024;
    let size = off + stride * h;
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(size as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(off as u32).to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&((stride * h) as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&256u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for c in pal.iter() {
        out.extend_from_slice(&[c[2], c[1], c[0], 0]);
    }
    for y in (0..h).rev() {
        out.extend_from_slice(&img.px[y * w..y * w + w]);
        out.extend(std::iter::repeat(0).take(stride - w));
    }
    std::fs::write(path, out).map_err(|e| format!("Can't save file: {e}"))
}

// ---------------------------------------------------------------- PNG

pub fn load_png_indexed(d: &[u8]) -> Result<Option<Loaded>, String> {
    let mut dec = png::Decoder::new(Cursor::new(d));
    dec.set_transformations(png::Transformations::IDENTITY);
    let mut reader = dec.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.color_type != png::ColorType::Indexed {
        return Ok(None);
    }
    let depth = info.bit_depth as u8;
    let (w, h) = (info.width as usize, info.height as usize);
    let mut pal = DEFAULT_PALETTE;
    if let Some(p) = &info.palette {
        for i in 0..(p.len() / 3).min(256) {
            pal[i] = [p[i * 3], p[i * 3 + 1], p[i * 3 + 2]];
        }
    }
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("too big")?];
    let fi = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let mut img = Image::new(w as i32, h as i32, 0);
    for y in 0..h {
        let row = &buf[y * fi.line_size..];
        for x in 0..w {
            let v = match depth {
                8 => row[x],
                4 => (row[x / 2] >> (4 - 4 * (x % 2))) & 15,
                2 => (row[x / 4] >> (6 - 2 * (x % 4))) & 3,
                1 => (row[x / 8] >> (7 - x % 8)) & 1,
                _ => 0,
            };
            img.px[y * w + x] = v;
        }
    }
    Ok(Some(Loaded { frames: vec![img], palette: pal, ranges: None, fps: None, frame_palettes: Vec::new(), grab: None, transparent: None, frame_delays: Vec::new(), depth: 0, camg: None }))
}

pub fn save_png(path: &Path, img: &Image, pal: &Palette, transparent: Option<u8>) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("Can't save file: {e}"))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    let p: Vec<u8> = pal.iter().flat_map(|c| c.iter().cloned()).collect();
    enc.set_palette(p);
    if let Some(t) = transparent {
        let mut trns = vec![255u8; 256];
        trns[t as usize] = 0;
        enc.set_trns(trns);
    }
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(&img.px).map_err(|e| e.to_string())?;
    wr.finish().map_err(|e| e.to_string())
}

/// True-colour export (JPEG, TIFF, TGA...) through the image library.
pub fn save_rgb(path: &Path, img: &Image, pal: &Palette) -> Result<(), String> {
    let mut buf = image::RgbImage::new(img.w as u32, img.h as u32);
    for (i, p) in buf.pixels_mut().enumerate() {
        let c = pal[img.px[i] as usize];
        *p = image::Rgb(c);
    }
    buf.save(path).map_err(|e| format!("Can't save file: {e}"))
}

// ---------------------------------------------------------------- IFF ANIM

/// Save an animation as Deluxe Paint IFF ANIM (operation 5, two loop frames).
pub fn save_anim(path: &Path, frames: &[Image], pal: &Palette, ranges: &[Range], fps: f32, frame_palettes: &[Option<Palette>]) -> Result<(), String> {
    save_anim_timed(path, frames, pal, ranges, fps, frame_palettes, &[])
}

pub fn save_anim_timed(path: &Path, frames: &[Image], pal: &Palette, ranges: &[Range], fps: f32, frame_palettes: &[Option<Palette>], frame_delays: &[u32]) -> Result<(), String> {
    save_anim_fmt(path, frames, pal, ranges, fps, frame_palettes, frame_delays, IffFormat::default())
}

#[allow(clippy::too_many_arguments)]
pub fn save_anim_fmt(path: &Path, frames: &[Image], pal: &Palette, ranges: &[Range], fps: f32, frame_palettes: &[Option<Palette>], frame_delays: &[u32], fmt: IffFormat) -> Result<(), String> {
    let opt = crate::anim::EncodeOptions {
        planes: fmt.planes,
        camg: fmt.camg,
        brush: false,
        grab: (0, 0),
        transparent: None,
        fps,
        ranges: ranges.to_vec(),
        frame_palettes: frame_palettes.to_vec(),
        frame_delays: frame_delays.to_vec(),
    };
    std::fs::write(path, crate::anim::encode(frames, pal, &opt)).map_err(|e| format!("Can't save file: {e}"))
}

/// Save an AnimBrush (IFF ANIM with XOR deltas, interleave 1 and a GRAB hotspot).
pub fn save_anim_brush(path: &Path, cells: &[Image], pal: &Palette, grab: (i16, i16), transparent: Option<u8>, fps: f32) -> Result<(), String> {
    let opt = crate::anim::EncodeOptions { brush: true, grab, transparent, fps, ranges: Vec::new(), frame_palettes: Vec::new(), frame_delays: Vec::new(), planes: 0, camg: 0 };
    std::fs::write(path, crate::anim::encode(cells, pal, &opt)).map_err(|e| format!("Can't save file: {e}"))
}

/// Save by file extension.
pub fn save_any(path: &Path, frames: &[Image], cur: usize, pal: &Palette, ranges: &[Range], fps: f32) -> Result<(), String> {
    save_any_fmt(path, frames, cur, pal, ranges, fps, IffFormat::default())
}

/// Save by file extension; IFF files keep the screen's bitplanes and display mode.
pub fn save_any_fmt(path: &Path, frames: &[Image], cur: usize, pal: &Palette, ranges: &[Range], fps: f32, fmt: IffFormat) -> Result<(), String> {
    match ext(path).as_str() {
        "lbm" | "ilbm" | "iff" | "bbm" | "pic" => save_ilbm_fmt(path, &frames[cur], pal, ranges, None, fmt),
        "anim" => save_anim_fmt(path, frames, pal, ranges, fps, &[], &[], fmt),
        "pcx" | "pcc" => save_pcx(path, &frames[cur], pal),
        "bmp" => save_bmp8(path, &frames[cur], pal),
        "anim" | "anm" => save_anim(path, frames, pal, ranges, fps, &[]),
        "gif" => save_rgb(path, &frames[cur], pal),
        "png" => save_png(path, &frames[cur], pal, None),
        "jpg" | "jpeg" | "tif" | "tiff" | "tga" => save_rgb(path, &frames[cur], pal),
        _ => save_png(path, &frames[cur], pal, None),
    }
}

/// Save each animation frame as its own numbered picture (name0001.png, ...).
pub fn save_frames(path: &Path, frames: &[Image], pal: &Palette, ranges: &[Range]) -> Result<usize, String> {
    save_frames_fmt(path, frames, pal, ranges, IffFormat::default())
}

pub fn save_frames_fmt(path: &Path, frames: &[Image], pal: &Palette, ranges: &[Range], fmt: IffFormat) -> Result<usize, String> {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or("frame".into());
    let e = ext(path);
    let e = if e.is_empty() { "png".to_string() } else { e };
    let dir = path.parent().unwrap_or(Path::new("."));
    for (i, f) in frames.iter().enumerate() {
        let p = dir.join(format!("{stem}{:04}.{e}", i + 1));
        save_any_fmt(&p, std::slice::from_ref(f), 0, pal, ranges, 12.0, fmt)?;
    }
    Ok(frames.len())
}
