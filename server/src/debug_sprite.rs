// 临时调试: 导出棋子精灵图 16 帧
#[cfg(test)]
mod tests {
    use image::RgbaImage;

    #[test]
    fn dump_sprite_sheet() {
        let png = include_bytes!("../../src/assets/images/piece.png");
        let img = image::load_from_memory(png).unwrap().to_rgba8();
        let (w, h) = img.dimensions();
        let frame_h = h / 16;
        let scale = 2;
        let mut sheet = RgbaImage::new((w * scale) * 16 + 15 * 6, frame_h * scale);
        for i in 0..16 {
            let frame = image::imageops::crop_imm(&img, 0, i as u32 * frame_h, w, frame_h).to_image();
            let big = image::imageops::resize(&frame, w * scale, frame_h * scale, image::imageops::FilterType::Nearest);
            image::imageops::overlay(&mut sheet, &big, ((w * scale + 6) * i) as i64, 0);
        }
        std::fs::create_dir_all("../scripts/out").ok();
        sheet.save("../scripts/out/sprite-sheet.png").unwrap();
        println!("frame count=16 frame_h={frame_h} w={w} h={h}");
    }
}
