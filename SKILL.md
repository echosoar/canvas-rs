---
name: canvas-cli
description: 一个命令行图片生成工具。可以通过读取一段简单的文本指令脚本，即可完成 2D 图形绘制 ，并可以将生成的图片输出为 PNG 图像文件或 Base64 Data URL。
---

## 命令行用法

```
canvas-cli --input=<文件路径或内联指令> --output=<输出PNG路径>
canvas-cli --input=<文件路径或内联指令> --output-data-url
```

选项：

| 选项 | 说明 |
|------|------|
| `--input=<path>` | 指令文件路径，或直接内联的指令字符串（用 `;` 分隔） |
| `--output=<path>` | 输出 PNG 文件路径 |
| `--output-data-url` | 将结果输出为 `data:image/png;base64,...` 字符串而非文件 |

---

## 指令脚本格式

每行一条指令，`#` 开头为注释，空行忽略。指令文件的**第一行必须是 `canvas` 指令**以初始化画布。

### 画布初始化

```
canvas <width> <height>
```

示例：`canvas 1080 200`

---

### 颜色与样式

```
set_fill_style <color>
set_stroke_style <color>
set_line_width <width>
```

`<color>` 支持：
- CSS 命名色：`red`、`white`、`black`、`blue` 等
- 十六进制：`#RGB`、`#RRGGBB`、`#RRGGBBAA`
- 函数式：`rgb(255,0,0)`、`rgba(255,0,0,128)`

---

### 字体

```
set_font <size>px <family>
```

示例：`set_font 32px common`

- `size`：字号（像素），决定文字高度。
- `family`：字体名，对应 `lib/<family>.txt` 位图字体文件。
- 内置字体：`common`（支持 ASCII 及常用中文字符）。

---

### 矩形绘制

```
fill_rect <x> <y> <w> <h>
stroke_rect <x> <y> <w> <h>
```

用当前 `fill_style` / `stroke_style` 填充或描边矩形。

---

### 文字绘制

```
fill_text "<text>" <x> <y>
```

- `<text>` 需用双引号包裹（支持空格和中文）。
- 坐标 `(x, y)` 是文字左上角位置（像素）。

---

### 路径绘制

路径操作与浏览器 Canvas API 完全对应：

```
begin_path
move_to <x> <y>
line_to <x> <y>
arc <cx> <cy> <radius> <start_angle> <end_angle>
close_path
fill
stroke
```

- 角度单位：弧度（`0` = 右方，顺时针为正）。
- `arc` 参数依次为：圆心 x、圆心 y、半径、起始角、终止角。
- `fill` / `stroke` 分别以 `fill_style` / `stroke_style` 填充/描边已定义路径。

---

### 图片绘制

```
draw_image <path> <x> <y>
```

- `<path>`：PNG 文件路径，支持相对路径（相对于输入脚本文件所在目录）或绝对路径。
- `(x, y)`：图片放置的左上角坐标。

---

## 完整示例脚本

```
canvas 1080 200
draw_image ../banner.png 860 0
set_fill_style black
set_font 32px common
fill_text "让天下没有难生成的图。" 20 50
set_fill_style red
fill_text "Make it so that no graph is difficult to generate." 20 90
set_fill_style blue
set_font 16px common
fill_text "--- 2026.03.15" 20 130
```

运行：

```bash
canvas-cli --input=script.txt --output=output.png
```

---
