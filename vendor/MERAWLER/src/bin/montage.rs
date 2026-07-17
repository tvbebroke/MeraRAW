//! `montage` — tile several PNG crops into one grid image for side-by-side
//! comparison. Cells are placed left-to-right, top-to-bottom in the order given,
//! separated by hairlines. (Labels are described by the caller; drawing text
//! would pull a font dependency.)
//!
//! Usage: `montage <out.png> <cols> <cell_px> <img1.png> <img2.png> ...`

use std::path::PathBuf;

use image::imageops::{resize, FilterType};
use image::{Rgb, RgbImage};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: montage <out.png> <cols> <cell_px> <img...>");
        std::process::exit(1);
    }
    let out = PathBuf::from(&args[0]);
    let cols: u32 = args[1].parse().expect("cols");
    let cell: u32 = args[2].parse().expect("cell_px");
    let imgs = &args[3..];
    let count = imgs.len() as u32;
    let rows = count.div_ceil(cols);
    let sep = 3u32;

    let w = cols * cell + (cols + 1) * sep;
    let h = rows * cell + (rows + 1) * sep;
    let mut canvas = RgbImage::from_pixel(w, h, Rgb([245, 245, 243]));

    for (i, path) in imgs.iter().enumerate() {
        let img = image::open(path).expect("open input").to_rgb8();
        let cellimg = resize(&img, cell, cell, FilterType::Triangle);
        let (r, c) = (i as u32 / cols, i as u32 % cols);
        let ox = sep + c * (cell + sep);
        let oy = sep + r * (cell + sep);
        for y in 0..cell {
            for x in 0..cell {
                canvas.put_pixel(ox + x, oy + y, *cellimg.get_pixel(x, y));
            }
        }
    }

    canvas.save(&out).expect("save montage");
    println!("wrote {} ({}x{}, {} cells)", out.display(), w, h, count);
}
