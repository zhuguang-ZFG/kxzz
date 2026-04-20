# 主程序模块对照表（Java -> Rust）

## 目标
把当前已反编译的 Java 主程序源码，映射成后续 Rust 重写时的模块边界。

源码根目录：
- `D:\GitHub\kxzz\decompiled_app2_full`

## 一、App Shell
### Rust 模块建议
- `app_shell`
- `app_shell::main_window`
- `app_shell::navigation`
- `app_shell::menu`

### 对应 Java 类
- `com.kvenjoy.drawfont.DrawFont`
- `com.kvenjoy.drawfont.c`
- `com.kvenjoy.drawfont.d`
- `com.kvenjoy.drawfont.f`
- `com.kvenjoy.drawfont.e`

### 职责
- 程序入口 `main`
- 主窗口 `JFrame`
- 菜单栏
- 页面切换（`CardLayout`）
- 保存/打开分发
- 关闭前钩子
- 全局初始化

### 说明
`DrawFont.java` 是总调度器，Rust 里应尽量只保留“导航和事件分发”，不要塞业务细节。

## 二、Start Page
### Rust 模块建议
- `app_core::start_page`

### 对应 Java 类
- `com.kvenjoy.drawfont.b.s`

### 职责
- 首页
- 新建字体
- 新建图形
- 打开字体
- 打开图形

## 三、Font Core
### Rust 模块建议
- `font_core::model`
- `font_core::glyph`
- `font_core::path`
- `font_core::iterator`
- `font_core::parser`
- `font_core::writer`
- `font_core::crypto`
- `font_core::zipstore`

### 对应 Java 类
- `com.kvenjoy.drawsoft.lib.b.d`：字体容器 `.gfont`
- `com.kvenjoy.drawsoft.lib.b.a`：单字形
- `com.kvenjoy.drawsoft.lib.b.i`：路径数据
- `com.kvenjoy.drawsoft.lib.b.k`：路径遍历器
- `com.kvenjoy.drawsoft.lib.b.c`：字体类型枚举
- `com.kvenjoy.drawsoft.lib.b.f/g/h/j`：辅助模型/转换

### 职责
- `.gfont` 读写
- 头部加解密
- ZIP 字形存储
- 预览字形读取
- 字符缺失分析
- 字形路径序列化

### 说明
这是最适合先独立重写的部分。

## 四、Font Editor
### Rust 模块建议
- `app_core::font_editor`
- `app_core::font_editor::state`
- `app_core::font_editor::search`
- `app_core::font_editor::preview`
- `app_core::font_editor::save`

### 对应 Java 类
- `com.kvenjoy.drawfont.b.h`
- `com.kvenjoy.drawfont.b.g`
- `com.kvenjoy.drawfont.b.m`
- `com.kvenjoy.drawfont.b.p`
- `com.kvenjoy.drawfont.b.a.a`
- `com.kvenjoy.drawfont.b.a.b`
- `com.kvenjoy.drawfont.b.a.c`

### 职责
- 字符列表
- 字形编辑画布
- 搜索/过滤
- 添加字符
- 路径设置
- 字体设置
- 保存 `.gfont`
- 导入背景图并自动生成路径

### 说明
`b.h` 是主字体编辑器页面，业务密度最高。

## 五、Font Creation Flow
### Rust 模块建议
- `app_core::font_create`

### 对应 Java 类
- `com.kvenjoy.drawfont.b.n`
- `com.kvenjoy.drawfont.b.t`

### 职责
- 新建字体表单
- 字体名称/作者/描述/密码/类型输入
- 创建初始 `font_core::FontFile`

## 六、Graph Core
### Rust 模块建议
- `graph_core::model`
- `graph_core::editor`
- `graph_core::serializer`

### 对应 Java 类
- `com.kvenjoy.drawsoft.lib.c.e`：图形文件模型（`.gap`）
- `com.kvenjoy.drawfont.b.j`：图形编辑器
- `com.kvenjoy.drawfont.b.o`：新建图形页

### 职责
- `.gap` 图形读写
- 图形编辑
- 背景图导入
- 图形保存

### 说明
可以独立成 `graph_core`，不要和 `font_core` 混在一起。

## 七、Editor Common UI
### Rust 模块建议
- `app_core::editor_common`
- `app_core::editor_common::toolbar`
- `app_core::editor_common::canvas`

### 对应 Java 类
- `com.kvenjoy.drawfont.b.l`
- `com.kvenjoy.drawfont.b.b`
- `com.kvenjoy.drawfont.a.*`

### 职责
- 编辑器基类
- 工具栏/操作绑定
- 画布事件
- 鼠标交互模型
- 选区/控制点/编辑动作

### 说明
`drawfont.a.*` 基本是编辑行为模型层，适合抽成 Rust 的画布交互核心。

## 八、Native Algorithms
### Rust 模块建议
- `native_algorithms`
- `native_algorithms::ffi`
- `native_algorithms::image_proc`
- `native_algorithms::path_proc`

### 对应 Java 类
- `com.kvenjoy.drawfont.CLibrary`
- `com.kvenjoy.drawfont.CLibraryUtils`

### 职责
- JNA 绑定本地算法库
- 折线简化（RDP）
- 二值化
- 阈值计算
- 骨架化
- 位图转路径
- 花纹/波浪线生成

### 说明
Rust 重写时有两种路线：
1. 先保留 FFI 调原 DLL
2. 再逐步纯 Rust 重写这些算法

## 九、Network / Share / Update
### Rust 模块建议
- `network`
- `network::share`
- `network::update`
- `network::oss`

### 对应 Java 类
- `com.kvenjoy.drawfont.i`：字体分享上传
- `com.kvenjoy.drawfont.o`：更新检查
- `com.kvenjoy.drawfont.b`：域名/版本常量

### 职责
- 序列化字体并上传
- 生成预览图并上传
- 检查版本更新
- 从 OSS 获取更新配置

## 十、Utilities
### Rust 模块建议
- `app_core::utils`
- `app_core::resource`
- `app_core::file_dialog`
- `app_core::platform`

### 对应 Java 类
- `com.kvenjoy.drawfont.m`
- `com.kvenjoy.drawfont.h`
- `com.kvenjoy.drawfont.g`
- `com.kvenjoy.drawfont.n`

### 职责
- 文件扩展名判断
- 文件对话框
- 浏览器打开
- 安装路径定位
- 资源字符表加载
- 偏好设置读取

## 十一、推荐 Rust 工程切分
### 1. `font_core`
来自：
- `drawsoft.lib.b.*`

### 2. `graph_core`
来自：
- `drawsoft.lib.c.e`
- `drawfont.b.j`
- `drawfont.b.o`

### 3. `native_algorithms`
来自：
- `CLibrary.java`
- `CLibraryUtils.java`

### 4. `app_core`
来自：
- `DrawFont.java`
- `drawfont.b.*`
- `drawfont.a.*`
- `drawfont.m`

### 5. `network`
来自：
- `drawfont.i`
- `drawfont.o`
- `drawfont.b`

## 十二、重写优先级
1. `font_core`
2. `graph_core`
3. `native_algorithms`（先 FFI 后纯 Rust）
4. `app_core`
5. `network`

## 十三、当前状态判断
如果目标是“完整主程序源码理解”：
- 已经足够

如果目标是“开始 Rust 重写”：
- 已经足够开始 `font_core` / `graph_core` / `app_core` 拆分

如果目标是“得到一份漂亮的 Java 工程源码”：
- 还需要继续做人类重命名和模块整理
