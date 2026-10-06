use std::io::BufRead;

pub fn read_http_response<R: BufRead>(reader: &mut R) -> Result<String, std::io::Error> {
    let mut content_length: Option<usize> = None;
    
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "EOF reading HTTP header"));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if trimmed.to_ascii_lowercase().starts_with("content-length:") {
            if let Some(val_str) = trimmed.split(':').nth(1) {
                if let Ok(len) = val_str.trim().parse::<usize>() {
                    content_length = Some(len);
                }
            }
        }
    }
    
    let body_len = content_length.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Missing Content-Length in HTTP response")
    })?;
    
    let mut body = vec![0u8; body_len];
    reader.read_exact(&mut body)?;
    Ok(String::from_utf8_lossy(&body).to_string())
}

pub fn parse_session_control_exit(body: &str) -> (bool, String) {
    let should_exit = body.contains("\"should_exit\":true") || (body.contains("\"should_exit\"") && body.contains("true"));
    let status = if let Some(idx) = body.find("\"status\":") {
        let rem = &body[idx + 9..];
        let trimmed = rem.trim_start();
        if trimmed.starts_with('"') {
            let inner = &trimmed[1..];
            if let Some(end_quote) = inner.find('"') {
                inner[..end_quote].to_string()
            } else {
                "Active".to_string()
            }
        } else {
            "Active".to_string()
        }
    } else {
        "Active".to_string()
    };
    (should_exit, status)
}
