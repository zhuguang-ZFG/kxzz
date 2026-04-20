use anyhow::Result;
use native_algorithms::{
    default_installed_app_dir, KenjoyDllBackend, NativeAlgorithms,
};
use std::path::PathBuf;

fn main() -> Result<()> {
    let app_dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("KXZZ_APP_DIR").map(PathBuf::from))
        .unwrap_or_else(default_installed_app_dir);

    let backend = KenjoyDllBackend::from_installed_app_dir(&app_dir)?;
    println!("loaded dll: {}", backend.library_path.display());

    let polyline = vec![0.0f32, 0.0, 10.0, 0.0, 20.0, 0.0, 20.0, 20.0];
    let simplified = backend.simplify_polyline(&polyline, 0.5)?;
    println!("simplified polyline floats: {}", simplified.len());

    let width = 8usize;
    let height = 8usize;
    let mut pixels = vec![0xFFFF_FFFFu32 as i32; width * height];
    for y in 2..6 {
        for x in 2..6 {
            pixels[y * width + x] = 0xFF00_0000u32 as i32;
        }
    }

    let threshold = backend.detect_threshold(&pixels)?;
    println!("threshold: {threshold}");

    let bitmap = backend.binary_image(&pixels, threshold)?;
    println!("bitmap bytes: {}", bitmap.len());

    let skeleton = backend.skeletonize(width, height, &bitmap, i32::MAX)?;
    println!("skeleton bytes: {}", skeleton.len());

    Ok(())
}
