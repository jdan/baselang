use crate::ast::{Span, SpannedError};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Int(i64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    PlusEq,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    For,
    From,
    To,
    End,
    If,
    Print,
    And,
    Or,
    While,
    Fn,
    Return,
    Break,
    Newline,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn comment_spans(input: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let bytes = input.as_bytes();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes[pos] == b'#' {
            let start = pos;
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            spans.push(Span { start, end: pos });
        } else {
            pos += 1;
        }
    }
    spans
}

pub fn lex(input: &str) -> Result<Vec<Token>, SpannedError> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut last_was_newline = true; // suppress leading newlines

    while pos < bytes.len() {
        let b = bytes[pos];

        // Skip spaces, tabs, carriage returns
        if b == b' ' || b == b'\t' || b == b'\r' {
            pos += 1;
            continue;
        }

        // Comments
        if b == b'#' {
            while pos < bytes.len() && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // Newlines
        if b == b'\n' {
            if !last_was_newline {
                tokens.push(Token {
                    kind: TokenKind::Newline,
                    span: Span {
                        start: pos,
                        end: pos + 1,
                    },
                });
                last_was_newline = true;
            }
            pos += 1;
            continue;
        }

        last_was_newline = false;

        // Numbers
        if b.is_ascii_digit() {
            let start = pos;
            while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                pos += 1;
            }
            let value: i64 = input[start..pos].parse().unwrap();
            tokens.push(Token {
                kind: TokenKind::Int(value),
                span: Span { start, end: pos },
            });
            continue;
        }

        // $ identifiers (view references)
        if b == b'$' {
            let start = pos;
            pos += 1;
            if pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
                while pos < bytes.len()
                    && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_')
                {
                    pos += 1;
                }
                let word = &input[start..pos];
                tokens.push(Token {
                    kind: TokenKind::Ident(word.to_string()),
                    span: Span { start, end: pos },
                });
                last_was_newline = false;
                continue;
            } else {
                return Err(SpannedError {
                    message: "expected identifier after '$'".to_string(),
                    span: Span {
                        start,
                        end: start + 1,
                    },
                });
            }
        }

        // Identifiers and keywords
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = pos;
            while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_') {
                pos += 1;
            }
            let word = &input[start..pos];
            let kind = match word {
                "for" => TokenKind::For,
                "from" => TokenKind::From,
                "to" => TokenKind::To,
                "end" => TokenKind::End,
                "if" => TokenKind::If,
                "print" => TokenKind::Print,
                "and" => TokenKind::And,
                "or" => TokenKind::Or,
                "while" => TokenKind::While,
                "fn" => TokenKind::Fn,
                "return" => TokenKind::Return,
                "break" => TokenKind::Break,
                _ => TokenKind::Ident(word.to_string()),
            };
            tokens.push(Token {
                kind,
                span: Span { start, end: pos },
            });
            continue;
        }

        // Operators and punctuation
        let start = pos;
        match b {
            b'+' => {
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::PlusEq,
                        span: Span {
                            start,
                            end: pos + 2,
                        },
                    });
                    pos += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Plus,
                        span: Span {
                            start,
                            end: pos + 1,
                        },
                    });
                    pos += 1;
                }
            }
            b'-' => {
                tokens.push(Token {
                    kind: TokenKind::Minus,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b'*' => {
                tokens.push(Token {
                    kind: TokenKind::Star,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b'/' => {
                tokens.push(Token {
                    kind: TokenKind::Slash,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b'%' => {
                tokens.push(Token {
                    kind: TokenKind::Percent,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b'=' => {
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::EqEq,
                        span: Span {
                            start,
                            end: pos + 2,
                        },
                    });
                    pos += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Eq,
                        span: Span {
                            start,
                            end: pos + 1,
                        },
                    });
                    pos += 1;
                }
            }
            b'!' => {
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::BangEq,
                        span: Span {
                            start,
                            end: pos + 2,
                        },
                    });
                    pos += 2;
                } else {
                    return Err(SpannedError {
                        message: "unexpected character: '!'".to_string(),
                        span: Span {
                            start,
                            end: pos + 1,
                        },
                    });
                }
            }
            b'<' => {
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::LtEq,
                        span: Span {
                            start,
                            end: pos + 2,
                        },
                    });
                    pos += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Lt,
                        span: Span {
                            start,
                            end: pos + 1,
                        },
                    });
                    pos += 1;
                }
            }
            b'>' => {
                if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::GtEq,
                        span: Span {
                            start,
                            end: pos + 2,
                        },
                    });
                    pos += 2;
                } else {
                    tokens.push(Token {
                        kind: TokenKind::Gt,
                        span: Span {
                            start,
                            end: pos + 1,
                        },
                    });
                    pos += 1;
                }
            }
            b'(' => {
                tokens.push(Token {
                    kind: TokenKind::LParen,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b')' => {
                tokens.push(Token {
                    kind: TokenKind::RParen,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b'[' => {
                tokens.push(Token {
                    kind: TokenKind::LBracket,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b']' => {
                tokens.push(Token {
                    kind: TokenKind::RBracket,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            b',' => {
                tokens.push(Token {
                    kind: TokenKind::Comma,
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
                pos += 1;
            }
            _ => {
                return Err(SpannedError {
                    message: format!("unexpected character: '{}'", b as char),
                    span: Span {
                        start,
                        end: pos + 1,
                    },
                });
            }
        }
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span {
            start: pos,
            end: pos,
        },
    });

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(input: &str) -> Vec<TokenKind> {
        lex(input)
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lex_integer() {
        assert_eq!(kinds("42"), vec![TokenKind::Int(42), TokenKind::Eof]);
    }

    #[test]
    fn lex_identifiers_and_keywords() {
        assert_eq!(
            kinds("for x from to end if print and or while fn return break foo"),
            vec![
                TokenKind::For,
                TokenKind::Ident("x".into()),
                TokenKind::From,
                TokenKind::To,
                TokenKind::End,
                TokenKind::If,
                TokenKind::Print,
                TokenKind::And,
                TokenKind::Or,
                TokenKind::While,
                TokenKind::Fn,
                TokenKind::Return,
                TokenKind::Break,
                TokenKind::Ident("foo".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_operators() {
        assert_eq!(
            kinds("+ - * / % = += == != < > <= >="),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Eq,
                TokenKind::PlusEq,
                TokenKind::EqEq,
                TokenKind::BangEq,
                TokenKind::Lt,
                TokenKind::Gt,
                TokenKind::LtEq,
                TokenKind::GtEq,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_parens() {
        assert_eq!(
            kinds("(1 + 2)"),
            vec![
                TokenKind::LParen,
                TokenKind::Int(1),
                TokenKind::Plus,
                TokenKind::Int(2),
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_newline_collapsing() {
        assert_eq!(
            kinds("x = 1\n\n\ny = 2"),
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Newline,
                TokenKind::Ident("y".into()),
                TokenKind::Eq,
                TokenKind::Int(2),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_leading_trailing_newlines_suppressed() {
        assert_eq!(
            kinds("\n\nx = 1\n\n"),
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_comments() {
        assert_eq!(
            kinds("x = 1 # comment\ny = 2"),
            vec![
                TokenKind::Ident("x".into()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Newline,
                TokenKind::Ident("y".into()),
                TokenKind::Eq,
                TokenKind::Int(2),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_unexpected_char() {
        let err = lex("x @ y").unwrap_err();
        assert_eq!(err.message, "unexpected character: '@'");
        assert_eq!(err.span, Span { start: 2, end: 3 });
    }

    #[test]
    fn lex_brackets_and_comma() {
        assert_eq!(
            kinds("arr[i] = foo(a, b)"),
            vec![
                TokenKind::Ident("arr".into()),
                TokenKind::LBracket,
                TokenKind::Ident("i".into()),
                TokenKind::RBracket,
                TokenKind::Eq,
                TokenKind::Ident("foo".into()),
                TokenKind::LParen,
                TokenKind::Ident("a".into()),
                TokenKind::Comma,
                TokenKind::Ident("b".into()),
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_dollar_ident() {
        assert_eq!(
            kinds("$abc = 1"),
            vec![
                TokenKind::Ident("$abc".into()),
                TokenKind::Eq,
                TokenKind::Int(1),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_dollar_ident_hex() {
        assert_eq!(
            kinds("$e3a + $f7b"),
            vec![
                TokenKind::Ident("$e3a".into()),
                TokenKind::Plus,
                TokenKind::Ident("$f7b".into()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lex_dollar_alone_is_error() {
        let err = lex("$ x").unwrap_err();
        assert_eq!(err.message, "expected identifier after '$'");
    }

    #[test]
    fn lex_spans() {
        let tokens = lex("total += 10").unwrap();
        assert_eq!(tokens[0].span, Span { start: 0, end: 5 }); // "total"
        assert_eq!(tokens[1].span, Span { start: 6, end: 8 }); // "+="
        assert_eq!(tokens[2].span, Span { start: 9, end: 11 }); // "10"
    }
}
