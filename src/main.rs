enum NodeType {
    Element,
    Attribute,
    Text,
    Comment,
    DocumentType,
}

pub struct DomElement {
    children: Vec<DomElement>,
    parentNode: Box<DomElement>,
    nodeValue: String,
    nodeType: NodeType,
    innerHTML: String,
    outerHTML: String,
    tagName: String,
}

fn tokenize(html: String) -> Vec<String> {
    let mut tokens: Vec<String> = vec![];

    let mut capturing = false;
    let mut capturedText = String::from("");

    for i in 0..html.len() {
        let c = html.chars().nth(i).unwrap();

        if c == '<' {
            if capturing {
                tokens.push(capturedText.to_string());
            } else {
                capturing = true;
            }
            capturedText = String::from("");
        } else if c == '>' || i == html.len() - 1 {
            capturing = false;
            capturedText.push(c);
        } else if !capturing {
            capturedText = String::from("");
        }

        if capturing {
            capturedText.push(c);
        }
    }

    return tokens;
}

fn main() {
    println!("Hello, world!");
}
