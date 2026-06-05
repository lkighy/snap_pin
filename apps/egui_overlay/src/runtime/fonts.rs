use std::sync::Arc;

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

pub(crate) fn install_system_fonts(ctx: &Context) {
    let fonts_to_install = load_system_cjk_fonts();
    if fonts_to_install.is_empty() {
        return;
    }

    let mut fonts = FontDefinitions::default();
    let mut installed_names = Vec::with_capacity(fonts_to_install.len());
    for (name, data) in fonts_to_install {
        fonts.font_data.insert(name.clone(), Arc::new(data));
        installed_names.push(name);
    }

    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let family_fonts = fonts.families.entry(family).or_default();
        for name in installed_names.iter().rev() {
            family_fonts.insert(0, name.clone());
        }
    }

    ctx.set_fonts(fonts);
}

// Prefer common Windows CJK fonts so overlay text remains readable in localized builds.
fn load_system_cjk_fonts() -> Vec<(String, FontData)> {
    let font_paths = [
        ("DengXian", r"C:\Windows\Fonts\Deng.ttf", &[0][..]),
        ("SimHei", r"C:\Windows\Fonts\simhei.ttf", &[0][..]),
        ("MicrosoftYaHeiUi", r"C:\Windows\Fonts\msyh.ttf", &[0][..]),
        ("MicrosoftYaHei", r"C:\Windows\Fonts\msyh.ttc", &[0][..]),
        (
            "MicrosoftYaHeiBold",
            r"C:\Windows\Fonts\msyhbd.ttc",
            &[0][..],
        ),
        ("SimSun", r"C:\Windows\Fonts\simsun.ttc", &[0][..]),
        ("Meiryo", r"C:\Windows\Fonts\meiryo.ttc", &[0][..]),
        ("YuGothic", r"C:\Windows\Fonts\YuGothR.ttc", &[0][..]),
        ("MalgunGothic", r"C:\Windows\Fonts\malgun.ttf", &[0][..]),
        ("Msgothic", r"C:\Windows\Fonts\msgothic.ttc", &[0][..]),
    ];

    font_paths
        .iter()
        .filter_map(|(name, path, indices)| {
            std::fs::read(path)
                .ok()
                .filter(|bytes| !bytes.is_empty())
                .map(|bytes| (*name, bytes, *indices))
        })
        .flat_map(|(name, bytes, indices)| {
            indices.iter().map(move |index| {
                let mut data = FontData::from_owned(bytes.clone());
                data.index = *index;
                (format!("{name}-{index}"), data)
            })
        })
        .collect()
}
