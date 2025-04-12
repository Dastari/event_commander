use quick_xml::{Reader, Writer, events::Event as XmlEvent};
use std::io::Cursor;

/// Sanitizes a filename by retaining only alphanumeric characters, dashes, underscores, and dots.
pub fn sanitize_filename(filename: &str) -> String {
    filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect()
}

/// Formats an XML string with indentation and returns the formatted XML or an error message.
pub fn pretty_print_xml(xml_str: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml_str);
    reader.trim_text(true);
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(XmlEvent::Start(e)) => writer
                .write_event(XmlEvent::Start(e))
                .map_err(|e| format!("XML Write Error (Start): {}", e))?,
            Ok(XmlEvent::End(e)) => writer
                .write_event(XmlEvent::End(e))
                .map_err(|e| format!("XML Write Error (End): {}", e))?,
            Ok(XmlEvent::Empty(e)) => writer
                .write_event(XmlEvent::Empty(e))
                .map_err(|e| format!("XML Write Error (Empty): {}", e))?,
            Ok(XmlEvent::Text(e)) => writer
                .write_event(XmlEvent::Text(e))
                .map_err(|e| format!("XML Write Error (Text): {}", e))?,
            Ok(XmlEvent::Comment(e)) => writer
                .write_event(XmlEvent::Comment(e))
                .map_err(|e| format!("XML Write Error (Comment): {}", e))?,
            Ok(XmlEvent::CData(e)) => writer
                .write_event(XmlEvent::CData(e))
                .map_err(|e| format!("XML Write Error (CData): {}", e))?,
            Ok(XmlEvent::Decl(e)) => writer
                .write_event(XmlEvent::Decl(e))
                .map_err(|e| format!("XML Write Error (Decl): {}", e))?,
            Ok(XmlEvent::PI(e)) => writer
                .write_event(XmlEvent::PI(e))
                .map_err(|e| format!("XML Write Error (PI): {}", e))?,
            Ok(XmlEvent::DocType(e)) => writer
                .write_event(XmlEvent::DocType(e))
                .map_err(|e| format!("XML Write Error (DocType): {}", e))?,
            Ok(XmlEvent::Eof) => break,
            Err(e) => return Err(format!("XML Read Error: {}", e)),
        }
        buf.clear();
    }

    let mut bytes = writer.into_inner().into_inner();
    // Trim trailing whitespace bytes (space, tab, newline, cr)
    while let Some(&last_byte) = bytes.last() {
        if last_byte == b' ' || last_byte == b'\t' || last_byte == b'\n' || last_byte == b'\r' {
            bytes.pop();
        } else {
            break;
        }
    }
    // Ensure a single trailing newline
    bytes.push(b'\n');
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 Conversion Error: {}", e))
}

/// Computes a centered fixed-size rectangle within a given rectangle.
pub fn centered_fixed_rect(
    width: u16,
    height: u16,
    r: ratatui::prelude::Rect,
) -> ratatui::prelude::Rect {
    use ratatui::prelude::Rect;
    let x = r.x + r.width.saturating_sub(width) / 2;
    let y = r.y + r.height.saturating_sub(height) / 2;
    Rect::new(x, y, width.min(r.width), height.min(r.height))
}
