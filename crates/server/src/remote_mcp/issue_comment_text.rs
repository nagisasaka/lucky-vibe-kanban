//! Bounded UTF-8 decoding shared by comment reads. Kept dependency-free so its
//! Unicode/offset contract can be checked without building the server.
pub(super) fn preview(bytes: &[u8]) -> Result<String, &'static str> {
    let valid_length = match std::str::from_utf8(bytes) {
        Ok(text) => text.len(),
        Err(error) if error.error_len().is_none() => error.valid_up_to(),
        Err(_) => return Err("Comment body is not valid UTF-8"),
    };
    Ok(std::str::from_utf8(&bytes[..valid_length])
        .map_err(|_| "Comment body is not valid UTF-8")?
        .chars()
        .take(256)
        .collect())
}

pub(super) struct TextPage {
    pending: Vec<u8>,
    total: u64,
    page: String,
    offset: u64,
    end: u64,
}
impl TextPage {
    pub(super) fn new(offset: u32, limit: u32) -> Self {
        Self {
            pending: Vec::new(),
            total: 0,
            page: String::new(),
            offset: offset.into(),
            end: u64::from(offset) + u64::from(limit),
        }
    }
    pub(super) fn push(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        self.pending.extend_from_slice(bytes);
        let valid_length = match std::str::from_utf8(&self.pending) {
            Ok(text) => text.len(),
            Err(error) if error.error_len().is_none() => error.valid_up_to(),
            Err(_) => return Err("Comment body is not valid UTF-8"),
        };
        for ch in std::str::from_utf8(&self.pending[..valid_length])
            .map_err(|_| "Comment body is not valid UTF-8")?
            .chars()
        {
            if self.total >= self.offset && self.total < self.end {
                self.page.push(ch);
            }
            self.total += 1;
        }
        self.pending.drain(..valid_length);
        Ok(())
    }
    pub(super) fn finish(self) -> Result<(String, u64), &'static str> {
        if !self.pending.is_empty() {
            return Err("Comment body is not valid UTF-8");
        }
        if self.offset > self.total {
            return Err("Comment offset is outside the body");
        }
        Ok((self.page, self.total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_utf8_chunk_boundaries_reconstruct_character_pages() {
        let body = "界😀e\u{301}\0\u{1}\\\"\n".repeat(2000);
        let count = body.chars().count();
        for chunk_size in [1, 3, 7, 16384] {
            let mut restored = String::new();
            for offset in (0..count).step_by(997) {
                let mut page = TextPage::new(offset as u32, 997);
                for chunk in body.as_bytes().chunks(chunk_size) {
                    page.push(chunk).unwrap();
                }
                let (text, total) = page.finish().unwrap();
                assert_eq!(total, count as u64);
                assert!(text.chars().count() <= 997);
                restored.push_str(&text);
            }
            assert_eq!(restored, body);
        }
    }
    #[test]
    fn terminal_empty_outside_and_invalid_utf8() {
        assert_eq!(
            TextPage::new(0, 16000).finish().unwrap(),
            (String::new(), 0)
        );
        for (offset, expected) in [(0, "😀"), (1, "e"), (2, "\u{301}"), (3, "")] {
            let mut page = TextPage::new(offset, 1);
            page.push("😀e\u{301}".as_bytes()).unwrap();
            assert_eq!(page.finish().unwrap(), (expected.into(), 3));
        }
        assert!(TextPage::new(1, 1).finish().is_err());
        assert!(TextPage::new(0, 1).push(&[0xff]).is_err());
        let mut incomplete = TextPage::new(0, 1);
        incomplete.push(&[0xf0]).unwrap();
        assert!(incomplete.finish().is_err());
    }
    #[test]
    fn previews_are_bounded_and_preserve_embedded_nul() {
        let body = "界😀\0e\u{301}".repeat(1000);
        for size in [0, 1, 2, 3, 1024] {
            let bytes = &body.as_bytes()[..size];
            let text = preview(bytes).unwrap();
            assert!(body.starts_with(&text));
            assert!(text.chars().count() <= 256);
        }
        assert_eq!(
            preview(&body.as_bytes()[..1024]).unwrap(),
            body.chars().take(256).collect::<String>()
        );
        assert!(preview(&[0xff]).is_err());
    }
}
