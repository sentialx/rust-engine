use crate::{css_value::CssValue, styles::{ComputedMargin, PropertyImpl, ScalarEvaluationContext, Style, StyleScalar}};

#[derive(Clone, Debug)]
pub struct Margin {
  pub top: MarginComponent,
  pub right: MarginComponent,
  pub bottom: MarginComponent,
  pub left: MarginComponent,
}

impl Margin {
  pub fn new(
    top: CssValue,
    right: CssValue,
    bottom: CssValue,
    left: CssValue,
  ) -> Margin {
    Margin {
      top: MarginComponent::new(top),
      right: MarginComponent::new(right),
      bottom: MarginComponent::new(bottom),
      left: MarginComponent::new(left),
    }
  }

  pub fn empty() -> Margin {
    Margin {
      top: MarginComponent::empty(),
      right: MarginComponent::empty(),
      bottom: MarginComponent::empty(),
      left: MarginComponent::empty(),
    }
  }

  pub fn evaluate(&mut self, ctx: &ScalarEvaluationContext) -> &Self {
    self.top.evaluate(&ctx);
    self.right.evaluate(&ctx);
    self.bottom.evaluate(&ctx);
    self.left.evaluate(&ctx);
    self
  }

  pub fn to_computed(&self) -> ComputedMargin {
    ComputedMargin {
      top: self.top.get(),
      right: self.right.get(),
      bottom: self.bottom.get(),
      left: self.left.get(),
    }
  }
}

impl PropertyImpl for Margin {
  fn create_inherited(&self, inherit_style: &Style) -> Margin {
    let mut margin = self.clone();

    margin.top = self.top.create_inherited(inherit_style);
    margin.right = self.right.create_inherited(inherit_style);
    margin.bottom = self.bottom.create_inherited(inherit_style);
    margin.left = self.left.create_inherited(inherit_style);

    margin
  }

  fn from_value(value: CssValue) -> Margin {
    match value {
      CssValue::Multiple(values) => {
        let mut top = MarginComponent::empty();
        let mut right = MarginComponent::empty();
        let mut bottom = MarginComponent::empty();
        let mut left = MarginComponent::empty();
        match values.len() {
          1 => {
            top = MarginComponent::from_value(values[0].clone());
            right = MarginComponent::from_value(values[0].clone());
            bottom = MarginComponent::from_value(values[0].clone());
            left = MarginComponent::from_value(values[0].clone());
          },
          2 => {
            top = MarginComponent::from_value(values[0].clone());
            right = MarginComponent::from_value(values[1].clone());
            bottom = MarginComponent::from_value(values[0].clone());
            left = MarginComponent::from_value(values[1].clone());
          },
          3 => {
            top = MarginComponent::from_value(values[0].clone());
            right =   MarginComponent::from_value(values[1].clone());
            bottom = MarginComponent::from_value(values[2].clone());
            left = MarginComponent::from_value(values[1].clone());
          },
          4 => {
            top = MarginComponent::from_value(values[0].clone());
            right = MarginComponent::from_value(values[1].clone());
            bottom = MarginComponent::from_value(values[2].clone());
            left = MarginComponent::from_value(values[3].clone());
          },
          _ => {},
        }
        Margin {
          top: top,
          right: right,
          bottom: bottom,
          left: left,
        }
      },
      _ => Margin::empty(),
    }
  }
}

#[derive(Clone, Debug)]
pub struct MarginComponent {
  pub value: StyleScalar,
}

impl MarginComponent {
  pub fn new(value: CssValue) -> MarginComponent {
    MarginComponent { value: StyleScalar::new(value) }
  }

  pub fn empty() -> MarginComponent {
    MarginComponent { value: StyleScalar::zero() }
  }

  pub fn has_numeric_value(&self) -> bool {
    match self.value.value {
      CssValue::Size(_) => true,
      CssValue::Number(_) => true,
      _ => false,
    }
  }

  pub fn evaluate(&mut self, ctx: &ScalarEvaluationContext) -> &Self {
    self.value.evaluate(&ctx);
    self
  }

  pub fn get(&self) -> f32 {
    match self.value.get() {
      Some(value) => value,
      None => 0.0,
    }
  }
}

impl PropertyImpl for MarginComponent {
  fn create_inherited(&self, inherit_style: &Style) -> MarginComponent {
    let mut margin = MarginComponent { value: self.value.clone() };
    if self.value.value.is_inherit() {
      margin = inherit_style.margin.top.clone();
    }
    margin
  }

  fn from_value(value: CssValue) -> MarginComponent {
    match &value {
      CssValue::Size(_) => MarginComponent::new(value.clone()),
      CssValue::Multiple(values) => {
        if values.len() > 0 {
          MarginComponent::from_value(values[0].clone())
        } else {
          MarginComponent::empty()
        }
      },
      _ => MarginComponent::empty(),
    }
  }
}