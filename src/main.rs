use anyhow::Result;
use eframe::egui;
use vpn_desktop::app::App;

fn main() -> Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("VPN Desktop")
            .with_app_id("vpn-desktop")
            .with_inner_size([400.0, 600.0])
            .with_min_inner_size([420.0, 560.0])
            .with_resizable(false)
            .with_icon(window_icon()),
        ..Default::default()
    };

    let (tray_tx, tray_rx) = crossbeam_channel::unbounded();
    let tray_ctx = std::sync::Arc::new(std::sync::Mutex::new(None));

    let tray = vpn_desktop::app::tray::VpnTray {
        tx: tray_tx,
        ctx: std::sync::Arc::clone(&tray_ctx),
        is_connected: false,
    };
    let tray_handle = ksni::blocking::TrayMethods::spawn(tray).unwrap();

    eframe::run_native(
        "VPN Desktop",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(App::new(
                cc,
                Some(tray_rx),
                Some(tray_ctx),
                Some(tray_handle),
            )))
        }),
    )
    .map_err(|err| anyhow::anyhow!("No se pudo iniciar la aplicación: {}", err))?;

    Ok(())
}

fn window_icon() -> egui::IconData {
    let width = 64;
    let height = 64;
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);

    for y in 0..height {
        for x in 0..width {
            let cx = x as f32 - 31.5;
            let cy = y as f32 - 31.5;
            let r = (cx * cx + cy * cy).sqrt();

            let mut pixel = if r <= 28.0 {
                if r > 26.0 {
                    [37, 99, 235, 255]
                } else {
                    [29, 78, 216, 255]
                }
            } else {
                [0, 0, 0, 0]
            };

            if (22..=42).contains(&x) && (28..=46).contains(&y) {
                pixel = [245, 247, 250, 255];
            }

            if (24..=40).contains(&x) && (16..=32).contains(&y) {
                let dx = (x as f32 - 32.0) / 8.0;
                let dy = (y as f32 - 32.0) / 10.0;
                if dx * dx + dy * dy <= 1.0 && y <= 28 {
                    pixel = [245, 247, 250, 255];
                }
            }

            if (27..=37).contains(&x) && (19..=31).contains(&y) {
                let dx = (x as f32 - 32.0) / 5.0;
                let dy = (y as f32 - 32.0) / 7.0;
                if dx * dx + dy * dy <= 1.0 && y <= 28 {
                    pixel = if r <= 28.0 {
                        [29, 78, 216, 255]
                    } else {
                        [0, 0, 0, 0]
                    };
                }
            }

            let keyhole_dx = x as i32 - 32;
            let keyhole_dy = y as i32 - 35;
            if keyhole_dx * keyhole_dx + keyhole_dy * keyhole_dy <= 6 {
                pixel = [29, 78, 216, 255];
            }

            if (31..=33).contains(&x) && (35..=42).contains(&y) {
                pixel = [29, 78, 216, 255];
            }

            rgba.extend_from_slice(&pixel);
        }
    }

    egui::IconData {
        rgba,
        width,
        height,
    }
}
