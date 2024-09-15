use crate::lisia_colors::Color;

pub type ColorTuple = (f64, f64, f64);

pub type ColorTupleA = (f64, f64, f64, f64);

fn make_parse_err(s: &str, t: &str) -> String {
  format!("Error while parsing color {} as {}", s, t)
}

fn make_parse_css_err(s: &str) -> String {
  format!("Error while parsing css color {}", s)
}

pub fn hex_to_rgb(s: &str) -> Result<ColorTuple, String> {
  let mut hex = s.replace("#", "").to_lowercase();
  let hex_chars = hex.chars().collect::<Vec<char>>();
  let count = hex_chars.len();

  if count == 3 {
    hex = hex_chars
      .iter()
      .map(|c| c.to_string().repeat(2))
      .collect::<Vec<String>>()
      .join("");
  } else if count != 6 {
    return Err(make_parse_err(s, "rgb"));
  }

  match usize::from_str_radix(&hex, 16) {
    Ok(num) => Ok(hex_num_to_rgb(num)),
    Err(_) => Err(make_parse_err(s, "rgb")),
  }
}

pub fn parse_css_color(s: &str) -> Result<ColorTupleA, String> {
  let s = s.to_string().replace("!important", "");
  // if s.starts_with("#") {
  //   match hex_to_rgb(&s) {
  //     Ok(r) => Ok((r.0, r.1, r.2, 255.0)),
  //     Err(_) => Err(make_parse_css_err(&s)),
  //   }
  // } else {
  //   if s.starts_with("rgb") {
  //     let start = s.find("(").unwrap_or(0) + 1;
  //     let end = s.find(")").unwrap_or(0);
  //     let parts = s[start..end].split(",").collect::<Vec<&str>>();

  //     if parts.len() == 3 {
  //       let r = parts[0].trim().parse::<f64>().unwrap_or(0.0);
  //       let g = parts[1].trim().parse::<f64>().unwrap_or(0.0);
  //       let b = parts[2].trim().parse::<f64>().unwrap_or(0.0);

  //       return Ok((r, g, b, 255.0));
  //     } else if parts.len() == 4 {
  //       let r = parts[0].trim().parse::<f64>().unwrap_or(0.0);
  //       let g = parts[1].trim().parse::<f64>().unwrap_or(0.0);
  //       let b = parts[2].trim().parse::<f64>().unwrap_or(0.0);
  //       let a = parts[3].trim().parse::<f64>().unwrap_or(0.0);

  //       return Ok((r, g, b, a * 255.0));
  //     }
  //   }
  //   Err(make_parse_css_err(&s))
  // }
  let arr = Color::new(&s).to_array().map(|x| x as f64);
  Ok((arr[0], arr[1], arr[2], arr[3]))
}

fn hex_num_to_rgb(num: usize) -> ColorTuple {
  let r = (num >> 16) as f64;
  let g = ((num >> 8) & 0x00FF) as f64;
  let b = (num & 0x0000_00FF) as f64;

  (r, g, b)
}
