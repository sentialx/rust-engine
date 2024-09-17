#[derive(Clone, Debug, PartialEq)]
pub enum CssTokenType {
    String,
    Number,
    Operator,
    ParenthesisStart,
    ParenthesisEnd,
    FunctionStart,
    ArgumentSeparator,
    MultiValueSeparator,
    Size,
    End,
}
#[derive(Clone, Debug)]
pub struct CssToken {
    pub values: Vec<String>,
    pub type_: CssTokenType,
}

#[derive(Clone, Debug)]
pub enum CssSizeUnit {
    Px,
    Em,
    Percent,
}

#[derive(Clone, Debug)]
pub struct CssSize {
    pub value: f32,
    pub unit: CssSizeUnit,
}

// todo eval css size with units as hardcoded values

#[derive(Clone, Debug)]
pub struct CssFunction {
    pub name: String,
    pub args: Vec<CssValue>,
}

#[derive(Clone, Debug)]
pub enum CssOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Clone, Debug)]
pub struct CssBinaryExpression {
    left: Option<Box<CssValue>>,
    operator: CssOperator,
    right: Option<Box<CssValue>>,
}

#[derive(Clone, Debug)]
pub enum CssValue {
    Size(CssSize),
    String(String),
    Number(f32),
    Function(CssFunction),
    Binary(CssBinaryExpression),
    Multiple(Vec<CssValue>),
    Invalid,
}

impl CssValue {
    pub fn is_inherit(&self) -> bool {
        match self {
            CssValue::String(s) => s == "inherit",
            _ => false,
        }
    }

    pub fn inherit() -> CssValue {
        CssValue::String("inherit".to_string())
    }
}

pub fn char_to_operator(c: char) -> Option<CssOperator> {
    match c {
        '+' => Some(CssOperator::Add),
        '-' => Some(CssOperator::Subtract),
        '*' => Some(CssOperator::Multiply),
        '/' => Some(CssOperator::Divide),
        _ => None,
    }
}

pub fn str_to_unit(s: &str) -> Option<CssSizeUnit> {
    match s {
        "px" => Some(CssSizeUnit::Px),
        "em" => Some(CssSizeUnit::Em),
        "%" => Some(CssSizeUnit::Percent),
        _ => None,
    }
}

pub fn tokenize_css_value(value: &str) -> Vec<CssToken> {
    let mut tokens: Vec<CssToken> = vec![];

    let mut curr_token = CssToken {
        values: vec!["".to_string()],
        type_: CssTokenType::End,
    };

    for c in value.chars() {
        let operator = char_to_operator(c);

        let last_type = curr_token.type_.clone();

        let token_type = if (c.is_numeric()
            || (c == '.'
                && curr_token.type_ == CssTokenType::Number))
            && (curr_token.type_ != CssTokenType::String)
        {
            CssTokenType::Number
        } else if c == '(' {
            if curr_token.type_ == CssTokenType::String {
                curr_token.type_ = CssTokenType::FunctionStart;
                CssTokenType::FunctionStart
            } else {
                CssTokenType::ParenthesisStart
            }
        } else if c == ')' {
            CssTokenType::ParenthesisEnd
        } else if operator.is_some() && curr_token.type_ != CssTokenType::String {
            if curr_token.type_ == CssTokenType::MultiValueSeparator {
                curr_token.type_ = CssTokenType::Operator;
                curr_token.values = vec!["".to_string()];
            }
            CssTokenType::Operator
        } else if c == ',' {
            CssTokenType::ArgumentSeparator
        } else if c == ' ' {
            if curr_token.type_ != CssTokenType::MultiValueSeparator
                && curr_token.type_ != CssTokenType::ArgumentSeparator
                && curr_token.type_ != CssTokenType::Operator
            {
                CssTokenType::MultiValueSeparator
            } else {
                continue;
            }
        } else if curr_token.type_ == CssTokenType::Size {
            CssTokenType::Size
        } else {
            CssTokenType::String
        };

        let is_unit = last_type == CssTokenType::Number && token_type == CssTokenType::String;

        if curr_token.values.len() == 1 && is_unit {
            curr_token.values.push("".to_string());
            curr_token.type_ = CssTokenType::Size;
        }

        // ( and ) are always separate tokens
        if (curr_token.type_ != token_type
            || (c == '(' && curr_token.type_ != CssTokenType::FunctionStart)
            || c == ')')
            && !is_unit
        {
            if curr_token.values.len() > 0 && curr_token.values[0] != "" {
                tokens.push(curr_token);
            }

            curr_token = CssToken {
                values: vec![c.to_string()],
                type_: token_type,
            };
        } else {
            curr_token.values.last_mut().unwrap().push(c);
        }
    }

    tokens.push(curr_token);
    tokens.push(CssToken {
        values: vec!["".to_string()],
        type_: CssTokenType::End,
    });

    tokens
}

fn add_to_current_context_stack(context_stack: &mut Vec<CssValue>, value: CssValue) -> Result<(), ()> {
    let last_ctx = context_stack.last_mut();
    if last_ctx.is_none() {
        return Err(());
    }
    let last_ctx = last_ctx.unwrap();

    match last_ctx {
        CssValue::Multiple(values) => {
            values.push(value);
        }
        CssValue::Binary(expr) => {
            if expr.left.is_none() {
                expr.left = Some(Box::new(value));
            } else if expr.right.is_none() {
                expr.right = Some(Box::new(value));
            }
        }
        CssValue::Function(func) => {
            func.args.push(value);
        }
        _ => {}
    }

    Ok(())
}

fn exit_expr_context_stack(context_stack: &mut Vec<CssValue>) -> Result<(), ()> {
    // exit from binary expressions
    loop {
        match context_stack.last().unwrap() {
            CssValue::Binary(expr) => {
                let ctx = context_stack.pop().unwrap();
                let res = add_to_current_context_stack(context_stack, ctx);
                if res.is_err() {
                    return res;
                }
            }
            _ => {
                break;
            }
        }
    }

    Ok(())
}

pub fn parse_css_value(tokens: Vec<CssToken>) -> CssValue {
    let mut context_stack: Vec<CssValue> = vec![CssValue::Multiple(vec![])];

    let mut pending_values: Vec<CssValue> = vec![];

    for (i, token) in tokens.iter().enumerate() {
        let is_end_of_single_value = token.type_ == CssTokenType::ArgumentSeparator
            || token.type_ == CssTokenType::End
            || token.type_ == CssTokenType::ParenthesisEnd
            || token.type_ == CssTokenType::MultiValueSeparator;

        match token {
            CssToken {
                values,
                type_: CssTokenType::Number,
            } => {
                let value = CssValue::Number(values[0].parse().unwrap());
                pending_values.push(value);
            }
            CssToken {
                values,
                type_: CssTokenType::String,
            } => {
                let value = CssValue::String(values[0].clone());
                pending_values.push(value);
            }
            CssToken {
                values,
                type_: CssTokenType::Size,
            } => {
                let value = CssValue::Size(CssSize {
                    value: values[0].parse().unwrap(),
                    unit: str_to_unit(&values[1]).unwrap_or(CssSizeUnit::Px),
                });
                pending_values.push(value);
            }
            CssToken {
                values,
                type_: CssTokenType::FunctionStart,
            } => {
                let function = CssFunction {
                    name: values[0].clone(),
                    args: vec![],
                };

                for value in &pending_values {
                    if add_to_current_context_stack(&mut context_stack, value.clone()).is_err() {
                        return CssValue::Invalid;
                    }
                }
                pending_values = vec![];
                context_stack.push(CssValue::Function(function));
            }
            CssToken {
                values,
                type_: CssTokenType::ParenthesisEnd,
            } => {
                for value in &pending_values {
                    if add_to_current_context_stack(&mut context_stack, value.clone()).is_err() {
                        return CssValue::Invalid;
                    }
                }

                if exit_expr_context_stack(&mut context_stack).is_err() {
                    return CssValue::Invalid;
                }

                let context = context_stack.pop();
                if add_to_current_context_stack(&mut context_stack, context.clone().unwrap()).is_err() {
                    return CssValue::Invalid;
                }

                match context.as_ref().unwrap() {
                    CssValue::Multiple(_) => {
                        pending_values = vec![context.clone().unwrap()];
                    }
                    _ => {
                        pending_values = vec![];
                    }
                }
            }
            CssToken {
                values,
                type_: CssTokenType::ParenthesisStart,
            } => {
                let multiple = CssValue::Multiple(vec![]);

                for value in &pending_values {
                    if add_to_current_context_stack(&mut context_stack, value.clone()).is_err() {
                        return CssValue::Invalid;
                    }
                }

                pending_values = vec![];
                context_stack.push(multiple);
            }
            CssToken {
                values,
                type_: CssTokenType::MultiValueSeparator,
            } => {
                match context_stack.last_mut().unwrap() {
                    CssValue::Function(func) => {
                        func.args.push(CssValue::Multiple(vec![]));
                    }
                    _ => {}
                }

                for value in &pending_values {
                    if add_to_current_context_stack(&mut context_stack, value.clone()).is_err() {
                        return CssValue::Invalid;
                    }
                }
                pending_values = vec![];
            }
            CssToken {
                values,
                type_: CssTokenType::ArgumentSeparator,
            } => {
                for value in &pending_values {
                    if add_to_current_context_stack(&mut context_stack, value.clone()).is_err() {
                        return CssValue::Invalid;
                    }
                }
                pending_values = vec![];
            }
            CssToken {
                values,
                type_: CssTokenType::End,
            } => {
                for value in &pending_values {
                    if add_to_current_context_stack(&mut context_stack, value.clone()).is_err() {
                        return CssValue::Invalid;
                    }
                }
                pending_values = vec![];
            }
            CssToken {
                values,
                type_: CssTokenType::Operator,
            } => {
                let operator = char_to_operator(values[0].chars().next().unwrap());
                let left = pending_values.pop();

                if left.is_none() {
                    continue;
                }

                let left = left.unwrap();

                // todo check if there already is a binary expression
                // match context_stack.last_mut().unwrap() {
                //     CssValues::Binary(expr) => {
                //         expr.right = Some(Box::new(left));
                //     },
                //     _ => {}
                // }
                let binary = CssValue::Binary(CssBinaryExpression {
                    left: Some(Box::new(left)),
                    operator: operator.unwrap(),
                    right: None,
                });
                context_stack.push(binary);
                // pending_values = vec![];
            }
            _ => {}
        }
    }

    context_stack[0].clone()
}
