use regex::Regex;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Color([f32; 4]);

impl Color {
    pub fn new(color: &str) -> Self {
        let normalized_color = if is_hex_color(color) {
            hex_to_rgb(color)
        } else if is_hsl_color(color) {
            hsl_to_rgba(color)
        } else if is_hsla_color(color) {
            hsla_to_rgba(color)
        } else if is_rgb_color(color) {
            rgb_to_rgba(color)
        } else if is_rgba_color(color) {
            rgba_to_rgba(color)
        } else if is_color_string(color) {
            normalize_color(color_string_to_rgba(color))
        } else if is_hexa_color(color) {
            hexa_to_argb(color)
        } else {
            [1.0, 1.0, 1.0, 0.0]
        };
        Color(normalized_color)
    }

    pub fn to_array(&self) -> [f32; 4] {
        [self.0[0], self.0[1], self.0[2], self.0[3]]
    }
}

fn get_named_colors() -> HashMap<&'static str, [f32; 4]> {
    let mut colors = HashMap::new();
    colors.insert("aliceblue", [250.0, 235.0, 215.0, 255.0]);
    colors.insert("antiquewhite", [250.0, 235.0, 215.0, 255.0]);
    colors.insert("aqua", [0.0, 255.0, 255.0, 255.0]);
    colors.insert("aquamarine", [127.0, 255.0, 212.0, 255.0]);
    colors.insert("azure", [240.0, 255.0, 255.0, 255.0]);
    colors.insert("beige", [245.0, 245.0, 220.0, 255.0]);
    colors.insert("bisque", [255.0, 228.0, 196.0, 255.0]);
    colors.insert("black", [0.0, 0.0, 0.0, 255.0]);
    colors.insert("blanchedalmond", [255.0, 235.0, 205.0, 255.0]);
    colors.insert("blue", [0.0, 0.0, 255.0, 255.0]);
    colors.insert("blueviolet", [138.0, 43.0, 226.0, 255.0]);
    colors.insert("brown", [165.0, 42.0, 42.0, 255.0]);
    colors.insert("burlywood", [222.0, 184.0, 135.0, 255.0]);
    colors.insert("cadetblue", [95.0, 158.0, 160.0, 255.0]);
    colors.insert("chartreuse", [127.0, 255.0, 0.0, 255.0]);
    colors.insert("chocolate", [210.0, 105.0, 30.0, 255.0]);
    colors.insert("coral", [255.0, 127.0, 80.0, 255.0]);
    colors.insert("cornflowerblue", [100.0, 149.0, 237.0, 255.0]);
    colors.insert("cornsilk", [255.0, 248.0, 220.0, 255.0]);
    colors.insert("crimson", [220.0, 20.0, 60.0, 255.0]);
    colors.insert("cyan", [0.0, 255.0, 255.0, 255.0]);
    colors.insert("darkblue", [0.0, 0.0, 139.0, 255.0]);
    colors.insert("darkcyan", [0.0, 139.0, 139.0, 255.0]);
    colors.insert("darkgoldenrod", [184.0, 134.0, 11.0, 255.0]);
    colors.insert("darkgray", [169.0, 169.0, 169.0, 255.0]);
    colors.insert("darkgreen", [0.0, 100.0, 0.0, 255.0]);
    colors.insert("darkkhaki", [189.0, 183.0, 107.0, 255.0]);
    colors.insert("darkmagenta", [139.0, 0.0, 139.0, 255.0]);
    colors.insert("darkolivegreen", [85.0, 107.0, 47.0, 255.0]);
    colors.insert("darkorange", [255.0, 140.0, 0.0, 255.0]);
    colors.insert("darkorchid", [153.0, 50.0, 204.0, 255.0]);
    colors.insert("darkred", [139.0, 0.0, 0.0, 255.0]);
    colors.insert("darksalmon", [233.0, 150.0, 122.0, 255.0]);
    colors.insert("darkseagreen", [143.0, 188.0, 143.0, 255.0]);
    colors.insert("darkslateblue", [72.0, 61.0, 139.0, 255.0]);
    colors.insert("darkslategray", [47.0, 79.0, 79.0, 255.0]);
    colors.insert("darkturquoise", [0.0, 206.0, 209.0, 255.0]);
    colors.insert("darkviolet", [148.0, 0.0, 211.0, 255.0]);
    colors.insert("deeppink", [255.0, 20.0, 147.0, 255.0]);
    colors.insert("deepskyblue", [0.0, 191.0, 255.0, 255.0]);
    colors.insert("dimgray", [105.0, 105.0, 105.0, 255.0]);
    colors.insert("dodgerblue", [30.0, 144.0, 255.0, 255.0]);
    colors.insert("firebrick", [178.0, 34.0, 34.0, 255.0]);
    colors.insert("floralwhite", [255.0, 250.0, 240.0, 255.0]);
    colors.insert("forestgreen", [34.0, 139.0, 34.0, 255.0]);
    colors.insert("fuchsia", [255.0, 0.0, 255.0, 255.0]);
    colors.insert("gainsboro", [220.0, 220.0, 220.0, 255.0]);
    colors.insert("ghostwhite", [248.0, 248.0, 255.0, 255.0]);
    colors.insert("gold", [255.0, 215.0, 0.0, 255.0]);
    colors.insert("goldenrod", [218.0, 165.0, 32.0, 255.0]);
    colors.insert("gray", [128.0, 128.0, 128.0, 255.0]);
    colors.insert("green", [0.0, 128.0, 0.0, 255.0]);
    colors.insert("greenyellow", [173.0, 255.0, 47.0, 255.0]);
    colors.insert("honeydew", [240.0, 255.0, 240.0, 255.0]);
    colors.insert("hotpink", [255.0, 105.0, 180.0, 255.0]);
    colors.insert("indianred", [205.0, 92.0, 92.0, 255.0]);
    colors.insert("indigo", [75.0, 0.0, 130.0, 255.0]);
    colors.insert("ivory", [255.0, 255.0, 240.0, 255.0]);
    colors.insert("khaki", [240.0, 230.0, 140.0, 255.0]);
    colors.insert("lavender", [230.0, 230.0, 250.0, 255.0]);
    colors.insert("lavenderblush", [255.0, 240.0, 245.0, 255.0]);
    colors.insert("lawngreen", [124.0, 252.0, 0.0, 255.0]);
    colors.insert("lemonchiffon", [255.0, 250.0, 205.0, 255.0]);
    colors.insert("lightblue", [173.0, 216.0, 230.0, 255.0]);
    colors.insert("lightcoral", [240.0, 128.0, 128.0, 255.0]);
    colors.insert("lightcyan", [224.0, 255.0, 255.0, 255.0]);
    colors.insert("lightgoldenrodyellow", [250.0, 250.0, 210.0, 255.0]);
    colors.insert("lightgray", [211.0, 211.0, 211.0, 255.0]);
    colors.insert("lightgreen", [144.0, 238.0, 144.0, 255.0]);
    colors.insert("lightpink", [255.0, 182.0, 193.0, 255.0]);
    colors.insert("lightsalmon", [255.0, 160.0, 122.0, 255.0]);
    colors.insert("lightseagreen", [32.0, 178.0, 170.0, 255.0]);
    colors.insert("lightskyblue", [135.0, 206.0, 250.0, 255.0]);
    colors.insert("lightslategray", [119.0, 136.0, 153.0, 255.0]);
    colors.insert("lightsteelblue", [176.0, 196.0, 222.0, 255.0]);
    colors.insert("lightyellow", [255.0, 255.0, 224.0, 255.0]);
    colors.insert("lime", [0.0, 255.0, 0.0, 255.0]);
    colors.insert("limegreen", [50.0, 205.0, 50.0, 255.0]);
    colors.insert("linen", [250.0, 240.0, 230.0, 255.0]);
    colors.insert("magenta", [255.0, 0.0, 255.0, 255.0]);
    colors.insert("maroon", [128.0, 0.0, 0.0, 255.0]);
    colors.insert("mediumaquamarine", [102.0, 205.0, 170.0, 255.0]);
    colors.insert("mediumblue", [0.0, 0.0, 205.0, 255.0]);
    colors.insert("mediumorchid", [186.0, 85.0, 211.0, 255.0]);
    colors.insert("mediumpurple", [147.0, 112.0, 219.0, 255.0]);
    colors.insert("mediumseagreen", [60.0, 179.0, 113.0, 255.0]);
    colors.insert("mediumslateblue", [123.0, 104.0, 238.0, 255.0]);
    colors.insert("mediumspringgreen", [0.0, 250.0, 154.0, 255.0]);
    colors.insert("mediumturquoise", [72.0, 209.0, 204.0, 255.0]);
    colors.insert("mediumvioletred", [199.0, 21.0, 133.0, 255.0]);
    colors.insert("midnightblue", [25.0, 25.0, 112.0, 255.0]);
    colors.insert("mintcream", [245.0, 255.0, 250.0, 255.0]);
    colors.insert("mistyrose", [255.0, 228.0, 225.0, 255.0]);
    colors.insert("moccasin", [255.0, 228.0, 181.0, 255.0]);
    colors.insert("navajowhite", [255.0, 222.0, 173.0, 255.0]);
    colors.insert("navy", [0.0, 0.0, 128.0, 255.0]);
    colors.insert("oldlace", [253.0, 245.0, 230.0, 255.0]);
    colors.insert("olive", [128.0, 128.0, 0.0, 255.0]);
    colors.insert("olivedrab", [107.0, 142.0, 35.0, 255.0]);
    colors.insert("orange", [255.0, 165.0, 0.0, 255.0]);
    colors.insert("orangered", [255.0, 69.0, 0.0, 255.0]);
    colors.insert("orchid", [218.0, 112.0, 214.0, 255.0]);
    colors.insert("palegoldenrod", [238.0, 232.0, 170.0, 255.0]);
    colors.insert("palegreen", [152.0, 251.0, 152.0, 255.0]);
    colors.insert("paleturquoise", [175.0, 238.0, 238.0, 255.0]);
    colors.insert("palevioletred", [219.0, 112.0, 147.0, 255.0]);
    colors.insert("papayawhip", [255.0, 239.0, 213.0, 255.0]);
    colors.insert("peachpuff", [255.0, 218.0, 185.0, 255.0]);
    colors.insert("peru", [205.0, 133.0, 63.0, 255.0]);
    colors.insert("pink", [255.0, 192.0, 203.0, 255.0]);
    colors.insert("plum", [221.0, 160.0, 221.0, 255.0]);
    colors.insert("powderblue", [176.0, 224.0, 230.0, 255.0]);
    colors.insert("purple", [128.0, 0.0, 128.0, 255.0]);
    colors.insert("rebeccapurple", [102.0, 51.0, 153.0, 255.0]);
    colors.insert("red", [255.0, 0.0, 0.0, 255.0]);
    colors.insert("rosybrown", [188.0, 143.0, 143.0, 255.0]);
    colors.insert("royalblue", [65.0, 105.0, 225.0, 255.0]);
    colors.insert("saddlebrown", [139.0, 69.0, 19.0, 255.0]);
    colors.insert("salmon", [250.0, 128.0, 114.0, 255.0]);
    colors.insert("sandybrown", [244.0, 164.0, 96.0, 255.0]);
    colors.insert("seagreen", [46.0, 139.0, 87.0, 255.0]);
    colors.insert("seashell", [255.0, 245.0, 238.0, 255.0]);
    colors.insert("sienna", [160.0, 82.0, 45.0, 255.0]);
    colors.insert("silver", [192.0, 192.0, 192.0, 255.0]);
    colors.insert("skyblue", [135.0, 206.0, 235.0, 255.0]);
    colors.insert("slateblue", [106.0, 90.0, 205.0, 255.0]);
    colors.insert("slategray", [112.0, 128.0, 144.0, 255.0]);
    colors.insert("snow", [255.0, 250.0, 250.0, 255.0]);
    colors.insert("springgreen", [0.0, 255.0, 127.0, 255.0]);
    colors.insert("steelblue", [70.0, 130.0, 180.0, 255.0]);
    colors.insert("tan", [210.0, 180.0, 140.0, 255.0]);
    colors.insert("teal", [0.0, 128.0, 128.0, 255.0]);
    colors.insert("thistle", [216.0, 191.0, 216.0, 255.0]);
    colors.insert("tomato", [255.0, 99.0, 71.0, 255.0]);
    colors.insert("turquoise", [64.0, 224.0, 208.0, 255.0]);
    colors.insert("violet", [238.0, 130.0, 238.0, 255.0]);
    colors.insert("wheat", [245.0, 222.0, 179.0, 255.0]);
    colors.insert("white", [255.0, 255.0, 255.0, 255.0]);
    colors.insert("whitesmoke", [245.0, 245.0, 245.0, 255.0]);
    colors.insert("yellow", [255.0, 255.0, 0.0, 255.0]);
    colors.insert("yellowgreen", [154.0, 205.0, 50.0, 255.0]);
    colors
}

fn normalize_color(color: [f32; 4]) -> [f32; 4] {
    [
        color[0] / 255.0,
        color[1] / 255.0,
        color[2] / 255.0,
        color[3] / 255.0,
    ]
}

fn is_hex_color(color: &str) -> bool {
    let re = Regex::new(r"^#(?:[0-9a-fA-F]{3}){1,2}$").unwrap();
    re.is_match(color)
}

fn hex_to_rgb(color: &str) -> [f32; 4] {
    let color = color.trim_start_matches('#');
    let (r, g, b) = if color.len() == 3 {
        (
            u8::from_str_radix(&color[0..1].repeat(2), 16).unwrap(),
            u8::from_str_radix(&color[1..2].repeat(2), 16).unwrap(),
            u8::from_str_radix(&color[2..3].repeat(2), 16).unwrap(),
        )
    } else {
        (
            u8::from_str_radix(&color[0..2], 16).unwrap(),
            u8::from_str_radix(&color[2..4], 16).unwrap(),
            u8::from_str_radix(&color[4..6], 16).unwrap(),
        )
    };
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

fn is_hexa_color(color: &str) -> bool {
    let re = Regex::new(r"^#(?:[0-9a-fA-F]{8})$").unwrap();
    re.is_match(color)
}

fn hexa_to_argb(color: &str) -> [f32; 4] {
    let color = color.trim_start_matches('#');
    let a = u8::from_str_radix(&color[0..2], 16).unwrap();
    let r = u8::from_str_radix(&color[2..4], 16).unwrap();
    let g = u8::from_str_radix(&color[4..6], 16).unwrap();
    let b = u8::from_str_radix(&color[6..8], 16).unwrap();
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

fn is_hsl_color(color: &str) -> bool {
    let re = Regex::new(r"hsl\((\d+),\s*(\d+)%,\s*(\d+)%\)").unwrap();
    re.is_match(color)
}

fn hsl_to_rgba(color: &str) -> [f32; 4] {
    let re = Regex::new(r"hsl\((\d+),\s*(\d+)%,\s*(\d+)%\)").unwrap();
    let caps = re.captures(&color).unwrap();
    let h = caps.get(1).unwrap().as_str().parse::<f32>().unwrap();
    let s = caps.get(2).unwrap().as_str().parse::<f32>().unwrap();
    let l = caps.get(3).unwrap().as_str().parse::<f32>().unwrap();
    let s = s / 100.0;
    let l = l / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let r = (r + m) * 255.0;
    let g = (g + m) * 255.0;
    let b = (b + m) * 255.0;
    [r / 255.0, g / 255.0, b / 255.0, 1.0]
}

fn is_hsla_color(color: &str) -> bool {
    let re = Regex::new(r"hsla\((\d+),\s*(\d+)%,\s*(\d+)%,\s*(\d+\.\d+)\)").unwrap();
    re.is_match(color)
}

fn hsla_to_rgba(color: &str) -> [f32; 4] {
    let re = Regex::new(r"hsla\((\d+),\s*(\d+)%,\s*(\d+)%,\s*(\d+\.\d+)\)").unwrap();
    let caps = re.captures(&color).unwrap();
    let h = caps.get(1).unwrap().as_str().parse::<f32>().unwrap();
    let s = caps.get(2).unwrap().as_str().parse::<f32>().unwrap();
    let l = caps.get(3).unwrap().as_str().parse::<f32>().unwrap();
    let a = caps.get(4).unwrap().as_str().parse::<f32>().unwrap();
    let s = s / 100.0;
    let l = l / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let r = (r + m) * 255.0;
    let g = (g + m) * 255.0;
    let b = (b + m) * 255.0;
    [r / 255.0, g / 255.0, b / 255.0, a]
}

fn is_rgb_color(color: &str) -> bool {
    let re = Regex::new(r"rgb\((\d+),\s*(\d+),\s*(\d+)\)").unwrap();
    re.is_match(color)
}

fn rgb_to_rgba(color: &str) -> [f32; 4] {
    let re = Regex::new(r"rgb\((\d+),\s*(\d+),\s*(\d+)\)").unwrap();
    let caps = re.captures(&color).unwrap();
    let r = caps.get(1).unwrap().as_str().parse::<f32>().unwrap();
    let g = caps.get(2).unwrap().as_str().parse::<f32>().unwrap();
    let b = caps.get(3).unwrap().as_str().parse::<f32>().unwrap();
    [r / 255.0, g / 255.0, b / 255.0, 1.0]
}

fn is_rgba_color(color: &str) -> bool {
    let re = Regex::new(r"rgba\((\d+),\s*(\d+),\s*(\d+),\s*(\d+\.\d+)\)").unwrap();
    re.is_match(color)
}

fn rgba_to_rgba(color: &str) -> [f32; 4] {
    let re = Regex::new(r"rgba\((\d+),\s*(\d+),\s*(\d+),\s*(\d+\.\d+)\)").unwrap();
    let caps = re.captures(&color).unwrap();
    let r = caps.get(1).unwrap().as_str().parse::<f32>().unwrap();
    let g = caps.get(2).unwrap().as_str().parse::<f32>().unwrap();
    let b = caps.get(3).unwrap().as_str().parse::<f32>().unwrap();
    let a = caps.get(4).unwrap().as_str().parse::<f32>().unwrap();
    [r / 255.0, g / 255.0, b / 255.0, a]
}

fn is_color_string(color: &str) -> bool {
    let colors = get_named_colors();
    colors.contains_key(&color.to_lowercase().as_str())
}

fn color_string_to_rgba(color: &str) -> [f32; 4] {
    let colors = get_named_colors();
    *colors.get(color.to_lowercase().as_str()).unwrap()
}
