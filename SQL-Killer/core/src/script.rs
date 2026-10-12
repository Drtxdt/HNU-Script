use crate::model::{source_range, SourceRange};

#[derive(Debug, Clone)]
pub struct SqlStatement {
    pub sql: String,
    pub range: SourceRange,
}

/// Split client scripts without treating quoted/commented delimiters as boundaries.
/// BEGIN/END nesting keeps SQLite triggers and MySQL stored programs intact.
pub fn split_script(source: &str) -> Vec<SqlStatement> {
    let b = source.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut start) = (0, 0);
    let mut quote = 0u8;
    let (mut line_comment, mut block_comment) = (false, false);
    let mut delimiter = ";".to_string();
    let mut depth = 0usize;
    let mut routine = false;
    let mut words = Vec::<String>::new();
    let mut after_end = false;
    while i < b.len() {
        if line_comment {
            if b[i] == b'\n' {
                line_comment = false;
            }
            i += 1;
            continue;
        }
        if block_comment {
            if b[i..].starts_with(b"*/") {
                block_comment = false;
                i += 2
            } else {
                i += 1
            }
            continue;
        }
        if quote != 0 {
            if b[i] == quote {
                if i + 1 < b.len() && b[i + 1] == quote {
                    i += 2;
                    continue;
                }
                quote = 0;
            } else if b[i] == b'\\' && quote != b']' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"--") || b[i] == b'#' {
            line_comment = true;
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"/*") {
            block_comment = true;
            i += 2;
            continue;
        }
        if matches!(b[i], b'\'' | b'"' | b'`' | b'[') {
            quote = if b[i] == b'[' { b']' } else { b[i] };
            i += 1;
            continue;
        }
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let begin = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let word = source[begin..i].to_ascii_uppercase();
            if word == "DELIMITER" && source[start..begin].trim().is_empty() {
                let end = source[i..].find('\n').map(|p| i + p).unwrap_or(b.len());
                let d = source[i..end].trim();
                if !d.is_empty() {
                    delimiter = d.into();
                }
                i = end;
                start = end;
                continue;
            }
            words.push(word.clone());
            if words.first().is_some_and(|s| s == "CREATE")
                && matches!(
                    word.as_str(),
                    "TRIGGER" | "PROCEDURE" | "FUNCTION" | "EVENT"
                )
            {
                routine = true;
            }
            if routine {
                if word == "END" {
                    depth = depth.saturating_sub(1);
                    after_end = true;
                } else if matches!(
                    word.as_str(),
                    "BEGIN" | "CASE" | "IF" | "LOOP" | "WHILE" | "REPEAT"
                ) {
                    if !after_end {
                        depth += 1;
                    }
                    after_end = false;
                } else {
                    after_end = false;
                }
            }
            continue;
        }
        if source.is_char_boundary(i)
            && source[i..].starts_with(&delimiter)
            && (delimiter != ";" || depth == 0)
        {
            push(&mut out, source, start, i);
            i += delimiter.len();
            start = i;
            routine = false;
            words.clear();
            depth = 0;
            continue;
        }
        i += 1;
    }
    push(&mut out, source, start, source.len());
    out
}
fn push(out: &mut Vec<SqlStatement>, s: &str, start: usize, end: usize) {
    let raw = &s[start..end];
    let trim = raw.trim();
    if trim.is_empty() {
        return;
    }
    let at = start + raw.len() - raw.trim_start().len();
    if tokenize(trim).is_empty() {
        return;
    }
    out.push(SqlStatement {
        sql: trim.to_string(),
        range: source_range(s, at, at + trim.len()),
    });
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub depth: usize,
    pub quoted: bool,
}
pub fn tokenize(s: &str) -> Vec<Token> {
    let b = s.as_bytes();
    let (mut i, mut depth) = (0usize, 0usize);
    let mut out = Vec::new();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if b[i..].starts_with(b"--") || b[i] == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1
            }
            continue;
        }
        if b[i..].starts_with(b"/*") {
            i += 2;
            while i < b.len() && !b[i..].starts_with(b"*/") {
                i += 1
            }
            i = (i + 2).min(b.len());
            continue;
        }
        let start = i;
        let quoted = matches!(b[i], b'\'' | b'"' | b'`' | b'[');
        if quoted {
            let end = if b[i] == b'[' { b']' } else { b[i] };
            i += 1;
            while i < b.len() {
                if b[i] == end {
                    i += 1;
                    if i < b.len() && b[i] == end {
                        i += 1;
                        continue;
                    }
                    break;
                }
                if b[i] == b'\\' {
                    i += 1
                }
                i += 1;
            }
            i = i.min(b.len());
        } else if b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 128 {
            i += 1;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] >= 128 || b[i] == b'$')
            {
                i += 1
            }
        } else {
            i += 1;
            if i < b.len()
                && matches!(&s[start..i], "<" | ">" | "!" | "|")
                && matches!(b[i], b'=' | b'>' | b'|')
            {
                i += 1;
            }
        }
        let text = s[start..i].to_string();
        if text == ")" {
            depth = depth.saturating_sub(1);
        }
        out.push(Token {
            text: text.clone(),
            start,
            end: i,
            depth,
            quoted,
        });
        if text == "(" {
            depth += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_delimiters() {
        let s = "-- hi;\nSELECT 'a;中😀'; SELECT 2 /* ; */;";
        let v = split_script(s);
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].sql, "SELECT 2 /* ; */");
    }
    #[test]
    fn routine() {
        assert_eq!(split_script("CREATE TRIGGER t AFTER INSERT ON a BEGIN INSERT INTO b VALUES(1); UPDATE b SET x=2; END; SELECT 1;").len(),2);
        assert_eq!(split_script("DELIMITER $$\nCREATE PROCEDURE p() BEGIN IF 1 THEN SELECT ';'; END IF; END$$\nDELIMITER ;\nCALL p();").len(),2);
    }
}
