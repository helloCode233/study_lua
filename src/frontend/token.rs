/// 词法 token 的“值级表示”。
///
/// 说明：
/// - `Identifier(String)` / `Number(f64)` 这类 token 携带语义值
/// - 关键字和符号 token 只表达类别，不携带额外内容
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(f64),
    True,
    False,
    Nil,
    Local,
    Function,
    Return,
    If,
    Then,
    Else,
    End,
    Plus,
    Minus,
    Star,
    Slash,
    Less,
    EqEq,
    Assign,
    LParen,
    RParen,
    Comma,
    Eof,
}

/// 词法 token 的“类别级表示”（不携带值）。
///
/// 为什么要单独有 `TokenTag`：
/// - parser 在做语法匹配时通常只关心“类别”（比如期待 `RParen`）
/// - 若直接在 `TokenKind` 上匹配会频繁处理 `Identifier(String)` 里的值
/// - 分离后，错误信息和匹配逻辑都更简洁
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TokenTag {
    Identifier,
    Number,
    True,
    False,
    Nil,
    Local,
    Function,
    Return,
    If,
    Then,
    Else,
    End,
    Plus,
    Minus,
    Star,
    Slash,
    Less,
    EqEq,
    Assign,
    LParen,
    RParen,
    Comma,
    Eof,
}

/// 带源码位置信息的 token。
///
/// `line/col` 用于在 parse/codegen 阶段构造高质量错误定位信息。
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl TokenKind {
    /// 把值级 token 映射到类别级 tag。
    pub fn tag(&self) -> TokenTag {
        match self {
            TokenKind::Identifier(_) => TokenTag::Identifier,
            TokenKind::Number(_) => TokenTag::Number,
            TokenKind::True => TokenTag::True,
            TokenKind::False => TokenTag::False,
            TokenKind::Nil => TokenTag::Nil,
            TokenKind::Local => TokenTag::Local,
            TokenKind::Function => TokenTag::Function,
            TokenKind::Return => TokenTag::Return,
            TokenKind::If => TokenTag::If,
            TokenKind::Then => TokenTag::Then,
            TokenKind::Else => TokenTag::Else,
            TokenKind::End => TokenTag::End,
            TokenKind::Plus => TokenTag::Plus,
            TokenKind::Minus => TokenTag::Minus,
            TokenKind::Star => TokenTag::Star,
            TokenKind::Slash => TokenTag::Slash,
            TokenKind::Less => TokenTag::Less,
            TokenKind::EqEq => TokenTag::EqEq,
            TokenKind::Assign => TokenTag::Assign,
            TokenKind::LParen => TokenTag::LParen,
            TokenKind::RParen => TokenTag::RParen,
            TokenKind::Comma => TokenTag::Comma,
            TokenKind::Eof => TokenTag::Eof,
        }
    }

    /// 用于错误信息展示的可读名称。
    ///
    /// 例如：`identifier(foo)`、`number(3.14)`、`+`、`end`。
    pub fn display_name(&self) -> String {
        match self {
            TokenKind::Identifier(name) => format!("identifier({})", name),
            TokenKind::Number(n) => format!("number({})", n),
            TokenKind::True => "true".to_string(),
            TokenKind::False => "false".to_string(),
            TokenKind::Nil => "nil".to_string(),
            TokenKind::Local => "local".to_string(),
            TokenKind::Function => "function".to_string(),
            TokenKind::Return => "return".to_string(),
            TokenKind::If => "if".to_string(),
            TokenKind::Then => "then".to_string(),
            TokenKind::Else => "else".to_string(),
            TokenKind::End => "end".to_string(),
            TokenKind::Plus => "+".to_string(),
            TokenKind::Minus => "-".to_string(),
            TokenKind::Star => "*".to_string(),
            TokenKind::Slash => "/".to_string(),
            TokenKind::Less => "<".to_string(),
            TokenKind::EqEq => "==".to_string(),
            TokenKind::Assign => "=".to_string(),
            TokenKind::LParen => "(".to_string(),
            TokenKind::RParen => ")".to_string(),
            TokenKind::Comma => ",".to_string(),
            TokenKind::Eof => "eof".to_string(),
        }
    }
}

impl TokenTag {
    /// 返回类别名称（用于“expected xxx”错误提示）。
    pub fn display_name(&self) -> &'static str {
        match self {
            TokenTag::Identifier => "identifier",
            TokenTag::Number => "number",
            TokenTag::True => "true",
            TokenTag::False => "false",
            TokenTag::Nil => "nil",
            TokenTag::Local => "local",
            TokenTag::Function => "function",
            TokenTag::Return => "return",
            TokenTag::If => "if",
            TokenTag::Then => "then",
            TokenTag::Else => "else",
            TokenTag::End => "end",
            TokenTag::Plus => "+",
            TokenTag::Minus => "-",
            TokenTag::Star => "*",
            TokenTag::Slash => "/",
            TokenTag::Less => "<",
            TokenTag::EqEq => "==",
            TokenTag::Assign => "=",
            TokenTag::LParen => "(",
            TokenTag::RParen => ")",
            TokenTag::Comma => ",",
            TokenTag::Eof => "eof",
        }
    }
}
