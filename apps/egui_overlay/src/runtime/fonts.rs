use std::sync::Arc;

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

pub(crate) fn install_system_fonts(ctx: &Context) {
    let Some((name, bytes)) = load_system_cjk_font() else {
        return;
    };

    let mut fonts = FontDefinitions::default();
    fonts
        .font_data
        .insert(name.clone(), Arc::new(FontData::from_owned(bytes)));

    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.clone());
    }

    ctx.set_fonts(fonts);
}

// Prefer common Windows CJK fonts so overlay text remains readable in localized builds.
fn load_system_cjk_font() -> Option<(String, Vec<u8>)> {
    let font_paths = [
        ("MicrosoftYaHei", r"C:\Windows\Fonts\msyh.ttc"),
        ("DengXian", r"C:\Windows\Fonts\Deng.ttf"),
        ("SimHei", r"C:\Windows\Fonts\simhei.ttf"),
        ("SimSun", r"C:\Windows\Fonts\simsun.ttc"),
        ("Meiryo", r"C:\Windows\Fonts\meiryo.ttc"),
        ("YuGothic", r"C:\Windows\Fonts\YuGothR.ttc"),
        ("MalgunGothic", r"C:\Windows\Fonts\malgun.ttf"),
        ("Msgothic", r"C:\Windows\Fonts\msgothic.ttc"),
    ];

    font_paths.iter().find_map(|(name, path)| {
        std::fs::read(path)
            .ok()
            .filter(|bytes| !bytes.is_empty())
            .map(|bytes| ((*name).to_owned(), bytes))
    })
}
