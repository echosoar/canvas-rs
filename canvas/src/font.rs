use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 字体宽度类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FontWidth {
    /// 等宽字体，所有字符都是固定宽度
    Same,
    /// 变宽字体，每个字符有自己的宽度
    Variable,
}

/// 字体配置信息
#[derive(Debug, Clone)]
pub struct FontConfig {
    /// 字符像素大小（宽度和高度，正方形）
    pub size: u32,
    /// 每个像素占据的位数（目前只支持 1）
    pub bits: u32,
    /// 字体宽度类型
    pub width: FontWidth,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig {
            size: 32,
            bits: 1,
            width: FontWidth::Same,
        }
    }
}

/// 字符宽度类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CharWidthType {
    /// 半角，宽度为 size 的一半
    Half,
    /// 全角，宽度为 size
    Full,
}

/// 单个字符的位图数据
#[derive(Debug, Clone)]
pub struct CharBitmap {
    /// 字符位图矩阵，true 表示需要渲染的像素点
    pub bitmap: Vec<Vec<bool>>,
    /// 字符宽度（像素）
    pub width: u32,
    /// 字符高度（像素）
    pub height: u32,
    /// 字符宽度类型
    pub width_type: CharWidthType,
}

/// 字体对象
#[derive(Debug, Clone)]
pub struct Font {
    /// 字体名称
    pub name: String,
    /// 字体配置
    pub config: FontConfig,
    /// 字符位图映射表
    chars: HashMap<char, CharBitmap>,
}

impl Font {
    /// 从 lib 目录加载字体文件
    ///
    /// # 参数
    /// - `font_name`: 字体名称，对应文件名为 `{font_name}.txt`，如果为空则使用 `common.txt`
    ///
    /// # 返回
    /// 成功返回 Font 对象，失败返回错误信息
    pub fn load(font_name: &str) -> Result<Self, String> {
        let normalized_font_name = if font_name.eq_ignore_ascii_case("arial") {
            "common"
        } else {
            font_name
        };

        let filename = if normalized_font_name.is_empty() {
            "common.txt".to_string()
        } else {
            format!("{}.txt", normalized_font_name)
        };

        // 尝试从多个路径查找字体文件
        let paths = vec![
            Path::new("lib").join(&filename),
            Path::new("./lib").join(&filename),
            Path::new("../lib").join(&filename),
            Path::new("canvas/lib").join(&filename),
            Path::new("./canvas/lib").join(&filename),
            Path::new("../canvas/lib").join(&filename),
        ];

        let mut file_path = None;
        for path in &paths {
            if path.exists() {
                file_path = Some(path.clone());
                break;
            }
        }

        let file_path = file_path.ok_or_else(|| {
            format!("Font file '{}' not found in lib directory", filename)
        });

        if let Ok(file_path) = file_path {
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read font file: {}", e))?;

            return Self::parse(&content, normalized_font_name);
        }

        // Fallback to an embedded default font so packaged binaries can still render text.
        if filename == "common.txt" {
            let embedded_font = include_str!("../lib/common.txt");
            let embedded_name = if normalized_font_name.is_empty() {
                "common"
            } else {
                normalized_font_name
            };
            return Self::parse(embedded_font, embedded_name);
        }

        Err(format!("Font file '{}' not found in lib directory", filename))
    }

    /// 从字符串内容解析字体
    pub fn parse(content: &str, font_name: &str) -> Result<Self, String> {
        let mut lines = content.lines();

        // 解析第一行配置
        let config_line = lines.next()
            .ok_or("Font file is empty")?;

        let config = Self::parse_config(config_line)?;

        // 解析字符数据
        let mut chars = HashMap::new();

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // 解析格式：字符:type:base64数据
            // type: 0 = 半角（宽度为 size/2），1 = 全角（宽度为 size）
            let parts: Vec<&str> = line.splitn(3, ':').collect();
            if parts.len() != 3 {
                continue;
            }

            let char_str = parts[0];
            let type_str = parts[1];
            let base64_data = parts[2];

            if char_str.is_empty() {
                continue;
            }

            let ch = char_str.chars().next().unwrap();

            // 解析类型
            let width_type = match type_str {
                "0" => CharWidthType::Half,
                "1" => CharWidthType::Full,
                _ => CharWidthType::Full, // 默认全角
            };

            // 解码 base64 数据
            match Self::decode_char_bitmap(base64_data, &config, width_type) {
                Ok(bitmap) => {
                    chars.insert(ch, bitmap);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to decode char '{}': {}", ch, e);
                }
            }
        }

        Ok(Font {
            name: font_name.to_string(),
            config,
            chars,
        })
    }

    /// 解析字体配置行
    fn parse_config(line: &str) -> Result<FontConfig, String> {
        let mut config = FontConfig::default();

        for part in line.split(',') {
            let part = part.trim();
            let kv: Vec<&str> = part.splitn(2, ':').collect();
            if kv.len() != 2 {
                continue;
            }

            let key = kv[0].trim();
            let value = kv[1].trim();

            match key {
                "size" => {
                    config.size = value.parse()
                        .map_err(|_| format!("Invalid size value: {}", value))?;
                }
                "bit" => {
                    config.bits = value.parse()
                        .map_err(|_| format!("Invalid bit value: {}", value))?;
                }
                "width" => {
                    config.width = match value {
                        "same" => FontWidth::Same,
                        "variable" => FontWidth::Variable,
                        _ => return Err(format!("Invalid width type: {}", value)),
                    };
                }
                _ => {}
            }
        }

        Ok(config)
    }

    /// 解码单个字符的位图数据
    fn decode_char_bitmap(base64_data: &str, config: &FontConfig, width_type: CharWidthType) -> Result<CharBitmap, String> {
        // 解码 base64
        let binary_data = base64_decode(base64_data)
            .map_err(|e| format!("Base64 decode error: {}", e))?;

        let size = config.size as usize;
        let bits = config.bits as usize;

        // 根据字符类型确定宽度
        let char_width = match width_type {
            CharWidthType::Half => size / 2,
            CharWidthType::Full => size,
        };

        // 创建位图矩阵
        let mut bitmap = vec![vec![false; char_width]; size];

        // 根据 bit 数解析二进制数据
        match bits {
            1 => {
                // 每个像素 1 bit
                // 位图数据按实际宽度存储
                let total_bits = char_width * size;
                let total_bytes = (total_bits + 7) / 8;

                if binary_data.len() < total_bytes {
                    return Err(format!(
                        "Insufficient data: expected {} bytes, got {}",
                        total_bytes,
                        binary_data.len()
                    ));
                }

                for y in 0..size {
                    for x in 0..char_width {
                        let bit_index = y * char_width + x;
                        let byte_index = bit_index / 8;
                        let bit_offset = 7 - (bit_index % 8);

                        if byte_index < binary_data.len() {
                            let byte = binary_data[byte_index];
                            bitmap[y][x] = (byte >> bit_offset) & 1 == 1;
                        }
                    }
                }
            }
            _ => {
                return Err(format!("Unsupported bit count: {}", bits));
            }
        }

        Ok(CharBitmap {
            bitmap,
            width: char_width as u32,
            height: config.size,
            width_type,
        })
    }

    /// 获取字符的位图数据
    pub fn get_char(&self, ch: char) -> Option<&CharBitmap> {
        self.chars.get(&ch)
    }

    /// 渲染文本，支持 fallback 字体
    ///
    /// # 参数
    /// - `text`: 要渲染的文本
    /// - `font_size`: 目标字体大小（像素）
    /// - `fallback`: fallback 字体，当当前字体找不到字符时使用
    ///
    /// # 返回
    /// 返回 (位图矩阵, 总宽度, 行高)
    pub fn render_text_with_fallback(&self, text: &str, font_size: u32, fallback: Option<&Font>) -> (Vec<Vec<bool>>, u32, u32) {
        if text.is_empty() {
            return (vec![], 0, 0);
        }

        // 计算缩放比例
        let scale = font_size as f64 / self.config.size as f64;
        let scaled_height = (self.config.size as f64 * scale).ceil() as u32;

        // 计算总宽度，包括空格字符的默认半角宽度
        let mut total_width = 0u32;
        let char_bitmaps: Vec<Option<&CharBitmap>> = text
            .chars()
            .map(|ch| {
                let bitmap = self.get_char(ch)
                    .or_else(|| fallback.and_then(|f| f.get_char(ch)));

                if let Some(bm) = bitmap {
                    total_width += bm.width;
                } else if ch == ' ' {
                    // 空格字符不在字体中时，默认按半角宽度计算
                    total_width += self.config.size / 2;
                }

                bitmap
            })
            .collect();

        if total_width == 0 {
            return (vec![], 0, scaled_height);
        }

        // 应用缩放后的总宽度
        let scaled_total_width = (total_width as f64 * scale).ceil() as u32;

        // 创建结果位图
        let mut result = vec![vec![false; scaled_total_width as usize]; scaled_height as usize];

        let mut x_offset = 0usize;

        for char_bm in char_bitmaps {
            if let Some(bm) = char_bm {
                // 缩放并绘制字符
                self.draw_scaled_char(
                    &mut result,
                    bm,
                    x_offset,
                    scale,
                    scaled_height,
                );

                x_offset += (bm.width as f64 * scale).ceil() as usize;
            } else {
                // 空格字符不在字体中时，按半角宽度推进位置
                let half_width = self.config.size / 2;
                x_offset += (half_width as f64 * scale).ceil() as usize;
            }
        }

        (result, scaled_total_width, scaled_height)
    }

    /// 渲染文本，返回位图数据和总宽度
    ///
    /// # 参数
    /// - `text`: 要渲染的文本
    /// - `font_size`: 目标字体大小（像素）
    ///
    /// # 返回
    /// 返回 (位图矩阵, 总宽度, 行高)
    pub fn render_text(&self, text: &str, font_size: u32) -> (Vec<Vec<bool>>, u32, u32) {
        if text.is_empty() {
            return (vec![], 0, 0);
        }

        // 计算缩放比例
        let scale = font_size as f64 / self.config.size as f64;
        let scaled_height = (self.config.size as f64 * scale).ceil() as u32;

        // 计算总宽度，包括空格字符的默认半角宽度
        let mut total_width = 0u32;
        let char_bitmaps: Vec<Option<&CharBitmap>> = text
            .chars()
            .map(|ch| {
                let bitmap = self.get_char(ch);

                if let Some(bm) = bitmap {
                    total_width += bm.width;
                } else if ch == ' ' {
                    // 空格字符不在字体中时，默认按半角宽度计算
                    total_width += self.config.size / 2;
                }

                bitmap
            })
            .collect();

        if total_width == 0 {
            return (vec![], 0, scaled_height);
        }

        // 应用缩放后的总宽度
        let scaled_total_width = (total_width as f64 * scale).ceil() as u32;

        // 创建结果位图
        let mut result = vec![vec![false; scaled_total_width as usize]; scaled_height as usize];

        let mut x_offset = 0usize;

        for char_bm in char_bitmaps {
            if let Some(bm) = char_bm {
                // 缩放并绘制字符
                self.draw_scaled_char(
                    &mut result,
                    bm,
                    x_offset,
                    scale,
                    scaled_height,
                );

                x_offset += (bm.width as f64 * scale).ceil() as usize;
            } else {
                // 空格字符不在字体中时，按半角宽度推进位置
                let half_width = self.config.size / 2;
                x_offset += (half_width as f64 * scale).ceil() as usize;
            }
        }

        (result, scaled_total_width, scaled_height)
    }

    /// 将缩放后的字符绘制到位图中
    fn draw_scaled_char(
        &self,
        result: &mut Vec<Vec<bool>>,
        char_bm: &CharBitmap,
        x_offset: usize,
        scale: f64,
        scaled_height: u32,
    ) {
        let src_height = char_bm.height as usize;
        let src_width = char_bm.width as usize;
        let scaled_width = (char_bm.width as f64 * scale).ceil() as usize;

        // 使用最近邻插值进行缩放
        for dst_y in 0..scaled_height as usize {
            for dst_x in 0..scaled_width {
                // 计算源坐标
                let src_x = (dst_x as f64 / scale).round() as usize;
                let src_y = (dst_y as f64 / scale).round() as usize;

                // 边界检查
                if src_x < src_width && src_y < src_height {
                    let dst_x_actual = x_offset + dst_x;
                    if dst_x_actual < result[0].len() {
                        result[dst_y][dst_x_actual] = char_bm.bitmap[src_y][src_x];
                    }
                }
            }
        }
    }
}

/// Base64 解码函数
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    const DECODE_TABLE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1, 63,
        52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1,
        -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14,
        15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1,
        -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40,
        41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
    ];

    let input = input.trim();
    let input = input.trim_end_matches('=');

    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let chars: Vec<u8> = input.bytes().collect();

    let mut buffer: u32 = 0;
    let mut bits = 0;

    for &ch in &chars {
        if ch >= 128 {
            return Err("Invalid base64 character".to_string());
        }

        let val = DECODE_TABLE[ch as usize];
        if val < 0 {
            return Err("Invalid base64 character".to_string());
        }

        buffer = (buffer << 6) | (val as u32);
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let config = Font::parse_config("size:32,bit:1,width:same").unwrap();
        assert_eq!(config.size, 32);
        assert_eq!(config.bits, 1);
        assert_eq!(config.width, FontWidth::Same);
    }

    #[test]
    fn test_base64_decode() {
        // "Hello" in base64 is "SGVsbG8="
        let result = base64_decode("SGVsbG8=").unwrap();
        assert_eq!(result, b"Hello");
    }
}