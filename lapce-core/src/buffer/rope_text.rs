use std::{borrow::Cow, ops::Range};

use lapce_xi_rope::{interval::IntervalBounds, Cursor, Rope};
use lsp_types::Position;

use crate::{
    encoding::{offset_utf16_to_utf8, offset_utf8_to_utf16},
    word::WordCursor,
};

/// A wrapper around a rope that provides utility functions atop it.
pub struct RopeText<'a> {
    text: &'a Rope,
}

impl<'a> RopeText<'a> {
    pub fn new(text: &'a Rope) -> Self {
        Self { text }
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The last line of the held rope
    pub fn last_line(&self) -> usize {
        self.line_of_offset(self.len())
    }

    /// Get the offset into the rope of the start of the given line.  
    /// If the line it out of bounds, then the last offset (the len) is returned.
    pub fn offset_of_line(&self, line: usize) -> usize {
        let last_line = self.last_line();
        let line = line.min(last_line + 1);
        self.text.offset_of_line(line)
    }

    pub fn offset_line_end(&self, offset: usize, caret: bool) -> usize {
        let line = self.line_of_offset(offset);
        self.line_end_offset(line, caret)
    }

    pub fn line_of_offset(&self, offset: usize) -> usize {
        let offset = offset.min(self.len());
        let offset = self
            .text
            .at_or_prev_codepoint_boundary(offset)
            .unwrap_or(offset);

        self.text.line_of_offset(offset)
    }

    /// Converts a UTF8 offset to a UTF16 LSP position  
    /// Returns None if it is not a valid UTF16 offset
    pub fn offset_to_position(&self, offset: usize) -> Position {
        let (line, col) = self.offset_to_line_col(offset);
        let line_offset = self.offset_of_line(line);

        let utf16_col =
            offset_utf8_to_utf16(self.char_indices_iter(line_offset..), col);

        Position {
            line: line as u32,
            character: utf16_col as u32,
        }
    }

    pub fn offset_of_position(&self, pos: &Position) -> usize {
        let (line, column) = self.position_to_line_col(pos);

        self.offset_of_line_col(line, column)
    }

    pub fn position_to_line_col(&self, pos: &Position) -> (usize, usize) {
        let line = pos.line as usize;
        let line_offset = self.offset_of_line(line);

        let column = offset_utf16_to_utf8(
            self.char_indices_iter(line_offset..),
            pos.character as usize,
        );

        (line, column)
    }

    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.len());
        let line = self.line_of_offset(offset);
        let line_start = self.offset_of_line(line);
        if offset == line_start {
            return (line, 0);
        }

        let col = offset - line_start;
        (line, col)
    }

    pub fn offset_of_line_col(&self, line: usize, col: usize) -> usize {
        let mut pos = 0;
        let mut offset = self.offset_of_line(line);
        for c in self
            .slice_to_cow(offset..self.offset_of_line(line + 1))
            .chars()
        {
            if c == '\n' {
                return offset;
            }

            let char_len = c.len_utf8();
            if pos + char_len > col {
                return offset;
            }
            pos += char_len;
            offset += char_len;
        }
        offset
    }

    pub fn line_end_col(&self, line: usize, caret: bool) -> usize {
        let line_start = self.offset_of_line(line);
        let offset = self.line_end_offset(line, caret);
        offset - line_start
    }

    /// Get the offset of the end of the line. The caret decides whether it is after the last
    /// character, or before it.  
    /// If the line is out of bounds, then the last offset (the len) is returned.
    /// ```rust,ignore
    /// let text = Rope::from("hello\nworld");
    /// let text = RopeText::new(&text);
    /// assert_eq!(text.line_end_offset(0, false), 4);  // "hell|o"
    /// assert_eq!(text.line_end_offset(0, true), 5);   // "hello|"
    /// assert_eq!(text.line_end_offset(1, false), 10); // "worl|d"
    /// assert_eq!(text.line_end_offset(1, true), 11);  // "world|"
    /// // Out of bounds
    /// assert_eq!(text.line_end_offset(2, false), 11); // "world|"
    /// ```
    pub fn line_end_offset(&self, line: usize, caret: bool) -> usize {
        let mut offset = self.offset_of_line(line + 1);
        let mut line_content: &str = &self.line_content(line);
        if line_content.ends_with("\r\n") {
            offset -= 2;
            line_content = &line_content[..line_content.len() - 2];
        } else if line_content.ends_with('\n') {
            offset -= 1;
            line_content = &line_content[..line_content.len() - 1];
        }
        if !caret && !line_content.is_empty() {
            offset = self.prev_grapheme_offset(offset, 1, 0);
        }
        offset
    }

    /// Returns the content of the given line.
    /// Includes the line ending if it exists. (-> the last line won't have a line ending)    
    /// Lines past the end of the document will return an empty string.
    pub fn line_content(&self, line: usize) -> Cow<'a, str> {
        self.text
            .slice_to_cow(self.offset_of_line(line)..self.offset_of_line(line + 1))
    }

    /// Get the offset of the previous grapheme cluster.
    pub fn prev_grapheme_offset(
        &self,
        offset: usize,
        count: usize,
        limit: usize,
    ) -> usize {
        let offset = offset.min(self.len());
        let mut cursor = Cursor::new(self.text, offset);
        let mut new_offset = offset;
        for _i in 0..count {
            if let Some(prev_offset) = cursor.prev_grapheme() {
                if prev_offset < limit {
                    return new_offset;
                }
                new_offset = prev_offset;
                cursor.set(prev_offset);
            } else {
                return new_offset;
            }
        }
        new_offset
    }

    /// Returns the offset of the first non-blank character on the given line.  
    /// If the line is one past the last line, then the offset at the end of the rope is returned.
    /// If the line is further past that, then it defaults to the last line.
    pub fn first_non_blank_character_on_line(&self, line: usize) -> usize {
        let last_line = self.last_line();
        let line = if line > last_line + 1 {
            last_line
        } else {
            line
        };
        let line_start_offset = self.text.offset_of_line(line);
        WordCursor::new(self.text, line_start_offset).next_non_blank_char()
    }

    pub fn indent_on_line(&self, line: usize) -> String {
        let line_start_offset = self.text.offset_of_line(line);
        let word_boundary =
            WordCursor::new(self.text, line_start_offset).next_non_blank_char();
        let indent = self.text.slice_to_cow(line_start_offset..word_boundary);
        indent.to_string()
    }

    /// Get the content of the rope as a Cow string, for 'nice' ranges (small, and at the right
    /// offsets) this will be a reference to the rope's data. Otherwise, it allocates a new string.
    /// You should be somewhat wary of requesting large parts of the rope, as it will allocate
    /// a new string since it isn't contiguous in memory for large chunks.
    pub fn slice_to_cow(&self, range: Range<usize>) -> Cow<'a, str> {
        self.text
            .slice_to_cow(range.start.min(self.len())..range.end.min(self.len()))
    }

    /// Iterate over (utf8_offset, char) values in the given range  
    /// This uses `iter_chunks` and so does not allocate, compared to `slice_to_cow` which can
    pub fn char_indices_iter<T: IntervalBounds>(
        &self,
        range: T,
    ) -> impl Iterator<Item = (usize, char)> + 'a {
        CharIndicesJoin::new(self.text.iter_chunks(range).map(str::char_indices))
    }

    /// The number of lines in the file
    pub fn num_lines(&self) -> usize {
        self.last_line() + 1
    }

    /// The length of the given line
    pub fn line_len(&self, line: usize) -> usize {
        self.offset_of_line(line + 1) - self.offset_of_line(line)
    }
}

/// Joins an iterator of iterators over char indices `(usize, char)` into one
/// as if they were from a single long string
/// Assumes the iterators end after the first `None` value
#[derive(Clone)]
pub struct CharIndicesJoin<I: Iterator<Item = (usize, char)>, O: Iterator<Item = I>>
{
    /// Our iterator of iterators
    main_iter: O,
    /// Our current working iterator of indices
    current_indices: Option<I>,
    /// The amount we should shift future offsets
    current_base: usize,
    /// The latest base, since we don't know when the `current_indices` iterator will end
    latest_base: usize,
}

impl<I: Iterator<Item = (usize, char)>, O: Iterator<Item = I>>
    CharIndicesJoin<I, O>
{
    pub fn new(main_iter: O) -> CharIndicesJoin<I, O> {
        CharIndicesJoin {
            main_iter,
            current_indices: None,
            current_base: 0,
            latest_base: 0,
        }
    }
}

impl<I: Iterator<Item = (usize, char)>, O: Iterator<Item = I>> Iterator
    for CharIndicesJoin<I, O>
{
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = &mut self.current_indices {
            if let Some((next_offset, next_ch)) = current.next() {
                // Shift by the current base offset, which is the accumulated offset from previous
                // iterators, which makes so the offset produced looks like it is from one long str
                let next_offset = self.current_base + next_offset;
                // Store the latest base offset, because we don't know when the current iterator
                // will end (though technically the str iterator impl does)
                self.latest_base = next_offset + next_ch.len_utf8();
                return Some((next_offset, next_ch));
            }
        }

        // Otherwise, if we didn't return something above, then we get a next iterator
        if let Some(next_current) = self.main_iter.next() {
            // Update our current working iterator
            self.current_indices = Some(next_current);
            // Update the current base offset with the previous iterators latest offset base
            // This is what we are shifting by
            self.current_base = self.latest_base;

            // Get the next item without new current iterator
            // As long as main_iter and the iterators it produces aren't infinite then this
            // recursion won't be infinite either
            // and even the non-recursion version would be infinite if those were infinite
            self.next()
        } else {
            // We didn't get anything from the main iter, so we're completely done.
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use lapce_xi_rope::Rope;

    use super::RopeText;

    #[test]
    fn test_line_content() {
        let text = Rope::from("");
        let text = RopeText::new(&text);

        assert_eq!(text.line_content(0), "");
        assert_eq!(text.line_content(1), "");
        assert_eq!(text.line_content(2), "");

        let text = Rope::from("abc\ndef\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.line_content(0), "abc\n");
        assert_eq!(text.line_content(1), "def\n");
        assert_eq!(text.line_content(2), "ghi");
        assert_eq!(text.line_content(3), "");
        assert_eq!(text.line_content(4), "");
        assert_eq!(text.line_content(5), "");

        let text = Rope::from("abc\r\ndef\r\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.line_content(0), "abc\r\n");
        assert_eq!(text.line_content(1), "def\r\n");
        assert_eq!(text.line_content(2), "ghi");
        assert_eq!(text.line_content(3), "");
        assert_eq!(text.line_content(4), "");
        assert_eq!(text.line_content(5), "");
    }

    #[test]
    fn test_offset_of_line() {
        let text = Rope::from("");
        let text = RopeText::new(&text);

        assert_eq!(text.offset_of_line(0), 0);
        assert_eq!(text.offset_of_line(1), 0);
        assert_eq!(text.offset_of_line(2), 0);

        let text = Rope::from("abc\ndef\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.offset_of_line(0), 0);
        assert_eq!(text.offset_of_line(1), 4);
        assert_eq!(text.offset_of_line(2), 8);
        assert_eq!(text.offset_of_line(3), text.len()); // 11
        assert_eq!(text.offset_of_line(4), text.len());
        assert_eq!(text.offset_of_line(5), text.len());

        let text = Rope::from("abc\r\ndef\r\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.offset_of_line(0), 0);
        assert_eq!(text.offset_of_line(1), 5);
        assert_eq!(text.offset_of_line(2), 10);
        assert_eq!(text.offset_of_line(3), text.len()); // 13
        assert_eq!(text.offset_of_line(4), text.len());
        assert_eq!(text.offset_of_line(5), text.len());
    }

    #[test]
    fn test_line_end_offset() {
        let text = Rope::from("");
        let text = RopeText::new(&text);

        assert_eq!(text.line_end_offset(0, false), 0);
        assert_eq!(text.line_end_offset(0, true), 0);
        assert_eq!(text.line_end_offset(1, false), 0);
        assert_eq!(text.line_end_offset(1, true), 0);
        assert_eq!(text.line_end_offset(2, false), 0);
        assert_eq!(text.line_end_offset(2, true), 0);

        let text = Rope::from("abc\ndef\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.line_end_offset(0, false), 2);
        assert_eq!(text.line_end_offset(0, true), 3);
        assert_eq!(text.line_end_offset(1, false), 6);
        assert_eq!(text.line_end_offset(1, true), 7);
        assert_eq!(text.line_end_offset(2, false), 10);
        assert_eq!(text.line_end_offset(2, true), text.len());
        assert_eq!(text.line_end_offset(3, false), text.len());
        assert_eq!(text.line_end_offset(3, true), text.len());
        assert_eq!(text.line_end_offset(4, false), text.len());
        assert_eq!(text.line_end_offset(4, true), text.len());

        // This is equivalent to the doc test for RopeText::line_end_offset
        // because you don't seem to be able to do a `use RopeText` in a doc test since it isn't
        // public..
        let text = Rope::from("hello\nworld");
        let text = RopeText::new(&text);
        assert_eq!(text.line_end_offset(0, false), 4); // "hell|o"
        assert_eq!(text.line_end_offset(0, true), 5); // "hello|"
        assert_eq!(text.line_end_offset(1, false), 10); // "worl|d"
        assert_eq!(text.line_end_offset(1, true), 11); // "world|"
                                                       // Out of bounds
        assert_eq!(text.line_end_offset(2, false), 11); // "world|"
    }

    #[test]
    fn test_prev_grapheme_offset() {
        let text = Rope::from("");
        let text = RopeText::new(&text);

        assert_eq!(text.prev_grapheme_offset(0, 0, 0), 0);
        assert_eq!(text.prev_grapheme_offset(0, 1, 0), 0);
        assert_eq!(text.prev_grapheme_offset(0, 1, 1), 0);

        let text = Rope::from("abc def ghi");
        let text = RopeText::new(&text);

        assert_eq!(text.prev_grapheme_offset(0, 0, 0), 0);
        assert_eq!(text.prev_grapheme_offset(0, 1, 0), 0);
        assert_eq!(text.prev_grapheme_offset(0, 1, 1), 0);
        assert_eq!(text.prev_grapheme_offset(2, 1, 0), 1);
        assert_eq!(text.prev_grapheme_offset(2, 1, 1), 1);
    }

    #[test]
    fn test_first_non_blank_character_on_line() {
        let text = Rope::from("");
        let text = RopeText::new(&text);

        assert_eq!(text.first_non_blank_character_on_line(0), 0);
        assert_eq!(text.first_non_blank_character_on_line(1), 0);
        assert_eq!(text.first_non_blank_character_on_line(2), 0);

        let text = Rope::from("abc\ndef\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.first_non_blank_character_on_line(0), 0);
        assert_eq!(text.first_non_blank_character_on_line(1), 4);
        assert_eq!(text.first_non_blank_character_on_line(2), 8);
        assert_eq!(text.first_non_blank_character_on_line(3), 11);
        assert_eq!(text.first_non_blank_character_on_line(4), 8);
        assert_eq!(text.first_non_blank_character_on_line(5), 8);

        let text = Rope::from("abc\r\ndef\r\nghi");
        let text = RopeText::new(&text);

        assert_eq!(text.first_non_blank_character_on_line(0), 0);
        assert_eq!(text.first_non_blank_character_on_line(1), 5);
        assert_eq!(text.first_non_blank_character_on_line(2), 10);
        assert_eq!(text.first_non_blank_character_on_line(3), 13);
        assert_eq!(text.first_non_blank_character_on_line(4), 10);
        assert_eq!(text.first_non_blank_character_on_line(5), 10);
    }
}
