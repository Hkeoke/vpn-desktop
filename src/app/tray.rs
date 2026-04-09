use crossbeam_channel::Sender;
use eframe::egui::Context;
use ksni::{menu::*, Tray};
use std::sync::{Arc, Mutex};

pub enum TrayAction {
    ToggleWindow,
    Quit,
}

pub struct VpnTray {
    pub tx: Sender<TrayAction>,
    pub ctx: Arc<Mutex<Option<Context>>>,
    pub is_connected: bool,
}

impl VpnTray {
    fn send_action(&self, action: TrayAction) {
        let _ = self.tx.send(action);
        if let Some(ctx) = self.ctx.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
    }
}

impl Tray for VpnTray {
    fn id(&self) -> String {
        "vpn-desktop".into()
    }

    fn icon_name(&self) -> String {
        "vpn-desktop".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let width = 64;
        let height = 64;
        let mut data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                let cx = x as f32 - 31.5;
                let cy = y as f32 - 31.5;
                let r = (cx * cx + cy * cy).sqrt();

                // [R, G, B, A] format in our thought process
                let mut rgba = if r <= 28.0 {
                    if self.is_connected {
                        if r > 26.0 {
                            [34, 197, 94, 255] // Lighter green border
                        } else {
                            [21, 128, 61, 255] // Dark green fill
                        }
                    } else {
                        if r > 26.0 {
                            [37, 99, 235, 255] // Blue border
                        } else {
                            [29, 78, 216, 255] // Blue fill
                        }
                    }
                } else {
                    [0, 0, 0, 0]
                };

                if (22..=42).contains(&x) && (28..=46).contains(&y) {
                    rgba = [245, 247, 250, 255];
                }

                if (24..=40).contains(&x) && (16..=32).contains(&y) {
                    let dx = (x as f32 - 32.0) / 8.0;
                    let dy = (y as f32 - 32.0) / 10.0;
                    if dx * dx + dy * dy <= 1.0 && y <= 28 {
                        rgba = [245, 247, 250, 255];
                    }
                }

                if (27..=37).contains(&x) && (19..=31).contains(&y) {
                    let dx = (x as f32 - 32.0) / 5.0;
                    let dy = (y as f32 - 32.0) / 7.0;
                    if dx * dx + dy * dy <= 1.0 && y <= 28 {
                        rgba = if r <= 28.0 {
                            if self.is_connected {
                                [21, 128, 61, 255]
                            } else {
                                [29, 78, 216, 255]
                            }
                        } else {
                            [0, 0, 0, 0]
                        };
                    }
                }

                let keyhole_dx = x as i32 - 32;
                let keyhole_dy = y as i32 - 35;
                if keyhole_dx * keyhole_dx + keyhole_dy * keyhole_dy <= 6 {
                    rgba = if self.is_connected {
                        [21, 128, 61, 255]
                    } else {
                        [29, 78, 216, 255]
                    };
                }

                if (31..=33).contains(&x) && (35..=42).contains(&y) {
                    rgba = if self.is_connected {
                        [21, 128, 61, 255]
                    } else {
                        [29, 78, 216, 255]
                    };
                }

                // ksni expects ARGB32 in network byte order (Big Endian)
                let a = rgba[3];
                let r = rgba[0];
                let g = rgba[1];
                let b = rgba[2];
                data.extend_from_slice(&[a, r, g, b]);
            }
        }

        vec![ksni::Icon {
            width,
            height,
            data,
        }]
    }

    fn title(&self) -> String {
        "VPN Desktop".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send_action(TrayAction::ToggleWindow);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Mostrar / Ocultar".into(),
                activate: Box::new(|this: &mut Self| {
                    this.send_action(TrayAction::ToggleWindow);
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Salir".into(),
                activate: Box::new(|this: &mut Self| {
                    this.send_action(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
