//! `bench textures`: contact sheet of the generated texture pack (4× scale, grey checker
//! background so transparency is visible), and optionally the pack as PNG files.

use std::path::PathBuf;

use crate::image::Image;

pub fn run(args: &[String]) -> anyhow::Result<()> {
    let mut out = PathBuf::from("bench-out/textures.png");
    let mut pack_dir: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--out" => out = PathBuf::from(it.next().ok_or_else(|| anyhow::anyhow!("--out DIR"))?),
            "--write-pack" => {
                pack_dir = Some(PathBuf::from(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("--write-pack DIR"))?,
                ))
            }
            other => anyhow::bail!("unknown argument {other}"),
        }
    }
    let tex = hearth_texgen::default_textures();
    let scale = 4usize;
    let cols = 16usize;
    let cell = 16 * scale + 4;
    let rows = tex.len().div_ceil(cols);
    let mut img = Image::new(cols * cell, rows * cell);
    for (k, t) in tex.iter().enumerate() {
        let (cx, cy) = ((k % cols) * cell, (k / cols) * cell);
        for y in 0..16 * scale {
            for x in 0..16 * scale {
                let p = t.tex.px[(y / scale) * 16 + x / scale];
                let bg = if ((x / 8) + (y / 8)) % 2 == 0 {
                    70u8
                } else {
                    90u8
                };
                let a = p[3] as f32 / 255.0;
                let mix = |c: u8| (c as f32 * a + bg as f32 * (1.0 - a)) as u8;
                img.set(cx + x, cy + y, [mix(p[0]), mix(p[1]), mix(p[2])]);
            }
        }
    }
    img.save(&out)?;
    println!("{} textures → {}", tex.len(), out.display());
    for (k, t) in tex.iter().enumerate() {
        if k % cols == 0 {
            print!("\nrow {:2}: ", k / cols);
        }
        print!("{} ", t.name.trim_start_matches("block/"));
    }
    println!();
    if let Some(dir) = pack_dir {
        let n = hearth_texgen::write_pack(&dir)?;
        println!("wrote {n} textures to {}", dir.display());
    }
    Ok(())
}
