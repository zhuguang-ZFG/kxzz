use crate::api::{
    native_path_to_chunks, validate_raster_size, NativeAlgorithms, NativeAlgorithmsError,
    SquiggleRequest,
};
use anyhow::{anyhow, Context, Result};
use font_core::GlyphPathChunk;
use libloading::Library;
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

const APP_JAR_NAME: &str = "app2.jar";
const RESOURCE_PREFIX: &str = "com/kvenjoy/drawfont/resources";

type RdpPpdFn = unsafe extern "system" fn(*const f32, i32, f32) -> *mut c_void;
type RdpSizeFn = unsafe extern "system" fn() -> i32;
type PathGenernalFn =
    unsafe extern "system" fn(*const u8, i32, i32, i32, i32, f32, f32);
type GetPathsArrayFn = unsafe extern "system" fn() -> *mut c_void;
type GetPathSizeFn = unsafe extern "system" fn() -> i32;
type GetThresholdFn = unsafe extern "system" fn(*const i32, i32) -> i32;
type BinaryImageFn = unsafe extern "system" fn(*const i32, i32, i32) -> *mut c_void;
type SkeletonizeFn = unsafe extern "system" fn(*const u8, i32, i32, i32) -> *mut c_void;
type GenernalSquiggleFn =
    unsafe extern "system" fn(*const i32, i32, i32, f32, i32, i32, i32, i32, f32) -> *mut c_void;
type GetSquigglePathSizeFn = unsafe extern "system" fn() -> i32;
type FreeMemoryFn = unsafe extern "system" fn(*mut c_void);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KenjoyArch {
    X86,
    X64,
}

impl KenjoyArch {
    pub fn current() -> Self {
        #[cfg(target_pointer_width = "64")]
        {
            Self::X64
        }
        #[cfg(not(target_pointer_width = "64"))]
        {
            Self::X86
        }
    }

    pub fn as_dir(self) -> &'static str {
        match self {
            Self::X86 => "x86",
            Self::X64 => "x64",
        }
    }
}

#[derive(Debug, Clone)]
pub struct KenjoyDllConfig {
    pub dll_path: Option<PathBuf>,
    pub app_dir: Option<PathBuf>,
    pub extract_dir: Option<PathBuf>,
    pub arch: KenjoyArch,
}

impl Default for KenjoyDllConfig {
    fn default() -> Self {
        Self {
            dll_path: None,
            app_dir: None,
            extract_dir: None,
            arch: KenjoyArch::current(),
        }
    }
}

pub struct KenjoyDllBackend {
    library: Library,
    pub library_path: PathBuf,
}

impl KenjoyDllBackend {
    pub fn from_library_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let library = unsafe { Library::new(&path) }
            .with_context(|| format!("failed to load kenjoy dll: {}", path.display()))?;
        Ok(Self {
            library,
            library_path: path,
        })
    }

    pub fn from_config(config: KenjoyDllConfig) -> Result<Self> {
        if let Some(path) = config.dll_path {
            return Self::from_library_path(path);
        }

        let app_dir = config
            .app_dir
            .ok_or_else(|| anyhow!("KenjoyDllConfig requires dll_path or app_dir"))?;
        let dll_path = extract_dll_from_app_dir(
            &app_dir,
            config.extract_dir.as_deref(),
            config.arch,
        )?;
        Self::from_library_path(dll_path)
    }

    pub fn from_installed_app_dir(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_config(KenjoyDllConfig {
            app_dir: Some(path.as_ref().to_path_buf()),
            ..KenjoyDllConfig::default()
        })
    }

    pub fn extract_from_installed_app_dir(
        path: impl AsRef<Path>,
        arch: KenjoyArch,
        extract_dir: Option<&Path>,
    ) -> Result<PathBuf> {
        extract_dll_from_app_dir(path.as_ref(), extract_dir, arch)
    }

    unsafe fn symbol<T>(&self, name: &[u8]) -> Result<libloading::Symbol<'_, T>> {
        self.library
            .get(name)
            .map_err(|err| anyhow!("failed to resolve symbol {:?}: {err}", String::from_utf8_lossy(name)))
    }

    unsafe fn free_memory(&self, ptr: *mut c_void) -> Result<()> {
        if ptr.is_null() {
            return Ok(());
        }
        let free_memory = self.symbol::<FreeMemoryFn>(b"freeMemory")?;
        free_memory(ptr);
        Ok(())
    }

    unsafe fn copy_f32_result(&self, ptr: *mut c_void, len: usize) -> Result<Vec<f32>> {
        if ptr.is_null() {
            return Err(NativeAlgorithmsError::Unavailable.into());
        }
        let slice = std::slice::from_raw_parts(ptr.cast::<f32>(), len);
        let out = slice.to_vec();
        self.free_memory(ptr)?;
        Ok(out)
    }

    unsafe fn copy_u8_result(&self, ptr: *mut c_void, len: usize) -> Result<Vec<u8>> {
        if ptr.is_null() {
            return Err(NativeAlgorithmsError::Unavailable.into());
        }
        let slice = std::slice::from_raw_parts(ptr.cast::<u8>(), len);
        let out = slice.to_vec();
        self.free_memory(ptr)?;
        Ok(out)
    }
}

pub fn default_installed_app_dir() -> PathBuf {
    PathBuf::from(r"C:\Program Files (x86)\奎享字体")
}

impl NativeAlgorithms for KenjoyDllBackend {
    fn simplify_polyline(&self, points: &[f32], epsilon: f32) -> Result<Vec<f32>> {
        let ptr = unsafe {
            let fn_ptr = self.symbol::<RdpPpdFn>(b"RDPppd")?;
            fn_ptr(points.as_ptr(), points.len() as i32, epsilon)
        };
        let len = unsafe {
            let fn_ptr = self.symbol::<RdpSizeFn>(b"RDPSize")?;
            fn_ptr()
        };
        if len < 0 {
            return Err(anyhow!("RDPSize returned negative length {len}"));
        }
        unsafe { self.copy_f32_result(ptr, len as usize) }
    }

    fn detect_threshold(&self, argb_pixels: &[i32]) -> Result<i32> {
        let value = unsafe {
            let fn_ptr = self.symbol::<GetThresholdFn>(b"getThreshold")?;
            fn_ptr(argb_pixels.as_ptr(), argb_pixels.len() as i32)
        };
        Ok(value)
    }

    fn binary_image(&self, argb_pixels: &[i32], threshold: i32) -> Result<Vec<u8>> {
        let ptr = unsafe {
            let fn_ptr = self.symbol::<BinaryImageFn>(b"binaryImage")?;
            fn_ptr(argb_pixels.as_ptr(), argb_pixels.len() as i32, threshold)
        };
        unsafe { self.copy_u8_result(ptr, argb_pixels.len()) }
    }

    fn skeletonize(
        &self,
        width: usize,
        height: usize,
        bitmap: &[u8],
        max_iterations: i32,
    ) -> Result<Vec<u8>> {
        validate_raster_size(width, height, bitmap.len())?;
        let ptr = unsafe {
            let fn_ptr = self.symbol::<SkeletonizeFn>(b"skeletonize")?;
            fn_ptr(bitmap.as_ptr(), width as i32, height as i32, max_iterations)
        };
        unsafe { self.copy_u8_result(ptr, bitmap.len()) }
    }

    fn create_path(
        &self,
        bitmap: &[u8],
        width: usize,
        path_mode: i32,
        path_merge: i32,
        path_simplify: f32,
    ) -> Result<Vec<GlyphPathChunk>> {
        if width == 0 || bitmap.len() % width != 0 {
            return Err(anyhow!("bitmap length {} is not divisible by width {}", bitmap.len(), width));
        }
        let height = bitmap.len() / width;
        unsafe {
            let path_genernal = self.symbol::<PathGenernalFn>(b"pathGenernal")?;
            path_genernal(
                bitmap.as_ptr(),
                width as i32,
                height as i32,
                path_mode,
                path_merge,
                0.0,
                path_simplify,
            );
        }
        let ptr = unsafe {
            let fn_ptr = self.symbol::<GetPathsArrayFn>(b"getPathsArray")?;
            fn_ptr()
        };
        let len = unsafe {
            let fn_ptr = self.symbol::<GetPathSizeFn>(b"getPathSize")?;
            fn_ptr()
        };
        if len < 0 {
            return Err(anyhow!("getPathSize returned negative length {len}"));
        }
        let stream = unsafe { self.copy_f32_result(ptr, len as usize) }?;
        native_path_to_chunks(&stream)
    }

    fn generate_squiggle(
        &self,
        argb_pixels: &[i32],
        request: &SquiggleRequest,
    ) -> Result<Vec<GlyphPathChunk>> {
        validate_raster_size(request.width, request.height, argb_pixels.len())?;
        let ptr = unsafe {
            let fn_ptr = self.symbol::<GenernalSquiggleFn>(b"genernalSquiggle")?;
            fn_ptr(
                argb_pixels.as_ptr(),
                request.width as i32,
                request.height as i32,
                request.density,
                request.angle,
                request.spacing,
                request.line_width,
                request.line_cap,
                request.jitter,
            )
        };
        let len = unsafe {
            let fn_ptr = self.symbol::<GetSquigglePathSizeFn>(b"getSquigglePathSize")?;
            fn_ptr()
        };
        if len < 0 {
            return Err(anyhow!("getSquigglePathSize returned negative length {len}"));
        }
        let stream = unsafe { self.copy_f32_result(ptr, len as usize) }?;
        native_path_to_chunks(&stream)
    }
}

fn extract_dll_from_app_dir(
    app_dir: &Path,
    extract_dir: Option<&Path>,
    arch: KenjoyArch,
) -> Result<PathBuf> {
    let jar_path = app_dir.join(APP_JAR_NAME);
    if !jar_path.exists() {
        return Err(anyhow!("app jar not found: {}", jar_path.display()));
    }

    let file = fs::File::open(&jar_path)
        .with_context(|| format!("failed to open app jar: {}", jar_path.display()))?;
    let mut jar = ZipArchive::new(file)
        .with_context(|| format!("failed to open zip archive: {}", jar_path.display()))?;

    let entry_name = format!("{}/{}/kenjoycncc.dll", RESOURCE_PREFIX, arch.as_dir());
    let mut entry = jar
        .by_name(&entry_name)
        .with_context(|| format!("dll resource not found in app jar: {entry_name}"))?;

    let out_dir = extract_dir
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("kxzz_native"));
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create extraction dir: {}", out_dir.display()))?;

    let out_path = out_dir.join(format!("kenjoycncc-{}.dll", arch.as_dir()));
    let mut out_file = fs::File::create(&out_path)
        .with_context(|| format!("failed to create dll output: {}", out_path.display()))?;
    std::io::copy(&mut entry, &mut out_file)
        .with_context(|| format!("failed to extract dll to: {}", out_path.display()))?;
    Ok(out_path)
}
