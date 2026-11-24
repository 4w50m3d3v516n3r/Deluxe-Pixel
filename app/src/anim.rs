//! Deluxe Paint IFF `ANIM` animations and AnimBrushes (EA IFF 85, big-endian, planar).
//!
//! Reading: `FORM ANIM` of `FORM ILBM` frames. Frame 1 carries BMHD, CMAP and a BODY; later frames
//! carry an ANHD and a DLTA (operation 5 "byte vertical delta"; operations 0, 1, 7 and 8 are read
//! too). Frame k is built on frame k-2 (interleave 0 or 2, double buffering) or k-1 (interleave 1,
//! AnimBrushes, which use XOR deltas). Two trailing loop frames are recognised and dropped.
//! Writing: operation 5, interleave 0 with two loop frames for animations; interleave 1 with XOR
//! deltas and a GRAB hotspot for AnimBrushes.

use crate::palette::{Palette, Range};
use crate::pix::Image;

// ---------------------------------------------------------------- bytes

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}
fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// The chunks inside one FORM's payload (after its 4-byte type), skipping odd-size pad bytes.
pub fn chunks(d: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= d.len() {
        let id = [d[pos], d[pos + 1], d[pos + 2], d[pos + 3]];
        let len = be32(d, pos + 4) as usize;
        let start = pos + 8;
        let end = (start + len).min(d.len());
        out.push((id, &d[start..end]));
        pos = start + len + (len & 1);
    }
    out
}

// ---------------------------------------------------------------- planar bitmaps

#[derive(Clone, Debug, PartialEq)]
pub struct Planar {
    pub w: usize,
    pub h: usize,
    pub bpr: usize,
    pub planes: Vec<Vec<u8>>,
}

pub fn bytes_per_row(w: usize) -> usize {
    ((w + 15) / 16) * 2
}

impl Planar {
    pub fn new(w: usize, h: usize, n: usize) -> Planar {
        let bpr = bytes_per_row(w);
        Planar { w, h, bpr, planes: vec![vec![0u8; bpr * h]; n] }
    }

    pub fn from_image(img: &Image, n: usize) -> Planar {
        let (w, h) = (img.w as usize, img.h as usize);
        let mut p = Planar::new(w, h, n);
        for y in 0..h {
            for x in 0..w {
                let v = img.px[y * w + x];
                for (k, plane) in p.planes.iter_mut().enumerate() {
                    if v & (1 << k) != 0 {
                        plane[y * p.bpr + x / 8] |= 0x80 >> (x % 8);
                    }
                }
            }
        }
        p
    }

    pub fn value(&self, x: usize, y: usize) -> u32 {
        let mut v = 0u32;
        let byte = y * self.bpr + x / 8;
        let bit = 0x80 >> (x % 8);
        for (k, plane) in self.planes.iter().enumerate() {
            if plane[byte] & bit != 0 {
                v |= 1 << k;
            }
        }
        v
    }

    /// Chunky indices (planes beyond 8 are ignored).
    pub fn to_image(&self) -> Image {
        let mut img = Image::new(self.w as i32, self.h as i32, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                img.px[y * self.w + x] = (self.value(x, y) & 0xFF) as u8;
            }
        }
        img
    }
}

// ---------------------------------------------------------------- ByteRun1

pub fn unpack_byterun1(src: &[u8], pos: &mut usize, out: &mut Vec<u8>, want: usize) -> Result<(), String> {
    let start = out.len();
    while out.len() - start < want {
        let n = *src.get(*pos).ok_or("Short IFF chunk.")? as i8;
        *pos += 1;
        if n >= 0 {
            let cnt = n as usize + 1;
            let s = src.get(*pos..*pos + cnt).ok_or("Mangled IFF file.")?;
            out.extend_from_slice(s);
            *pos += cnt;
        } else if n != -128 {
            let cnt = (-(n as i32)) as usize + 1;
            let b = *src.get(*pos).ok_or("Mangled IFF file.")?;
            *pos += 1;
            out.extend(std::iter::repeat(b).take(cnt));
        }
    }
    out.truncate(start + want);
    Ok(())
}

pub fn pack_byterun1(src: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < src.len() {
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

// ---------------------------------------------------------------- headers

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bmhd {
    pub w: u16,
    pub h: u16,
    pub n_planes: u8,
    pub masking: u8,
    pub compression: u8,
    pub transparent: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anhd {
    pub op: u8,
    pub mask: u8,
    pub w: u16,
    pub h: u16,
    pub x: i16,
    pub y: i16,
    pub reltime: u32,
    pub interleave: u8,
    pub bits: u32,
}

impl Anhd {
    fn parse(c: &[u8]) -> Anhd {
        let g = |o: usize| c.get(o).copied().unwrap_or(0);
        let g16 = |o: usize| if c.len() >= o + 2 { be16(c, o) } else { 0 };
        let g32 = |o: usize| if c.len() >= o + 4 { be32(c, o) } else { 0 };
        Anhd {
            op: g(0),
            mask: g(1),
            w: g16(2),
            h: g16(4),
            x: g16(6) as i16,
            y: g16(8) as i16,
            reltime: g32(14),
            interleave: g(18),
            bits: g32(20),
        }
    }
    fn write(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(40);
        v.push(self.op);
        v.push(self.mask);
        v.extend_from_slice(&self.w.to_be_bytes());
        v.extend_from_slice(&self.h.to_be_bytes());
        v.extend_from_slice(&self.x.to_be_bytes());
        v.extend_from_slice(&self.y.to_be_bytes());
        v.extend_from_slice(&0u32.to_be_bytes());
        v.extend_from_slice(&self.reltime.to_be_bytes());
        v.push(self.interleave);
        v.push(0);
        v.extend_from_slice(&self.bits.to_be_bytes());
        v.extend(std::iter::repeat(0).take(16));
        v
    }
    /// Deluxe Paint marks XOR deltas (AnimBrushes) in the bits field.
    pub fn xor(&self) -> bool {
        self.op == 5 && (self.bits & 0x06) != 0
    }
}

pub const CAMG_HAM: u32 = 0x0800;
pub const CAMG_EHB: u32 = 0x0080;

/// One `FORM ILBM` of the animation, as stored.
#[derive(Clone, Debug, Default)]
pub struct FrameRec {
    pub anhd: Option<Anhd>,
    pub cmap: Option<Vec<[u8; 3]>>,
    pub body: Option<Vec<u8>>,
    pub dlta: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Default)]
pub struct AnimFile {
    pub bmhd: Bmhd,
    pub camg: u32,
    pub dpan_frames: Option<u16>,
    pub grab: Option<(i16, i16)>,
    pub ranges: Vec<Range>,
    pub forms: Vec<FrameRec>,
}

fn parse_cmap(c: &[u8]) -> Vec<[u8; 3]> {
    (0..c.len() / 3).map(|i| [c[i * 3], c[i * 3 + 1], c[i * 3 + 2]]).collect()
}

/// Read the structure of a `FORM ANIM` file.
pub fn parse(d: &[u8]) -> Result<AnimFile, String> {
    if d.len() < 12 || &d[0..4] != b"FORM" || &d[8..12] != b"ANIM" {
        return Err("Not an IFF ANIM file.".into());
    }
    let end = (8 + be32(d, 4) as usize).min(d.len());
    let mut a = AnimFile::default();
    let mut have_bmhd = false;
    for (id, payload) in chunks(&d[12..end]) {
        if &id != b"FORM" || payload.len() < 4 || &payload[0..4] != b"ILBM" {
            continue;
        }
        let mut f = FrameRec::default();
        for (cid, c) in chunks(&payload[4..]) {
            match &cid {
                b"BMHD" if c.len() >= 20 && !have_bmhd => {
                    a.bmhd = Bmhd { w: be16(c, 0), h: be16(c, 2), n_planes: c[8], masking: c[9], compression: c[10], transparent: be16(c, 12) };
                    have_bmhd = true;
                }
                b"CMAP" => f.cmap = Some(parse_cmap(c)),
                b"CAMG" if c.len() >= 4 => a.camg = be32(c, 0),
                b"DPAN" if c.len() >= 4 => a.dpan_frames = Some(be16(c, 2)),
                b"GRAB" if c.len() >= 4 => a.grab = Some((be16(c, 0) as i16, be16(c, 2) as i16)),
                b"CRNG" if c.len() >= 8 => {
                    let mut r = Range::new(c[6], c[7], be16(c, 2).min(16384));
                    let flags = be16(c, 4);
                    r.active = flags & 1 != 0;
                    r.reverse = flags & 2 != 0;
                    if a.forms.is_empty() {
                        a.ranges.push(r);
                    }
                }
                b"ANHD" => f.anhd = Some(Anhd::parse(c)),
                b"BODY" => f.body = Some(c.to_vec()),
                b"DLTA" => f.dlta = Some(c.to_vec()),
                _ => {}
            }
        }
        a.forms.push(f);
    }
    if !have_bmhd || a.forms.is_empty() {
        return Err("Mangled IFF ANIM file (no BMHD).".into());
    }
    if a.bmhd.w == 0 || a.bmhd.h == 0 {
        return Err("Mangled IFF ANIM file.".into());
    }
    Ok(a)
}

// ---------------------------------------------------------------- decoding

/// Decode a full BODY (raw or ByteRun1; an interleaved mask plane is skipped).
pub fn decode_body(body: &[u8], bmhd: &Bmhd) -> Result<Planar, String> {
    let n = bmhd.n_planes as usize;
    let mut p = Planar::new(bmhd.w as usize, bmhd.h as usize, n);
    let total = n + if bmhd.masking == 1 { 1 } else { 0 };
    let bpr = p.bpr;
    let mut pos = 0usize;
    let mut line = Vec::with_capacity(bpr * total);
    for y in 0..p.h {
        line.clear();
        if bmhd.compression == 1 {
            unpack_byterun1(body, &mut pos, &mut line, bpr * total)?;
        } else {
            line.extend_from_slice(body.get(pos..pos + bpr * total).ok_or("Short IFF chunk.")?);
            pos += bpr * total;
        }
        for k in 0..n {
            p.planes[k][y * bpr..(y + 1) * bpr].copy_from_slice(&line[k * bpr..(k + 1) * bpr]);
        }
    }
    Ok(p)
}

fn rd<const N: usize>(d: &[u8], pos: &mut usize) -> Option<u32> {
    let s = d.get(*pos..*pos + N)?;
    *pos += N;
    Some(s.iter().fold(0u32, |a, b| (a << 8) | *b as u32))
}

/// Operation 5: byte vertical delta (XOR when Deluxe Paint marked it so).
fn apply_op5(p: &mut Planar, d: &[u8], xor: bool) {
    let bpr = p.bpr;
    let n = p.planes.len().min(16);
    for k in 0..n {
        if d.len() < 64 {
            return;
        }
        let off = be32(d, k * 4) as usize;
        if off == 0 || off >= d.len() {
            continue;
        }
        let plane = &mut p.planes[k];
        let mut pos = off;
        for col in 0..bpr {
            let Some(op_count) = rd::<1>(d, &mut pos) else { return };
            let mut dest = col;
            for _ in 0..op_count {
                let Some(op) = rd::<1>(d, &mut pos) else { return };
                let mut put = |dest: &mut usize, v: u8| {
                    if let Some(b) = plane.get_mut(*dest) {
                        if xor {
                            *b ^= v
                        } else {
                            *b = v
                        }
                    }
                    *dest += bpr;
                };
                if op == 0 {
                    let (Some(cnt), Some(v)) = (rd::<1>(d, &mut pos), rd::<1>(d, &mut pos)) else { return };
                    for _ in 0..cnt {
                        put(&mut dest, v as u8);
                    }
                } else if op & 0x80 != 0 {
                    for _ in 0..(op & 0x7F) {
                        let Some(v) = rd::<1>(d, &mut pos) else { return };
                        put(&mut dest, v as u8);
                    }
                } else {
                    dest += op as usize * bpr;
                }
            }
        }
    }
}

fn put_word(plane: &mut [u8], at: usize, v: u32, size: usize) {
    for i in 0..size {
        if let Some(b) = plane.get_mut(at + i) {
            *b = (v >> (8 * (size - 1 - i))) as u8;
        }
    }
}

/// Operation 7: opcodes and data in separate streams, columns of 2 or 4 bytes.
fn apply_op7(p: &mut Planar, d: &[u8], long: bool) {
    if d.len() < 64 {
        return;
    }
    let ws = if long { 4 } else { 2 };
    let bpr = p.bpr;
    for k in 0..p.planes.len().min(8) {
        let opo = be32(d, k * 4) as usize;
        let dto = be32(d, 32 + k * 4) as usize;
        if opo == 0 || opo >= d.len() {
            continue;
        }
        let (mut op_pos, mut dt_pos) = (opo, dto);
        let plane = &mut p.planes[k];
        for col in 0..bpr / ws {
            let Some(op_count) = rd::<1>(d, &mut op_pos) else { return };
            let mut dest = col * ws;
            for _ in 0..op_count {
                let Some(op) = rd::<1>(d, &mut op_pos) else { return };
                let mut read_val = |pos: &mut usize| if long { rd::<4>(d, pos) } else { rd::<2>(d, pos) };
                if op == 0 {
                    let Some(cnt) = rd::<1>(d, &mut op_pos) else { return };
                    let Some(v) = read_val(&mut dt_pos) else { return };
                    for _ in 0..cnt {
                        put_word(plane, dest, v, ws);
                        dest += bpr;
                    }
                } else if op & 0x80 != 0 {
                    for _ in 0..(op & 0x7F) {
                        let Some(v) = read_val(&mut dt_pos) else { return };
                        put_word(plane, dest, v, ws);
                        dest += bpr;
                    }
                } else {
                    dest += op as usize * bpr;
                }
            }
        }
    }
}

/// Operation 8: like operation 5 with 16- or 32-bit columns, counts and values.
fn apply_op8(p: &mut Planar, d: &[u8], long: bool) {
    if d.len() < 64 {
        return;
    }
    let bpr = p.bpr;
    for k in 0..p.planes.len().min(8) {
        let off = be32(d, k * 4) as usize;
        if off == 0 || off >= d.len() {
            continue;
        }
        let mut pos = off;
        let plane = &mut p.planes[k];
        let mut col = 0usize;
        while col < bpr {
            let ws = if long && col + 4 <= bpr { 4 } else { 2 };
            let read = |pos: &mut usize| if ws == 4 { rd::<4>(d, pos) } else { rd::<2>(d, pos) };
            let uniq: u32 = if ws == 4 { 0x8000_0000 } else { 0x8000 };
            let Some(op_count) = read(&mut pos) else { return };
            let mut dest = col;
            for _ in 0..op_count {
                let Some(op) = read(&mut pos) else { return };
                if op == 0 {
                    let (Some(cnt), Some(v)) = (read(&mut pos), read(&mut pos)) else { return };
                    for _ in 0..cnt {
                        put_word(plane, dest, v, ws);
                        dest += bpr;
                    }
                } else if op & uniq != 0 {
                    for _ in 0..(op & !uniq) {
                        let Some(v) = read(&mut pos) else { return };
                        put_word(plane, dest, v, ws);
                        dest += bpr;
                    }
                } else {
                    dest += op as usize * bpr;
                }
            }
            col += ws;
        }
    }
}

/// Operation 1: a (ByteRun1) BODY of a box, XORed onto the planes chosen by ANHD.mask.
fn apply_op1(p: &mut Planar, data: &[u8], a: &Anhd, compressed: bool) -> Result<(), String> {
    let (bw, bh) = (a.w as usize, a.h as usize);
    let planes: Vec<usize> = (0..p.planes.len()).filter(|k| a.mask & (1 << k) != 0).collect();
    let bbpr = bytes_per_row(bw);
    let mut pos = 0usize;
    let mut line = Vec::new();
    for yy in 0..bh {
        line.clear();
        let want = bbpr * planes.len();
        if compressed {
            unpack_byterun1(data, &mut pos, &mut line, want)?;
        } else {
            line.extend_from_slice(data.get(pos..pos + want).ok_or("Short IFF chunk.")?);
            pos += want;
        }
        for (pi, &k) in planes.iter().enumerate() {
            for xx in 0..bw {
                if line[pi * bbpr + xx / 8] & (0x80 >> (xx % 8)) != 0 {
                    let x = a.x as i64 + xx as i64;
                    let y = a.y as i64 + yy as i64;
                    if x >= 0 && y >= 0 && (x as usize) < p.w && (y as usize) < p.h {
                        p.planes[k][y as usize * p.bpr + x as usize / 8] ^= 0x80 >> (x as usize % 8);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Decode every stored `FORM ILBM` (loop frames included), each with the palette active on it.
pub fn decode_forms(a: &AnimFile) -> Result<Vec<(Planar, Vec<[u8; 3]>)>, String> {
    let mut out: Vec<(Planar, Vec<[u8; 3]>)> = Vec::with_capacity(a.forms.len());
    let mut pal: Vec<[u8; 3]> = Vec::new();
    for (k, f) in a.forms.iter().enumerate() {
        if let Some(c) = &f.cmap {
            pal = c.clone();
        }
        let p = if k == 0 {
            let body = f.body.as_ref().ok_or("The first frame has no BODY.")?;
            decode_body(body, &a.bmhd)?
        } else {
            let anhd = f.anhd.unwrap_or_default();
            let il = match anhd.interleave {
                0 => 2,
                n => n as usize,
            };
            let mut p = if k >= il { out[k - il].0.clone() } else { out[0].0.clone() };
            match anhd.op {
                0 => {
                    if let Some(b) = &f.body {
                        p = decode_body(b, &a.bmhd)?;
                    }
                }
                1 => {
                    if let Some(b) = f.body.as_ref().or(f.dlta.as_ref()) {
                        apply_op1(&mut p, b, &anhd, a.bmhd.compression == 1)?;
                    }
                }
                5 => {
                    if let Some(d) = &f.dlta {
                        apply_op5(&mut p, d, anhd.xor());
                    } else if let Some(b) = &f.body {
                        p = decode_body(b, &a.bmhd)?;
                    }
                }
                7 => {
                    if let Some(d) = &f.dlta {
                        apply_op7(&mut p, d, anhd.bits & 1 == 1);
                    }
                }
                8 => {
                    if let Some(d) = &f.dlta {
                        apply_op8(&mut p, d, anhd.bits & 1 == 1);
                    }
                }
                op => return Err(format!("Animation compression method {op} is not supported.")),
            }
            p
        };
        out.push((p, pal.clone()));
    }
    Ok(out)
}

/// Two trailing loop frames repeat frames 1 and 2.
pub fn has_loop_frames(forms: &[(Planar, Vec<[u8; 3]>)]) -> bool {
    let n = forms.len();
    n >= 4 && forms[n - 2].0 == forms[0].0 && forms[n - 1].0 == forms[1].0
}

/// Old 4-bit palettes store each component in the high nibble only; stretch them to 8 bits.
pub fn fix_palette(pal: &[[u8; 3]]) -> Vec<[u8; 3]> {
    let four_bit = !pal.is_empty() && pal.len() <= 32 && pal.iter().all(|c| c.iter().all(|v| v & 0x0F == 0)) && pal.iter().any(|c| c.iter().any(|v| *v != 0));
    pal.iter().map(|c| if four_bit { [c[0] | c[0] >> 4, c[1] | c[1] >> 4, c[2] | c[2] >> 4] } else { *c }).collect()
}

/// Extra Half-Brite: 64 colours, entries 32..63 are entries 0..31 at half brightness.
pub fn ehb_palette(pal: &[[u8; 3]]) -> Vec<[u8; 3]> {
    let mut p: Vec<[u8; 3]> = pal.iter().take(32).cloned().collect();
    p.resize(32, [0, 0, 0]);
    for i in 0..32 {
        let c = p[i];
        p.push([c[0] >> 1, c[1] >> 1, c[2] >> 1]);
    }
    p
}

/// Hold-And-Modify (HAM6 with 6 planes, HAM8 with 8) to RGB.
pub fn ham_to_rgb(p: &Planar, pal: &[[u8; 3]]) -> Vec<[u8; 3]> {
    let n = p.planes.len();
    let vb = n.saturating_sub(2).max(1);
    let vmask = (1u32 << vb) - 1;
    let scale = |v: u32| -> u8 {
        if vb == 4 {
            (v * 17) as u8
        } else {
            ((v << (8 - vb)) | (v >> (2 * vb - 8).max(0))) as u8
        }
    };
    let mut out = vec![[0u8; 3]; p.w * p.h];
    for y in 0..p.h {
        let mut c = pal.first().copied().unwrap_or([0, 0, 0]);
        for x in 0..p.w {
            let v = p.value(x, y);
            let ctrl = v >> vb;
            let val = v & vmask;
            match ctrl {
                0 => c = pal.get(val as usize).copied().unwrap_or([0, 0, 0]),
                1 => c[2] = scale(val),
                2 => c[0] = scale(val),
                _ => c[1] = scale(val),
            }
            out[y * p.w + x] = c;
        }
    }
    out
}

/// A decoded animation ready for the program.
pub struct Decoded {
    pub frames: Vec<Image>,
    pub palette: Palette,
    /// Frames that bring their own palette (CMAP inside the frame).
    pub frame_palettes: Vec<Option<Palette>>,
    pub ranges: Vec<Range>,
    pub fps: f32,
    pub grab: Option<(i16, i16)>,
    pub transparent: Option<u8>,
    pub loop_frames: bool,
    pub interleave: u8,
    /// Display time of each frame in 1/60 s (0 = the global rate), when frames differ.
    pub frame_delays: Vec<u32>,
    /// Bitplanes of the file (8 for HAM, which is turned into 256 colours).
    pub depth: u32,
    pub camg: Option<u32>,
}

fn to_palette(p: &[[u8; 3]]) -> Palette {
    let mut out = crate::palette::DEFAULT_PALETTE;
    for (i, c) in p.iter().take(256).enumerate() {
        out[i] = *c;
    }
    out
}

pub fn decode(d: &[u8]) -> Result<Decoded, String> {
    let a = parse(d)?;
    let forms = decode_forms(&a)?;
    let loop_frames = has_loop_frames(&forms);
    let n = if loop_frames { forms.len() - 2 } else { forms.len() };
    let ham = a.camg & CAMG_HAM != 0 && (a.bmhd.n_planes == 6 || a.bmhd.n_planes == 8);
    let ehb = !ham && a.camg & CAMG_EHB != 0 && a.bmhd.n_planes == 6;
    let expand = |p: &[[u8; 3]]| -> Vec<[u8; 3]> {
        let f = fix_palette(p);
        if ehb {
            ehb_palette(&f)
        } else {
            f
        }
    };
    let base_pal = expand(&forms[0].1);
    let mut frames = Vec::with_capacity(n);
    let mut frame_palettes = vec![None; n];
    let palette;
    if ham {
        // Hold-And-Modify pictures are true colour: reduce them to one shared 256-colour palette.
        let rgb: Vec<Vec<[u8; 3]>> = forms[..n].iter().map(|(p, pal)| ham_to_rgb(p, &fix_palette(pal))).collect();
        let all: Vec<[u8; 3]> = rgb.iter().flat_map(|f| f.iter().step_by(3).cloned()).collect();
        let shared = crate::convert::median_cut(&all, 256);
        palette = to_palette(&shared);
        let opts = crate::convert::ConvertOptions {
            palette: crate::convert::PaletteChoice::Current,
            method: crate::convert::Method::Closest,
            ..Default::default()
        };
        for f in rgb {
            let r = crate::convert::Rgba { w: a.bmhd.w as i32, h: a.bmhd.h as i32, px: f };
            frames.push(crate::convert::reduce(&r, &opts, &palette).0);
        }
    } else {
        palette = to_palette(&base_pal);
        for (k, (p, _)) in forms[..n].iter().enumerate() {
            frames.push(p.to_image());
            if k > 0 && a.forms[k].cmap.is_some() {
                frame_palettes[k] = Some(to_palette(&expand(&forms[k].1)));
            }
        }
    }
    let reltime = a.forms.get(1).and_then(|f| f.anhd).map(|h| h.reltime).filter(|r| *r > 0).unwrap_or(5);
    let interleave = a.forms.get(1).and_then(|f| f.anhd).map(|h| h.interleave).unwrap_or(0);
    // ANHD.reltime of a frame is how long the frame before it stays on screen.
    let times: Vec<u32> = (0..n).map(|k| a.forms.get(k + 1).and_then(|f| f.anhd).map(|h| h.reltime).filter(|r| *r > 0).unwrap_or(reltime)).collect();
    let frame_delays = if times.iter().all(|t| *t == reltime) { vec![0; n] } else { times };
    Ok(Decoded {
        frames,
        palette,
        frame_palettes,
        ranges: a.ranges.clone(),
        fps: 60.0 / reltime as f32,
        grab: a.grab,
        transparent: if a.bmhd.masking == 2 || a.grab.is_some() { Some(a.bmhd.transparent as u8) } else { None },
        loop_frames,
        interleave,
        frame_delays,
        depth: if ham { 8 } else { a.bmhd.n_planes as u32 },
        camg: if a.camg != 0 { Some(a.camg) } else { None },
    })
}

/// Playback order for `cycles` passes over N frames with loop frames: frames 1..N on the first
/// pass; afterwards the loop frames bring the buffers back to frames 1 and 2 and play goes on at 3.
pub fn play_sequence(n: usize, cycles: usize) -> Vec<usize> {
    let mut seq: Vec<usize> = (0..n).collect();
    for _ in 1..cycles {
        seq.push(0);
        if n > 1 {
            seq.push(1);
        }
        seq.extend(2..n);
    }
    seq
}

// ---------------------------------------------------------------- encoding

/// One column of an operation-5 delta. Returns the encoded ops (opCount first), or None when the
/// column doesn't change.
fn encode_column(old: &[u8], new: &[u8], xor: bool) -> Option<Vec<u8>> {
    let h = new.len();
    let vals: Vec<u8> = if xor { old.iter().zip(new).map(|(a, b)| a ^ b).collect() } else { new.to_vec() };
    let changed: Vec<bool> = (0..h).map(|i| old[i] != new[i]).collect();
    let first = changed.iter().position(|c| *c)?;
    let last = changed.iter().rposition(|c| *c).unwrap();
    let mut ops: Vec<Vec<u8>> = Vec::new();
    let mut i = 0usize;
    while i <= last {
        // Skip unchanged rows.
        let mut skip = 0usize;
        while i + skip <= last && !changed[i + skip] {
            skip += 1;
        }
        while skip > 0 {
            let s = skip.min(127);
            ops.push(vec![s as u8]);
            skip -= s;
            i += s;
        }
        if i > last {
            break;
        }
        // The changed stretch (single unchanged rows inside it are written over too).
        let mut j = i;
        while j <= last && (changed[j] || (j + 1 <= last && changed[j + 1])) {
            j += 1;
        }
        let seg = &vals[i..j];
        let mut k = 0usize;
        let mut lit: Vec<u8> = Vec::new();
        let flush = |lit: &mut Vec<u8>, ops: &mut Vec<Vec<u8>>| {
            for chunk in lit.chunks(127) {
                let mut o = vec![0x80 | chunk.len() as u8];
                o.extend_from_slice(chunk);
                ops.push(o);
            }
            lit.clear();
        };
        while k < seg.len() {
            let mut run = 1;
            while k + run < seg.len() && run < 255 && seg[k + run] == seg[k] {
                run += 1;
            }
            if run >= 3 {
                flush(&mut lit, &mut ops);
                ops.push(vec![0, run as u8, seg[k]]);
                k += run;
            } else {
                lit.push(seg[k]);
                k += 1;
            }
        }
        flush(&mut lit, &mut ops);
        i = j;
    }
    if ops.len() > 255 {
        // Too many commands for one column: one skip, then literals to the last change.
        ops.clear();
        let mut s = first;
        while s > 0 {
            let n = s.min(127);
            ops.push(vec![n as u8]);
            s -= n;
        }
        for chunk in vals[first..=last].chunks(127) {
            let mut o = vec![0x80 | chunk.len() as u8];
            o.extend_from_slice(chunk);
            ops.push(o);
        }
    }
    let mut out = vec![ops.len() as u8];
    for o in ops {
        out.extend(o);
    }
    Some(out)
}

/// An operation-5 DLTA turning `old` into `new`.
pub fn encode_dlta(old: &Planar, new: &Planar, xor: bool) -> Vec<u8> {
    let mut out = vec![0u8; 64];
    let bpr = new.bpr;
    for k in 0..new.planes.len().min(16) {
        let mut stream = Vec::new();
        let mut any = false;
        for col in 0..bpr {
            let o: Vec<u8> = (0..new.h).map(|y| old.planes[k][y * bpr + col]).collect();
            let n: Vec<u8> = (0..new.h).map(|y| new.planes[k][y * bpr + col]).collect();
            match encode_column(&o, &n, xor) {
                Some(c) => {
                    any = true;
                    stream.extend(c);
                }
                None => stream.push(0),
            }
        }
        if any {
            let off = out.len() as u32;
            out[k * 4..k * 4 + 4].copy_from_slice(&off.to_be_bytes());
            out.extend(stream);
        }
    }
    out
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
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend(body);
    out
}

pub struct EncodeOptions {
    /// AnimBrush: interleave 1 with XOR deltas and a GRAB hotspot, no loop frames.
    pub brush: bool,
    pub grab: (i16, i16),
    pub transparent: Option<u8>,
    pub fps: f32,
    pub ranges: Vec<Range>,
    pub frame_palettes: Vec<Option<Palette>>,
    /// Per-frame display time in 1/60 s (0 or missing = the global rate).
    pub frame_delays: Vec<u32>,
    /// Bitplanes to write (0 = as few as the pixels need) and the Amiga display mode id (0 = none).
    pub planes: u32,
    pub camg: u32,
}

pub fn planes_needed(frames: &[Image]) -> usize {
    let maxc = frames.iter().flat_map(|f| f.px.iter()).cloned().max().unwrap_or(0) as usize;
    let mut n = 1;
    while (1usize << n) <= maxc {
        n += 1;
    }
    n.min(8)
}

/// Write an IFF ANIM (or AnimBrush).
pub fn encode(frames: &[Image], pal: &Palette, opt: &EncodeOptions) -> Vec<u8> {
    let n_planes = planes_needed(frames).max(opt.planes as usize).min(8);
    let (w, h) = (frames[0].w as u16, frames[0].h as u16);
    let planar: Vec<Planar> = frames.iter().map(|f| Planar::from_image(f, n_planes)).collect();
    let nf = frames.len();
    // Playback sequence: the frames, then two loop frames for animations of 2 frames or more.
    let mut seq: Vec<usize> = (0..nf).collect();
    if !opt.brush && nf >= 2 {
        seq.push(0);
        seq.push(1);
    }
    let reltime = ((60.0 / opt.fps.max(0.5)).round() as u32).max(1);
    let ncol = 1usize << n_planes;
    let cmap_of = |p: &Palette| -> Vec<u8> { p.iter().take(ncol).flat_map(|c| c.iter().cloned()).collect() };
    let interleave: usize = if opt.brush { 1 } else { 2 };
    let mut body = Vec::new();
    for (k, &fi) in seq.iter().enumerate() {
        let mut f = Vec::new();
        if k == 0 {
            let mut bmhd = Vec::new();
            bmhd.extend_from_slice(&w.to_be_bytes());
            bmhd.extend_from_slice(&h.to_be_bytes());
            bmhd.extend_from_slice(&0i16.to_be_bytes());
            bmhd.extend_from_slice(&0i16.to_be_bytes());
            bmhd.push(n_planes as u8);
            bmhd.push(if opt.transparent.is_some() { 2 } else { 0 });
            bmhd.push(1);
            bmhd.push(0);
            bmhd.extend_from_slice(&(opt.transparent.unwrap_or(0) as u16).to_be_bytes());
            bmhd.push(1);
            bmhd.push(1);
            bmhd.extend_from_slice(&(w as i16).to_be_bytes());
            bmhd.extend_from_slice(&(h as i16).to_be_bytes());
            chunk(&mut f, b"BMHD", &bmhd);
            let first_pal = opt.frame_palettes.first().cloned().flatten().unwrap_or(*pal);
            chunk(&mut f, b"CMAP", &cmap_of(&first_pal));
            if opt.camg != 0 {
                chunk(&mut f, b"CAMG", &opt.camg.to_be_bytes());
            }
            let mut dpan = Vec::new();
            dpan.extend_from_slice(&4u16.to_be_bytes());
            dpan.extend_from_slice(&(nf as u16).to_be_bytes());
            dpan.extend_from_slice(&0u32.to_be_bytes());
            chunk(&mut f, b"DPAN", &dpan);
            if opt.brush {
                let mut g = Vec::new();
                g.extend_from_slice(&opt.grab.0.to_be_bytes());
                g.extend_from_slice(&opt.grab.1.to_be_bytes());
                chunk(&mut f, b"GRAB", &g);
            }
            for r in opt.ranges.iter().take(16) {
                let mut c = Vec::new();
                c.extend_from_slice(&0u16.to_be_bytes());
                c.extend_from_slice(&r.rate.to_be_bytes());
                let flags: u16 = (r.active as u16) | ((r.reverse as u16) << 1);
                c.extend_from_slice(&flags.to_be_bytes());
                c.push(r.low);
                c.push(r.high);
                chunk(&mut f, b"CRNG", &c);
            }
            let anhd = Anhd { op: 0, w, h, reltime, interleave: if opt.brush { 1 } else { 0 }, ..Default::default() };
            chunk(&mut f, b"ANHD", &anhd.write());
            let mut b = Vec::new();
            let p = &planar[fi];
            for y in 0..p.h {
                for k in 0..n_planes {
                    pack_byterun1(&p.planes[k][y * p.bpr..(y + 1) * p.bpr], &mut b);
                }
            }
            chunk(&mut f, b"BODY", &b);
        } else {
            let base = if k >= interleave { seq[k - interleave] } else { seq[0] };
            // This frame's time is how long the previous frame stays.
            let prev = seq[k - 1];
            let rt = match opt.frame_delays.get(prev).copied().unwrap_or(0) {
                0 => reltime,
                j => j,
            };
            let anhd = Anhd {
                op: 5,
                w,
                h,
                reltime: rt,
                interleave: if opt.brush { 1 } else { 0 },
                bits: if opt.brush { 4 } else { 0 },
                ..Default::default()
            };
            chunk(&mut f, b"ANHD", &anhd.write());
            if let Some(Some(p)) = opt.frame_palettes.get(fi) {
                chunk(&mut f, b"CMAP", &cmap_of(p));
            }
            chunk(&mut f, b"DLTA", &encode_dlta(&planar[base], &planar[fi], opt.brush));
        }
        body.extend(form(b"ILBM", f));
    }
    form(b"ANIM", body)
}

/// Does this file start like an IFF ANIM?
pub fn is_anim(d: &[u8]) -> bool {
    d.len() >= 12 && &d[0..4] == b"FORM" && &d[8..12] == b"ANIM"
}
