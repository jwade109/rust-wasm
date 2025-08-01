use crate::prelude::*;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

pub fn read_image(path: &Path) -> Option<RgbaImage> {
    Some(image::open(path).ok()?.to_rgba8())
}

pub fn diagram_color(part: &PartPrototype) -> [f32; 4] {
    match part {
        PartPrototype::Cargo(..) => [0.0, 0.45, 0.0, 1.0],
        PartPrototype::Thruster(..) => [1.0, 0.0, 0.0, 1.0],
        PartPrototype::Tank(..) => [1.0, 0.6, 0.0, 1.0],
        _ => match part.layer() {
            PartLayer::Exterior => [0.2, 0.2, 0.2, 1.0],
            PartLayer::Internal => [0.4, 0.4, 0.4, 1.0],
            PartLayer::Structural => [0.9, 0.9, 0.9, 1.0],
            PartLayer::Plumbing => [0.6, 0.0, 0.6, 1.0],
        },
    }
}

pub fn generate_image(
    vehicle: &Vehicle,
    parts_dir: &Path,
    schematic: bool,
) -> Option<DynamicImage> {
    let (pixel_min, pixel_max) = vehicle.pixel_bounds()?;
    let dims = pixel_max - pixel_min;
    let mut img = DynamicImage::new_rgba8(dims.x as u32, dims.y as u32);
    let to_export = img.as_mut_rgba8().unwrap();
    for layer in enum_iterator::all::<PartLayer>() {
        for (_, instance) in vehicle.parts() {
            if instance.prototype().layer() != layer {
                continue;
            }

            let path = parts_dir
                .join(instance.prototype().sprite_path())
                .join("skin.png");
            let img = read_image(&path)?;

            let px = (instance.origin().x - pixel_min.x) as u32;
            let py = (instance.origin().y - pixel_min.y) as u32;

            let color = diagram_color(&instance.prototype());

            for x in 0..img.width() {
                for y in 0..img.height() {
                    let p = IVec2::new(x as i32, y as i32);
                    let xp = img.width() as i32 - p.x - 1;
                    let yp = img.height() as i32 - p.y - 1;
                    let p = match instance.rotation() {
                        Rotation::East => IVec2::new(p.x, yp),
                        Rotation::North => IVec2::new(p.y, p.x),
                        Rotation::West => IVec2::new(xp, p.y),
                        Rotation::South => IVec2::new(yp, xp),
                    }
                    .as_uvec2();

                    let src = img.get_pixel_checked(x, y);
                    let dst = to_export
                        .get_pixel_mut_checked(px + p.x, to_export.height() - (py + p.y) - 1);
                    if let Some((src, dst)) = src.zip(dst) {
                        if src.0[3] > 0 {
                            for i in 0..3 {
                                dst.0[i] = if schematic {
                                    (color[i] * 255.0) as u8
                                } else {
                                    src.0[i]
                                };
                            }
                            dst.0[3] = 255;
                        }
                    }
                }
            }
        }
    }

    Some(img)
}
