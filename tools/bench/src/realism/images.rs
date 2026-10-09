//! `bench realism images DIR`: statistics of rendered shots (or photographs) for the realism
//! suite (T §3.3): how bright and how contrasted, how colourful, how their detail falls off with
//! scale (natural scenes' power falls as 1/f^α with α near 2), and how much the ground repeats
//! itself (a tiled texture or a pattern of things shows as a second peak of the autocorrelation).

use std::fmt::Write as _;
use std::path::Path;

use rayon::prelude::*;

/// A picture's luminance (0–1, sRGB-encoded, Rec. 709 weights) and colour.
struct Picture {
    w: usize,
    h: usize,
    luma: Vec<f32>,
    rgb: Vec<[f32; 3]>,
}

/// A photograph (JPEG) as RGB rows.
fn load_jpeg(path: &Path) -> anyhow::Result<Picture> {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;
    let bytes = std::fs::read(path)?;
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut dec = JpegDecoder::new_with_options(ZCursor::new(&bytes), options);
    let px = dec
        .decode()
        .map_err(|e| anyhow::anyhow!("{}: {e:?}", path.display()))?;
    let (w, h) = dec
        .dimensions()
        .ok_or_else(|| anyhow::anyhow!("{}: no size", path.display()))?;
    let rgb: Vec<[f32; 3]> = px
        .chunks_exact(3)
        .map(|p| {
            [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
            ]
        })
        .collect();
    let luma = rgb
        .iter()
        .map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2])
        .collect();
    Ok(Picture { w, h, luma, rgb })
}

fn load(path: &Path) -> anyhow::Result<Picture> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
    {
        return load_jpeg(path);
    }
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut r = dec.read_info()?;
    let mut buf = vec![0; r.output_buffer_size().unwrap_or(0)];
    let info = r.next_frame(&mut buf)?;
    anyhow::ensure!(
        info.bit_depth == png::BitDepth::Eight,
        "{}: 8-bit pictures only",
        path.display()
    );
    let ch = info.color_type.samples();
    anyhow::ensure!(ch >= 3, "{}: not a colour picture", path.display());
    let (w, h) = (info.width as usize, info.height as usize);
    let rgb: Vec<[f32; 3]> = buf[..w * h * ch]
        .chunks_exact(ch)
        .map(|p| {
            [
                p[0] as f32 / 255.0,
                p[1] as f32 / 255.0,
                p[2] as f32 / 255.0,
            ]
        })
        .collect();
    let luma = rgb
        .iter()
        .map(|c| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2])
        .collect();
    Ok(Picture { w, h, luma, rgb })
}

/// What a picture's statistics are.
#[derive(Default)]
struct Stats {
    /// Luminance at the 5th, 50th and 95th percentiles.
    luma: [f32; 3],
    /// Hasler and Süsstrunk's colourfulness (0 grey; some 40–60 for natural scenes).
    colourful: f32,
    /// The spectral exponent α of P(f) ∝ f^-α over the middle octaves.
    alpha: f32,
    /// The ground's repetition: the highest rise of its autocorrelation along a row after it
    /// first falls, over lags of 16 pixels and more (0 where nothing repeats).
    repeat: f32,
    /// The lag (pixels) of that rise.
    repeat_lag: usize,
}

fn percentile(v: &mut [f32], p: f64) -> f32 {
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

/// The power of a detrended, windowed row at each frequency 1..n/2 (a plain DFT).
fn power(row: &[f32]) -> Vec<f64> {
    let n = row.len();
    let mean = row.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
    let x: Vec<f64> = row
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let hann = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (n - 1) as f64).cos();
            (v as f64 - mean) * hann
        })
        .collect();
    (1..n / 2)
        .map(|k| {
            let (mut re, mut im) = (0.0, 0.0);
            let w = std::f64::consts::TAU * k as f64 / n as f64;
            for (i, v) in x.iter().enumerate() {
                let a = w * i as f64;
                re += v * a.cos();
                im -= v * a.sin();
            }
            re * re + im * im
        })
        .collect()
}

fn measure(p: &Picture) -> Stats {
    let mut s = Stats::default();
    let mut l = p.luma.clone();
    s.luma = [
        percentile(&mut l, 0.05),
        percentile(&mut l, 0.5),
        percentile(&mut l, 0.95),
    ];

    // Colourfulness from the opponent channels rg = R − G and yb = (R + G)/2 − B (in 0–255).
    let (rg, yb): (Vec<f64>, Vec<f64>) = p
        .rgb
        .iter()
        .map(|c| {
            let (r, g, b) = (
                c[0] as f64 * 255.0,
                c[1] as f64 * 255.0,
                c[2] as f64 * 255.0,
            );
            (r - g, 0.5 * (r + g) - b)
        })
        .unzip();
    let stat = |v: &[f64]| {
        let m = v.iter().sum::<f64>() / v.len() as f64;
        let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt();
        (m, sd)
    };
    let ((mr, sr), (my, sy)) = (stat(&rg), stat(&yb));
    s.colourful = ((sr * sr + sy * sy).sqrt() + 0.3 * (mr * mr + my * my).sqrt()) as f32;

    // The spectrum along every fourth row and column, its slope fitted over the middle
    // octaves (from 1/64 to 1/4 of the picture's frequencies).
    let rows: Vec<Vec<f32>> = (0..p.h)
        .step_by(4)
        .map(|y| p.luma[y * p.w..(y + 1) * p.w].to_vec())
        .collect();
    let cols: Vec<Vec<f32>> = (0..p.w)
        .step_by(4)
        .map(|x| (0..p.h).map(|y| p.luma[y * p.w + x]).collect())
        .collect();
    let mut points = Vec::new();
    for (lines, n) in [(&rows, p.w), (&cols, p.h)] {
        let spectra: Vec<Vec<f64>> = lines.par_iter().map(|r| power(r)).collect();
        for k in (n / 64).max(2)..n / 4 {
            let mean = spectra.iter().map(|sp| sp[k - 1]).sum::<f64>() / spectra.len() as f64;
            if mean > 0.0 {
                points.push(((k as f64 / n as f64).ln(), mean.ln()));
            }
        }
    }
    let k = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / k;
    let my = points.iter().map(|p| p.1).sum::<f64>() / k;
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    s.alpha = (-sxy / sxx) as f32;

    // The ground (the lower half): its rows' autocorrelation by lag.
    let max_lag = p.w / 4;
    let mut r = vec![0.0f64; max_lag + 1];
    let mut count = 0usize;
    for y in p.h / 2..p.h {
        let row = &p.luma[y * p.w..(y + 1) * p.w];
        let m = row.iter().map(|&v| v as f64).sum::<f64>() / p.w as f64;
        let var = row.iter().map(|&v| (v as f64 - m).powi(2)).sum::<f64>();
        if var <= 1e-9 {
            continue;
        }
        count += 1;
        for (lag, out) in r.iter_mut().enumerate() {
            let c: f64 = (0..p.w - lag)
                .map(|x| (row[x] as f64 - m) * (row[x + lag] as f64 - m))
                .sum();
            *out += c / var;
        }
    }
    if count > 0 {
        for v in r.iter_mut() {
            *v /= count as f64;
        }
        let mut lowest = r[0];
        for (lag, &v) in r.iter().enumerate().skip(1) {
            lowest = lowest.min(v);
            if lag >= 16 && (v - lowest) as f32 > s.repeat {
                s.repeat = (v - lowest) as f32;
                s.repeat_lag = lag;
            }
        }
    }
    s
}

/// The pictures in a directory and the directories in it.
fn pictures(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> anyhow::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            pictures(&p, out)?;
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg"))
        {
            out.push(p);
        }
    }
    Ok(())
}

pub fn run(dir: &Path, out: &Path) -> anyhow::Result<()> {
    let mut files = Vec::new();
    pictures(dir, &mut files)?;
    files.sort();
    let mut md = String::from(
        "| Shot | Luminance p5 · p50 · p95 | Colourfulness | Spectral α | Repetition (lag px) |\n\
         |---|---|---|---|---|\n",
    );
    let mut tsv = String::from("shot\tl5\tl50\tl95\tcolourful\talpha\trepeat\trepeat_lag\n");
    for f in &files {
        let p = match load(f) {
            Ok(p) => p,
            Err(e) => {
                println!("{}: {e}", f.display());
                continue;
            }
        };
        let s = measure(&p);
        let name = f
            .strip_prefix(dir)
            .unwrap_or(f)
            .with_extension("")
            .display()
            .to_string()
            .replace('\\', "/");
        let _ = writeln!(
            md,
            "| {name} | {:.2} · {:.2} · {:.2} | {:.0} | {:.2} | {:.2} ({}) |",
            s.luma[0], s.luma[1], s.luma[2], s.colourful, s.alpha, s.repeat, s.repeat_lag
        );
        let _ = writeln!(
            tsv,
            "{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            s.luma[0], s.luma[1], s.luma[2], s.colourful, s.alpha, s.repeat, s.repeat_lag
        );
    }
    println!("{md}");
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("images.md"), md)?;
    std::fs::write(out.join("images.tsv"), tsv)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> Picture {
        let luma: Vec<f32> = (0..w * h).map(|k| f(k % w, k / w)).collect();
        let rgb = luma.iter().map(|&l| [l, l, l]).collect();
        Picture { w, h, luma, rgb }
    }

    #[test]
    fn a_tiled_pattern_repeats_and_grey_has_no_colour() {
        // Stripes 32 pixels apart: the autocorrelation rises again at a lag of 32.
        let p = picture(256, 64, |x, _| {
            0.5 + 0.4 * (std::f32::consts::TAU * x as f32 / 32.0).sin()
        });
        let s = measure(&p);
        assert!(s.repeat > 0.5, "{}", s.repeat);
        assert_eq!(s.repeat_lag % 32, 0, "{}", s.repeat_lag);
        assert!(s.colourful < 1e-3);
    }

    #[test]
    fn detail_falling_as_one_over_f_has_alpha_two() {
        // Waves along x and along y whose amplitudes fall as 1/f (their power as 1/f²), at
        // phases scattered by a hash.
        let n = 256;
        let phase = |k: usize, salt: usize| {
            let h = (k * 2_654_435_761 + salt * 40_503) % 1_000_003;
            h as f32 / 1_000_003.0 * std::f32::consts::TAU
        };
        let wave = |t: usize, salt: usize| -> f32 {
            (1..n / 2)
                .map(|k| {
                    (std::f32::consts::TAU * k as f32 * t as f32 / n as f32 + phase(k, salt)).sin()
                        / k as f32
                })
                .sum()
        };
        let wx: Vec<f32> = (0..n).map(|x| wave(x, 1)).collect();
        let wy: Vec<f32> = (0..n).map(|y| wave(y, 2)).collect();
        let p = picture(n, n, |x, y| 0.5 + 0.05 * (wx[x] + wy[y]));
        let s = measure(&p);
        assert!((s.alpha - 2.0).abs() < 0.2, "{}", s.alpha);
    }
}
