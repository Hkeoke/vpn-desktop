use eframe::egui::Color32;

pub fn log_line_color(line: &str) -> Color32 {
    let lower = line.to_ascii_lowercase();

    if line.starts_with('━') || line.starts_with("━━") {
        return Color32::from_rgb(200, 200, 255);
    }

    if lower.contains("initialization sequence completed") {
        return Color32::from_rgb(50, 220, 100);
    }

    if lower.contains("[e]")
        || lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fatal")
    {
        return Color32::from_rgb(220, 80, 80);
    }

    if lower.contains("warning") || lower.contains("warn") {
        return Color32::from_rgb(240, 190, 50);
    }

    if line.starts_with('$') {
        return Color32::from_rgb(160, 210, 255);
    }

    if lower.contains("connected") || lower.contains("tunnel") {
        return Color32::from_rgb(120, 220, 120);
    }

    Color32::from_rgb(200, 200, 200)
}
