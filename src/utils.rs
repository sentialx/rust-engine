pub fn S(st: &str) -> String {
  st.to_string()
}

#[derive(Clone, Debug)]
pub struct KeyValue(pub String, pub String);

impl KeyValue {
  pub fn new() -> KeyValue {
    KeyValue {
      0: "".to_string(),
      1: "".to_string(),
    }
  }

  pub fn new_values(key: &str, value: &str) -> KeyValue {
    KeyValue {
      0: key.to_string(),
      1: value.to_string(),
    }
  }
}
