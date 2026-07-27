//! 二维码生成模块

use anyhow::Result;

/// 生成二维码并返回 PNG 格式的 base64 字符串
pub fn generate_qr_png_base64(data: &str, size: u32) -> Result<String> {
    use qrcode::QrCode;
    use image::Luma;

    // 生成二维码矩阵
    let code = QrCode::new(data.as_bytes())
        .map_err(|e| anyhow::anyhow!("二维码生成失败: {}", e))?;

    // 渲染为灰度图像
    let image = code.render::<Luma<u8>>().min_dimensions(size, size).build();

    // 转为 PNG 字节
    let mut png_bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    image
        .write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| anyhow::anyhow!("PNG 编码失败: {}", e))?;

    // Base64 编码
    Ok(base64::encode(&png_bytes))
}

/// 生成配对二维码数据
pub fn generate_pairing_qr_data(ip: &str, port: u16, pin: &str) -> String {
    format!("qingqi://pair?ip={}&port={}&pin={}", ip, port, pin)
}
