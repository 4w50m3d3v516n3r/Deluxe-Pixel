//! Tests for the IFF ANIM / AnimBrush reader and writer (the Amiga part of the person's spec).

use crate::anim::*;
use crate::palette::{Range, DEFAULT_PALETTE};
use crate::pix::Image;

fn frame(w: i32, h: i32, seed: u32, colours: u32) -> Image {
    let mut img = Image::new(w, h, 0);
    for y in 0..h {
        for x in 0..w {
            let v = ((x as u32 * 7 + y as u32 * 13 + seed * 31) / 5 + seed) % colours;
            img.set(x, y, v as u8);
        }
    }
    img
}

/// A moving square on a striped background: small deltas, like a real animation.
fn moving(w: i32, h: i32, k: i32) -> Image {
    let mut img = Image::new(w, h, 0);
    for y in 0..h {
        for x in 0..w {
            img.set(x, y, ((x / 4 + y / 3) % 5) as u8);
        }
    }
    for y in 0..8 {
        for x in 0..8 {
            img.set(2 + k * 3 + x, 3 + k + y, 12 + (k as u8 % 3));
        }
    }
    img
}

fn opts(brush: bool) -> EncodeOptions {
    EncodeOptions {
        brush,
        grab: (3, 4),
        transparent: if brush { Some(0) } else { None },
        fps: 10.0,
        ranges: vec![Range::new(16, 31, 4096)],
        frame_palettes: Vec::new(),
        frame_delays: Vec::new(),
        planes: 0,
        camg: 0,
    }
}

/// Apply one operation-5 delta to `p` through the decoder (single buffer).
fn apply5(p: &Planar, dlta: &[u8]) -> Planar {
    let mut a = AnimFile::default();
    a.bmhd = Bmhd { w: p.w as u16, h: p.h as u16, n_planes: p.planes.len() as u8, masking: 0, compression: 0, transparent: 0 };
    let mut body = Vec::new();
    for y in 0..p.h {
        for k in 0..p.planes.len() {
            body.extend_from_slice(&p.planes[k][y * p.bpr..(y + 1) * p.bpr]);
        }
    }
    a.forms.push(FrameRec { body: Some(body), ..Default::default() });
    a.forms.push(FrameRec { anhd: Some(Anhd { op: 5, interleave: 1, ..Default::default() }), dlta: Some(dlta.to_vec()), ..Default::default() });
    decode_forms(&a).unwrap()[1].0.clone()
}

#[test]
fn odd_chunk_padding_is_skipped() {
    let c = chunks(b"CMAP\0\0\0\x09abcdefghi\0BODY\0\0\0\x02xyANHD\0\0\0\x01z\0DLTA\0\0\0\x02pq");
    assert_eq!(c.len(), 4);
    assert_eq!(&c[0].0, b"CMAP");
    assert_eq!(c[0].1, b"abcdefghi");
    assert_eq!(&c[1].0, b"BODY");
    assert_eq!(c[1].1, b"xy");
    assert_eq!(&c[2].0, b"ANHD");
    assert_eq!(c[2].1, b"z");
    assert_eq!(&c[3].0, b"DLTA");
    assert_eq!(c[3].1, b"pq");
    // A whole file with an odd-sized chunk in frame 1 still decodes.
    let frames = vec![frame(16, 2, 1, 4)];
    let data = encode(&frames, &DEFAULT_PALETTE, &opts(false));
    let at = 24; // after "FORM size ANIM FORM size ILBM"
    let name = b"NAME\0\0\0\x05hello\0";
    let mut out = data[..at].to_vec();
    out.extend_from_slice(name);
    out.extend_from_slice(&data[at..]);
    for o in [4usize, 16] {
        let v = u32::from_be_bytes([out[o], out[o + 1], out[o + 2], out[o + 3]]) + name.len() as u32;
        out[o..o + 4].copy_from_slice(&v.to_be_bytes());
    }
    assert_eq!(decode(&out).unwrap().frames[0], frames[0]);
}

#[test]
fn odd_widths_round_trip() {
    assert_eq!(bytes_per_row(13), 2);
    assert_eq!(bytes_per_row(17), 4);
    assert_eq!(bytes_per_row(319), 40);
    for w in [13, 17, 319] {
        let frames: Vec<Image> = (0..4).map(|k| frame(w, 7, k, 16)).collect();
        let d = decode(&encode(&frames, &DEFAULT_PALETTE, &opts(false))).unwrap();
        assert_eq!(d.frames, frames, "width {w}");
    }
}

#[test]
fn double_buffer_uses_frame_two_back() {
    let f: Vec<Image> = (0..4).map(|k| frame(16, 4, k, 2)).collect();
    let p: Vec<Planar> = f.iter().map(|i| Planar::from_image(i, 1)).collect();
    let a = parse(&encode(&f, &DEFAULT_PALETTE, &opts(false))).unwrap();
    // Frame 3 is stored as a delta from frame 1, frame 4 as a delta from frame 2.
    assert_eq!(a.forms[2].dlta.as_ref().unwrap(), &encode_dlta(&p[0], &p[2], false));
    assert_eq!(a.forms[3].dlta.as_ref().unwrap(), &encode_dlta(&p[1], &p[3], false));
    for il in [0u8, 2] {
        let mut a2 = a.clone();
        for fr in a2.forms.iter_mut().skip(1) {
            fr.anhd.as_mut().unwrap().interleave = il;
        }
        let forms = decode_forms(&a2).unwrap();
        for k in 0..4 {
            assert_eq!(forms[k].0, p[k], "interleave {il}, frame {}", k + 1);
        }
    }
}

#[test]
fn animbrush_xor_single_buffer_keeps_grab_and_transparency() {
    let cells: Vec<Image> = (0..5).map(|k| frame(20, 9, k, 8)).collect();
    let data = encode(&cells, &DEFAULT_PALETTE, &opts(true));
    let a = parse(&data).unwrap();
    let h = a.forms[1].anhd.unwrap();
    assert_eq!((h.op, h.interleave, h.bits), (5, 1, 4));
    assert!(h.xor());
    let p: Vec<Planar> = cells.iter().map(|c| Planar::from_image(c, 3)).collect();
    // Frame k is an XOR delta on frame k-1.
    assert_eq!(a.forms[2].dlta.as_ref().unwrap(), &encode_dlta(&p[1], &p[2], true));
    let d = decode(&data).unwrap();
    assert_eq!(d.frames, cells);
    assert_eq!(d.grab, Some((3, 4)));
    assert_eq!(d.transparent, Some(0));
    assert_eq!(d.interleave, 1);
    assert!(!d.loop_frames);
}

#[test]
fn loop_frames_are_found_and_play_seamlessly() {
    let frames: Vec<Image> = (0..4).map(|k| moving(40, 24, k)).collect();
    let data = encode(&frames, &DEFAULT_PALETTE, &opts(false));
    let a = parse(&data).unwrap();
    assert_eq!(a.forms.len(), 6, "4 frames + 2 loop frames");
    assert_eq!(a.dpan_frames, Some(4));
    let forms = decode_forms(&a).unwrap();
    assert!(has_loop_frames(&forms));
    // The original's two-buffer player over three passes.
    let mut buf = [forms[0].0.clone(), forms[0].0.clone()];
    let mut shown = vec![buf[0].clone()];
    for k in 1..4 {
        let b = k % 2;
        buf[b] = apply5(&buf[b], a.forms[k].dlta.as_ref().unwrap());
        shown.push(buf[b].clone());
    }
    for _ in 0..2 {
        for k in [4usize, 5, 2, 3] {
            let b = k % 2;
            buf[b] = apply5(&buf[b], a.forms[k].dlta.as_ref().unwrap());
            if k >= 2 && k <= 3 {
                shown.push(buf[b].clone());
            }
            if k == 4 || k == 5 {
                // Loop frames bring the buffers back to frames 1 and 2.
                assert_eq!(buf[b], forms[k - 4].0);
                shown.push(buf[b].clone());
            }
        }
    }
    let planes = forms[0].0.planes.len();
    let expect: Vec<Planar> = play_sequence(4, 3).into_iter().map(|i| Planar::from_image(&frames[i], planes)).collect();
    assert_eq!(shown.len(), expect.len());
    for (i, (s, e)) in shown.iter().zip(expect.iter()).enumerate() {
        assert_eq!(s, e, "step {i}");
    }
    // The program sees the 4 real frames.
    assert_eq!(decode(&data).unwrap().frames, frames);
}

#[test]
fn ehb_and_ham6() {
    let pal: Vec<[u8; 3]> = (0..32).map(|i| [i as u8 * 8, 200, 100]).collect();
    let e = ehb_palette(&pal);
    assert_eq!(e.len(), 64);
    for i in 0..32 {
        assert_eq!(e[32 + i], [pal[i][0] >> 1, pal[i][1] >> 1, pal[i][2] >> 1]);
    }
    // HAM6: palette colour, then modify red, green, blue.
    let mut img = Image::new(4, 1, 0);
    img.set(0, 0, 0b00_0011);
    img.set(1, 0, 0b10_1111);
    img.set(2, 0, 0b11_0000);
    img.set(3, 0, 0b01_1000);
    let p = Planar::from_image(&img, 6);
    let hp: Vec<[u8; 3]> = (0..16).map(|i| [i * 16, i * 16, i * 16]).collect();
    let rgb = ham_to_rgb(&p, &hp);
    assert_eq!(rgb[0], [48, 48, 48]);
    assert_eq!(rgb[1], [255, 48, 48]);
    assert_eq!(rgb[2], [255, 0, 48]);
    assert_eq!(rgb[3], [255, 0, 136]);
}

#[test]
fn palette_changes_mid_animation() {
    let frames: Vec<Image> = (0..4).map(|k| frame(16, 4, k, 4)).collect();
    let mut p2 = DEFAULT_PALETTE;
    p2[1] = [1, 2, 3];
    let mut o = opts(false);
    o.frame_palettes = vec![None, None, Some(p2), None];
    let d = decode(&encode(&frames, &DEFAULT_PALETTE, &o)).unwrap();
    assert!(d.frame_palettes[1].is_none());
    assert_eq!(d.frame_palettes[2].unwrap()[1], [1, 2, 3]);
    assert!(d.frame_palettes[3].is_none());
}

#[test]
fn round_trip_animation_and_animbrush_over_three_loops() {
    let frames: Vec<Image> = (0..6).map(|k| moving(64, 32, k)).collect();
    for brush in [false, true] {
        let data = encode(&frames, &DEFAULT_PALETTE, &opts(brush));
        let a = parse(&data).unwrap();
        let forms = decode_forms(&a).unwrap();
        let n = frames.len();
        for (k, (p, _)) in forms.iter().enumerate() {
            let want = if k < n { &frames[k] } else { &frames[k - n] };
            assert_eq!(&p.to_image(), want, "brush {brush}, form {}", k + 1);
        }
        let d = decode(&data).unwrap();
        let shown: Vec<&Image> = play_sequence(n, 3).iter().map(|&i| &d.frames[i]).collect();
        let expect: Vec<&Image> = (0..3).flat_map(|_| frames.iter()).collect();
        assert_eq!(shown, expect);
    }
}

#[test]
fn walk_anim_sample_decodes() {
    // The person's sample (640 x 400, 4 planes, 10 frames + 2 loop frames, operation 5).
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("anim").join("walk.anim");
    let Ok(data) = std::fs::read(&path) else { return };
    let a = parse(&data).unwrap();
    assert_eq!((a.bmhd.w, a.bmhd.h, a.bmhd.n_planes), (640, 400, 4));
    assert_eq!(a.forms.len(), 12);
    assert_eq!(a.dpan_frames, Some(10));
    let d = decode(&data).unwrap();
    assert!(d.loop_frames);
    assert_eq!(d.frames.len(), 10);
    assert!((d.fps - 60.0).abs() < 0.01);
    assert_ne!(d.frames[0], d.frames[5]);
    let o = EncodeOptions { brush: false, grab: (0, 0), transparent: None, fps: d.fps, ranges: d.ranges.clone(), frame_palettes: Vec::new(), frame_delays: Vec::new(), planes: 0, camg: 0 };
    let again = decode(&encode(&d.frames, &d.palette, &o)).unwrap();
    assert_eq!(again.frames, d.frames);
}
