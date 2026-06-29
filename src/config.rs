use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    #[serde(default = "default_idle_bg_opacity")]
    pub idle_bg_opacity: u8,
    #[serde(default = "default_row_bg_opacity")]
    pub row_bg_opacity: u8,
    #[serde(default = "default_cell_bg_opacity")]
    pub cell_bg_opacity: u8,
    #[serde(default = "default_font_size_divisor")]
    pub font_size_divisor: u32,
    #[serde(default = "default_grid_color")]
    pub grid_color: String,
    #[serde(default = "default_text_color")]
    pub text_color: String,
    #[serde(default = "default_grid_opacity")]
    pub grid_opacity: u8,
    #[serde(default = "default_scale_ratio")]
    pub scale_ratio: f64,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
}

impl ConfigFile {
    pub fn logical_width(&self) -> Option<u32> {
        self.window_width
            .map(|w| (f64::from(w) / self.scale_ratio) as u32)
    }
    pub fn logical_height(&self) -> Option<u32> {
        self.window_height
            .map(|h| (f64::from(h) / self.scale_ratio) as u32)
    }
}

fn default_idle_bg_opacity() -> u8 {
    0
}
fn default_row_bg_opacity() -> u8 {
    0
}
fn default_cell_bg_opacity() -> u8 {
    0
}
fn default_font_size_divisor() -> u32 {
    2
}
fn default_grid_color() -> String {
    "888888".to_string()
}
fn default_text_color() -> String {
    "FFFFFF".to_string()
}
fn default_grid_opacity() -> u8 {
    255
}
fn default_scale_ratio() -> f64 {
    1.0
}

impl Default for ConfigFile {
    fn default() -> Self {
        ConfigFile {
            idle_bg_opacity: default_idle_bg_opacity(),
            row_bg_opacity: default_row_bg_opacity(),
            cell_bg_opacity: default_cell_bg_opacity(),
            font_size_divisor: default_font_size_divisor(),
            grid_color: default_grid_color(),
            text_color: default_text_color(),
            grid_opacity: default_grid_opacity(),
            scale_ratio: default_scale_ratio(),
            window_width: None,
            window_height: None,
        }
    }
}

pub fn parse_hex_color(hex: &str) -> u32 {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

pub fn load_config() -> ConfigFile {
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("chopsticks/config.toml");

    if let Ok(content) = std::fs::read_to_string(&config_path)
        && let Ok(config) = toml::from_str(&content)
    {
        return config;
    }

    ConfigFile::default()
}
