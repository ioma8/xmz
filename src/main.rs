use memmap2::Mmap;
use std::{fs::File, str};

#[derive(Debug)]
enum Token<'a> {
    StartTag(&'a str),
    EndTag(&'a str),
    Text(&'a str),
}

/// Streams tokens from XML without allocations.
/// Calls `on_token` for each parsed token.
fn stream_xml<'a, F>(xml: &'a str, mut on_token: F)
where
    F: FnMut(Token<'a>),
{
    let bytes = xml.as_bytes();
    let mut pos = 0;

    while pos < bytes.len() {
        // Skip whitespace between tokens
        while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= bytes.len() {
            break;
        }

        if bytes[pos] == b'<' {
            // End tag
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                let start = pos + 2;
                if let Some(end) = bytes[start..].iter().position(|&b| b == b'>') {
                    let name = &xml[start..start + end];
                    on_token(Token::EndTag(name));
                    pos = start + end + 1;
                } else {
                    break;
                }
            }
            // Start tag
            else {
                let start = pos + 1;
                if let Some(end) = bytes[start..].iter().position(|&b| b == b'>') {
                    let name = &xml[start..start + end];
                    on_token(Token::StartTag(name));
                    pos = start + end + 1;
                } else {
                    break;
                }
            }
        } else {
            // Text node
            let start = pos;
            let end = bytes[start..]
                .iter()
                .position(|&b| b == b'<')
                .unwrap_or(bytes.len() - start);
            let text = &xml[start..start + end];
            if !text.trim().is_empty() {
                on_token(Token::Text(text.trim()));
            }
            pos = start + end;
        }
    }
}

fn main() -> std::io::Result<()> {
    let file = File::open("psd7003.xml")?;
    let mmap = unsafe { Mmap::map(&file)? };
    let xml = std::str::from_utf8(&mmap).expect("Invalid UTF-8 XML");

    let mut depth = 0;
    stream_xml(xml, |token| {
        let spacing = " ".repeat(depth * 4);
        match token {
            Token::StartTag(name) => {
                println!("{}<{}>", spacing, name);
                depth += 1;
            }
            Token::EndTag(name) => {
                println!("{}</{}>", spacing, name);
                depth -= 1;
            }
            Token::Text(text) => {
                println!("{}{}", spacing, text);
            }
        }
    });

    Ok(())
}
