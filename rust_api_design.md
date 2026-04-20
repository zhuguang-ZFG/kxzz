# Rust API 设计稿（font_core / app_core）

## 目标
基于已逆出的 Java 主程序源码，为 Rust 重写提供第一版公开 API 设计。

对应来源：
- `D:\GitHub\kxzz\decompiled_app2_full\com\kvenjoy\drawsoft\lib\b\*.java`
- `D:\GitHub\kxzz\decompiled_app2_full\com\kvenjoy\drawfont\*.java`
- `D:\GitHub\kxzz\decompiled_app2_full\com\kvenjoy\drawfont\b\*.java`

## 一、font_core

### 1.1 核心模型
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontKind {
    Word,
    Number,
    Symbol,
    Other(i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontMeta {
    pub version: i32,
    pub kind: FontKind,
    pub name: String,
    pub author: String,
    pub description: String,
    pub size: i32,
    pub glyph_count: i32,
    pub extra_v: Option<String>,
    pub password: Option<String>,
    pub uuid: Option<String>,
    pub file_path: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphPathChunk {
    pub points: Vec<f32>,
    pub verbs: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlyphData {
    pub key: String,
    pub chunks: Vec<GlyphPathChunk>,
    pub cached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GfontFile {
    pub meta: FontMeta,
    pub glyphs: std::collections::HashMap<String, GlyphData>,
    pub zip_blob: Option<Vec<u8>>,
}
```

### 1.2 读取/写入 API
对应 Java：
- `drawsoft.lib.b.d.a(InputStream)`
- `drawsoft.lib.b.d.a(OutputStream)`
- `drawsoft.lib.b.a.a(DataInputStream, boolean)`
- `drawsoft.lib.b.a.a(DataOutputStream, boolean)`

```rust
pub fn parse_gfont(bytes: &[u8]) -> anyhow::Result<GfontFile>;
pub fn write_gfont(file: &GfontFile) -> anyhow::Result<Vec<u8>>;

impl GfontFile {
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self>;
    pub fn to_path(&self, path: &std::path::Path) -> anyhow::Result<()>;
}
```

### 1.3 字形访问 API
对应 Java：
- `d.b()` 列出字形
- `d.c(String)` 懒加载单字形
- `d.b(String)` 缺字拆分
- `a.a(int)` 获取某个 path

```rust
impl GfontFile {
    pub fn version(&self) -> i32;
    pub fn list_glyph_keys(&self) -> Vec<String>;
    pub fn list_loaded_glyphs(&self) -> Vec<&GlyphData>;
    pub fn has_glyph(&self, key: &str) -> bool;
    pub fn get_glyph(&self, key: &str) -> Option<&GlyphData>;
    pub fn get_glyph_mut(&mut self, key: &str) -> Option<&mut GlyphData>;
    pub fn load_glyph(&mut self, key: &str) -> anyhow::Result<Option<&GlyphData>>;
    pub fn insert_glyph(&mut self, glyph: GlyphData);
    pub fn missing_tokens(&self, text: &str) -> Vec<String>;
}
```

### 1.4 编辑 API
对应 Java：
- `a.a(List, int)`
- `a.a(float[], byte[])`
- `h.a(String)`
- `h.g()`

```rust
impl GlyphData {
    pub fn new(key: impl Into<String>) -> Self;
    pub fn path_count(&self) -> usize;
    pub fn get_path(&self, index: usize) -> Option<&GlyphPathChunk>;
    pub fn get_path_mut(&mut self, index: usize) -> Option<&mut GlyphPathChunk>;
    pub fn push_path(&mut self, chunk: GlyphPathChunk);
    pub fn replace_path(&mut self, index: usize, chunk: GlyphPathChunk) -> anyhow::Result<()>;
    pub fn remove_path(&mut self, index: usize) -> anyhow::Result<GlyphPathChunk>;
}

impl GfontFile {
    pub fn add_missing_tokens_from_text(&mut self, text: &str);
    pub fn set_password(&mut self, password: Option<String>);
}
```

### 1.5 路径转换 API
对应 Java：
- `drawsoft.lib.b.i`
- `drawsoft.lib.b.k`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathVerb {
    MoveTo,
    LineTo,
    CurveTo,
    Close,
    Unknown(u8),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathSegment {
    MoveTo { x: f32, y: f32 },
    LineTo { x: f32, y: f32 },
    CurveTo { x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32 },
    Close,
}

pub fn chunk_to_segments(chunk: &GlyphPathChunk) -> Vec<PathSegment>;
pub fn segments_to_chunk(segments: &[PathSegment]) -> GlyphPathChunk;
pub fn glyph_to_segments(glyph: &GlyphData) -> Vec<Vec<PathSegment>>;
```

### 1.6 加密/版本支持 API
对应 Java：
- `drawsoft.lib.b.d.a(DataOutputStream)`
- `drawsoft.lib.a.a.a`

```rust
pub mod crypto {
    pub fn decrypt_header(version: i32, encrypted: &[u8]) -> anyhow::Result<Vec<u8>>;
    pub fn encrypt_header(version: i32, plain: &[u8]) -> anyhow::Result<Vec<u8>>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GfontCompatibility {
    V1To4Plain,
    V5To8Encrypted,
    V9Encrypted,
}

pub fn compatibility_for_version(version: i32) -> anyhow::Result<GfontCompatibility>;
```

## 二、app_core

### 2.1 应用级模型
```rust
#[derive(Debug, Clone)]
pub enum WorkspacePage {
    Start,
    NewFont,
    NewGraph,
    EditFont,
    EditGraph,
}

#[derive(Debug, Clone)]
pub struct FontDraft {
    pub name: String,
    pub kind: FontKind,
    pub author: String,
    pub description: String,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GraphDraft {
    pub name: String,
    pub author: String,
    pub description: String,
}
```

### 2.2 字体创建 / 打开 / 保存
对应 Java：
- `drawfont.b.n`
- `DrawFont.b(File)`
- `drawfont.b.h.i()`

```rust
pub fn create_font(draft: FontDraft) -> GfontFile;
pub fn open_font(path: &std::path::Path) -> anyhow::Result<GfontFile>;
pub fn save_font(font: &GfontFile, path: &std::path::Path) -> anyhow::Result<()>;
pub fn verify_font_password(font: &GfontFile, input: &str) -> bool;
```

### 2.3 字体编辑器状态
对应 Java：
- `drawfont.b.h`

```rust
#[derive(Debug)]
pub struct FontEditorState {
    pub font: GfontFile,
    pub visible_glyph_keys: Vec<String>,
    pub selected_glyph: Option<String>,
    pub selected_path_index: Option<usize>,
    pub search_mode: bool,
    pub background_image: Option<image::GrayImage>,
}

impl FontEditorState {
    pub fn new(font: GfontFile) -> Self;
    pub fn select_glyph(&mut self, key: &str) -> anyhow::Result<()>;
    pub fn select_path(&mut self, index: usize) -> anyhow::Result<()>;
    pub fn search(&mut self, text: &str);
    pub fn clear_search(&mut self);
    pub fn add_missing_chars_from_text(&mut self, text: &str);
    pub fn set_background_image(&mut self, img: Option<image::GrayImage>);
    pub fn save_to(&self, path: &std::path::Path) -> anyhow::Result<()>;
}
```

### 2.4 图像转字形路径
对应 Java：
- `drawfont.b.h.a(File)`
- `drawfont.b.h.b(float)`
- `drawfont.b.h.a(h, float)`
- `CLibraryUtils.binaryImage/skeleton/createPath`

```rust
pub struct AutoTraceOptions {
    pub threshold: u8,
    pub path_mode: i32,
    pub simplify_level: i32,
    pub min_path_len: f32,
}

pub fn trace_background_to_paths(
    image: &image::GrayImage,
    opts: &AutoTraceOptions,
) -> anyhow::Result<Vec<GlyphPathChunk>>;
```

### 2.5 图形编辑状态
对应 Java：
- `drawfont.b.j`
- `drawsoft.lib.c.e`

```rust
#[derive(Debug, Clone)]
pub struct GraphDocument {
    pub name: String,
    pub author: String,
    pub description: String,
    pub paths: Vec<GlyphPathChunk>,
}

pub fn create_graph(draft: GraphDraft) -> GraphDocument;
pub fn open_graph(path: &std::path::Path) -> anyhow::Result<GraphDocument>;
pub fn save_graph(doc: &GraphDocument, path: &std::path::Path) -> anyhow::Result<()>;
```

### 2.6 编辑器公共动作
对应 Java：
- `drawfont.b.l`
- `drawfont.a.*`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorCommand {
    Clear,
    Undo,
    ZoomIn,
    ZoomOut,
    AddStyle,
}

pub trait EditableCanvas {
    fn clear(&mut self);
    fn undo(&mut self);
    fn zoom_in(&mut self);
    fn zoom_out(&mut self);
}
```

## 三、native_algorithms（给 app_core 依赖）
```rust
pub trait NativeAlgorithms {
    fn rdp_simplify(&self, points: &[f32], epsilon: f32) -> anyhow::Result<Vec<f32>>;
    fn binary_image(&self, rgb: &[i32], threshold: u8) -> anyhow::Result<Vec<u8>>;
    fn skeletonize(&self, image: &[u8], width: i32, height: i32, max_iter: i32) -> anyhow::Result<Vec<u8>>;
    fn create_path(&self, image: &[u8], width: i32, path_mode: i32, simplify_level: i32, epsilon: f32) -> anyhow::Result<Vec<GlyphPathChunk>>;
}
```

建议：
- 第一版做 `JnaCompatAlgorithms` / `DllCompatAlgorithms`
- 后续再换纯 Rust 实现

## 四、network（后续）
```rust
pub struct SharePayload {
    pub font_bytes: Vec<u8>,
    pub preview_jpeg: Vec<u8>,
}

pub trait FontShareService {
    fn share_font(&self, payload: SharePayload) -> anyhow::Result<()>;
}

pub trait UpdateService {
    fn check_update(&self) -> anyhow::Result<Option<UpdateInfo>>;
}
```

## 五、优先实现顺序
1. `font_core::model / parser / writer`
2. `font_core::path`
3. `app_core::font_create`
4. `app_core::font_editor`
5. `graph_core`
6. `native_algorithms`
7. `network`

## 六、当前可直接开工的最小集
### font_core MVP
- `FontMeta`
- `GlyphPathChunk`
- `GlyphData`
- `GfontFile`
- `parse_gfont()`
- `write_gfont()`
- `missing_tokens()`
- `chunk_to_segments()`

### app_core MVP
- `FontDraft`
- `create_font()`
- `open_font()`
- `save_font()`
- `FontEditorState`
- `search()/clear_search()/select_glyph()`
