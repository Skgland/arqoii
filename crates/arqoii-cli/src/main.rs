use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod gui;
mod png;
mod qoi;

#[derive(Parser, Debug)]
struct CmdArgs {
    #[command(subcommand)]
    command: Command,

    #[clap(global = true)]
    paths: Vec<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Command {
    View,
    Convert,
}

fn main() {
    let args = CmdArgs::parse();
    match args.command {
        Command::View => {
            gui::open(args);
        }
        Command::Convert => {
            for src in args.paths {
                let ext = src.extension();

                let Some(ext) = ext else {
                    eprint!(
                        "Skipping {}, as the the file extension was not found!",
                        src.display()
                    );
                    continue;
                };

                if ext == "png" {
                    transcode_png_to_qoi(&src);
                } else if ext == "qoi" {
                    transcode_qoi_to_png(&src);
                }
            }
        }
    }
}

fn transcode_png_to_qoi(src: &Path) {
    let png_bytes = std::fs::read(src).unwrap();
    let (channels, size, frames) = png::load(&png_bytes);
    match frames.as_slice() {
        [frame] => {
            let dest = src.with_extension("qoi");
            qoi::save(channels, size, frame, &dest);
        }
        _ => {
            for (idx, frame) in frames.iter().enumerate() {
                let dest = src.with_extension(format!("{idx}.qoi"));
                qoi::save(channels.clone(), size, frame, &dest);
            }
        }
    }
}

fn transcode_qoi_to_png(src: &Path) {
    let dest = src.with_extension("png");
    let qoi_bytes = std::fs::read(src).unwrap();
    let (header, pixels) = qoi::load(&qoi_bytes);
    png::save(
        header.channels,
        (header.width, header.height),
        &pixels,
        &dest,
    );
}
