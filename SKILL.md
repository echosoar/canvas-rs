---
name: canvas-cli
description: 一个命令行图片生成工具。可以通过读取一段简单的文本指令脚本，即可完成 2D 图形绘制，并可以将生成的图片输出为 PNG 图像文件或 Base64 Data URL。也可以使用本工具的 draw_image 指令对已有的 PNG 图片进行裁剪、缩放和重绘。
---

> 提示：`canvas-cli` 命令的可执行文件放在本技能目录中，下述说明中的 `./canvas-cli` 指的就是这个可执行文件，canvas-cli 是可以直接运行的二进制可执行文件，不是脚本文件。。

## 命令行用法

```
./canvas-cli --input=<文件路径或内联指令> --output=<输出PNG路径>
./canvas-cli --input=<文件路径或内联指令> --output-data-url
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
- CSS 命名色：`red`、`white`、`black`、`blue` 等，例如 `set_fill_style red`
- 十六进制：`#RGB`、`#RRGGBB`、`#RRGGBBAA`，例如 `set_fill_style #FF0000`
- 函数式：`rgb(255,0,0)`、`rgba(255,0,0,128)`，例如 `set_stroke_style rgba(0,255,0,0.5)`

`<width>` 是线宽，单位像素，例如 `set_line_width 5`

---

### 字体

```
set_font <size>px <family>
set_text_align <align>
```

示例：`set_font 32px common`

- `size`：字号（像素），决定文字高度。
- `family`：字体名，对应 `lib/<family>.txt` 位图字体文件，没有特别指定的情况下使用 common 这个内置字体。
- 内置字体：`common`（支持 ASCII 及常用中文字符）。

`set_text_align` 设置文本对齐方式：

- `start` / `left`：左对齐（默认），`x` 是文本左边缘。
- `end` / `right`：右对齐，`x` 是文本右边缘。
- `center`：居中对齐，`x` 是文本中心点。

示例：`set_text_align center`

---

### 矩形绘制

```
fill_rect <x> <y> <w> <h>
stroke_rect <x> <y> <w> <h>
clear_rect <x> <y> <w> <h>
```

- `fill_rect`：用当前 `fill_style` 填充矩形。
- `stroke_rect`：用当前 `stroke_style` 描边矩形轮廓。
- `clear_rect`：将矩形区域清除为完全透明（不受当前样式影响）。

---

### 文字绘制

```
fill_text "<text>" <x> <y>
```

- `<text>` 需用双引号包裹（支持空格和中文）。
- 坐标 `(x, y)` 的含义取决于当前 `text_align` 设置：
  - `start` / `left`：`x` 是文本左边缘位置。
  - `end` / `right`：`x` 是文本右边缘位置。
  - `center`：`x` 是文本中心位置。
- `y` 始终是文本顶部位置。

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

### 渐变

支持绘制线性渐变和径向渐变，当用户提到渐变时，没有特殊指定则认为是线性渐变。

#### 创建渐变

```
# 线性渐变：从点 (x0, y0) 到点 (x1, y1)
create_linear_gradient <id> <x0> <y0> <x1> <y1>

# 径向渐变：从内圆 (x0, y0, r0) 到外圆 (x1, y1, r1)
create_radial_gradient <id> <x0> <y0> <r0> <x1> <y1> <r1>
```

- `<id>`：渐变标识符，用于后续引用。
- 坐标和半径均为像素值。

#### 添加颜色停止点

```
add_color_stop <gradient_id> <offset> <color>
```

- `<offset>`：位置偏移，范围 `0` ~ `1`（0 为起点，1 为终点）。
- `<color>`：颜色值，格式同 `set_fill_style`。

#### 应用渐变

```
set_fill_gradient <gradient_id>
set_stroke_gradient <gradient_id>
```

#### 渐变示例

```
# 创建线性渐变（从左到右）
create_linear_gradient my_gradient 0 0 200 0
add_color_stop my_gradient 0 red
add_color_stop my_gradient 0.5 yellow
add_color_stop my_gradient 1 blue
set_fill_gradient my_gradient
fill_rect 0 0 200 100

# 创建径向渐变（从中心向外）
create_radial_gradient radial 100 100 0 100 100 100
add_color_stop radial 0 white
add_color_stop radial 1 black
set_fill_gradient radial
fill_rect 0 0 200 200
```

---

### 状态保存与恢复

```
save
restore
```

保存/恢复当前绘图状态（包括样式、线宽、裁剪区域、字体设置等），但不包括当前路径。与 Web Canvas API 行为一致。

---

### 图片绘制

```
draw_image <path> <dx> <dy>
draw_image <path> <dx> <dy> <dWidth> <dHeight>
draw_image <path> <sx> <sy> <sWidth> <sHeight> <dx> <dy> <dWidth> <dHeight>
```

- `<path>`：PNG 文件路径，支持相对路径（相对于输入脚本文件所在目录）或绝对路径。
- `dx` / `dy`：目标画布中放置图片左上角的坐标。
- `dWidth` / `dHeight`：目标绘制尺寸，用于缩放图片。
- `sx` / `sy`：源图中裁剪区域左上角坐标。
- `sWidth` / `sHeight`：源图裁剪区域尺寸。

说明：
- 3 参数形式：按原始尺寸绘制整张图片。
- 5 参数形式：绘制整张图片，并缩放到指定目标尺寸。
- 9 参数形式：先从源图裁剪子区域，再绘制到目标区域并缩放。
- `sWidth`、`sHeight`、`dWidth`、`dHeight` 可以为负值，表示向相反方向扩展区域，但不会导致图片翻转。

示例：

```text
# 按原图尺寸绘制
draw_image ./avatar.png 40 20

# 缩放到 120x120
draw_image ./avatar.png 200 20 120 120

# 从源图裁剪 (10, 10, 80, 80)，绘制到目标 (380, 20, 160, 160)
draw_image ./avatar.png 10 10 80 80 380 20 160 160
```

---

## 完整示例脚本

```
canvas 1080 200

# 左边一半背景 - 径向渐变 (从中心向外)
create_radial_gradient radial_bg 270 100 0 270 100 200
add_color_stop radial_bg 0 white
add_color_stop radial_bg 0.5 yellow
add_color_stop radial_bg 1 blue
set_fill_gradient radial_bg
fill_rect 0 0 540 200

# 右边一半背景 - 线性渐变 (从左到右)
create_linear_gradient linear_bg 540 0 1080 0
add_color_stop linear_bg 0 gray
add_color_stop linear_bg 0.5 white
add_color_stop linear_bg 1 purple
set_fill_gradient linear_bg
fill_rect 540 0 540 200

# 绘制图片和文字
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
./canvas-cli --input=script.txt --output=output.png
```

---
