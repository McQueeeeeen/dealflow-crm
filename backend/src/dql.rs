use chrono::{Days, Months, NaiveDate};
use sqlx::{Encode, Postgres, Type};
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::StageKind;
use crate::pagination::Params;
use crate::validation::like_pattern;

const MAX_CHARS: usize = 500;
const MAX_DEPTH: usize = 16;
const MAX_CENTS: f64 = 1e16;
const FIELDS: &str = "value, stage, owner, company, status, closes, age, quiet";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
pub struct ParseError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    Text(String),
    Value(Cmp, i64),
    Stage(String),
    Owner(Option<String>),
    Company(String),
    Status(StageKind),
    Closes(Cmp, NaiveDate),
    Age(Cmp, i32),
    Quiet(Cmp, i32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Cond(Cond),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Open,
    Close,
    Minus,
    Op(Cmp),
    Word(String),
    Quoted(String),
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    start: usize,
    end: usize,
}

fn fail<T>(message: impl Into<String>, start: usize, end: usize) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
        start,
        end,
    })
}

fn is_special(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | ')' | ':' | '=' | '<' | '>' | '"')
}

fn lex(input: &[char]) -> Result<Vec<Token>, ParseError> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut i = 0;
    while i < input.len() {
        let start = i;
        let after_op = matches!(
            tokens.last(),
            Some(Token {
                tok: Tok::Op(_),
                ..
            })
        );
        let tok = match input[i] {
            c if c.is_whitespace() => {
                i += 1;
                continue;
            }
            '(' => Tok::Open,
            ')' => Tok::Close,
            '-' if !after_op && input.get(i + 1).is_some_and(|n| !is_special(*n)) => Tok::Minus,
            ':' | '=' => Tok::Op(Cmp::Eq),
            c @ ('<' | '>') => {
                let inclusive = input.get(i + 1) == Some(&'=');
                i += usize::from(inclusive);
                Tok::Op(match (c, inclusive) {
                    ('<', false) => Cmp::Lt,
                    ('<', true) => Cmp::Le,
                    ('>', false) => Cmp::Gt,
                    _ => Cmp::Ge,
                })
            }
            '"' => {
                let Some(close) = input[i + 1..].iter().position(|c| *c == '"') else {
                    return fail("this quote is never closed", start, input.len());
                };
                i += close + 1;
                Tok::Quoted(input[start + 1..i].iter().collect())
            }
            _ => {
                let len = input[i..]
                    .iter()
                    .position(|c| is_special(*c))
                    .unwrap_or(input.len() - i);
                i += len - 1;
                Tok::Word(input[start..=i].iter().collect())
            }
        };
        i += 1;
        tokens.push(Token { tok, start, end: i });
    }
    Ok(tokens)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    depth: usize,
    today: NaiveDate,
    len: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn keyword(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Token { tok: Tok::Word(w), .. }) if w == word)
    }

    fn here(&self) -> (usize, usize) {
        self.peek()
            .map_or((self.len, self.len), |t| (t.start, t.end))
    }

    fn or(&mut self) -> Result<Expr, ParseError> {
        let mut items = vec![self.and()?];
        while self.keyword("OR") {
            self.pos += 1;
            items.push(self.and()?);
        }
        Ok(if items.len() == 1 {
            items.remove(0)
        } else {
            Expr::Or(items)
        })
    }

    fn and(&mut self) -> Result<Expr, ParseError> {
        let mut items = Vec::new();
        while let Some(token) = self.peek() {
            if token.tok == Tok::Close || self.keyword("OR") {
                break;
            }
            items.push(self.unary()?);
        }
        match items.len() {
            0 => {
                let (start, end) = self.here();
                fail("expected a search term here", start, end)
            }
            1 => Ok(items.remove(0)),
            _ => Ok(Expr::And(items)),
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        match token.tok {
            Tok::Minus => Ok(Expr::Not(Box::new(self.unary()?))),
            Tok::Word(w) if w == "NOT" && self.peek().is_some() => {
                Ok(Expr::Not(Box::new(self.unary()?)))
            }
            Tok::Open => {
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return fail("too many nested parentheses", token.start, token.end);
                }
                let inner = self.or()?;
                if self.peek().map(|t| &t.tok) != Some(&Tok::Close) {
                    return fail("this parenthesis is never closed", token.start, token.end);
                }
                self.pos += 1;
                self.depth -= 1;
                Ok(inner)
            }
            Tok::Close => fail("nothing to close here", token.start, token.end),
            Tok::Op(_) => fail(
                "a comparison needs a field before it",
                token.start,
                token.end,
            ),
            Tok::Quoted(text) => Ok(Expr::Cond(Cond::Text(text))),
            Tok::Word(word) => match self.peek().map(|t| t.tok.clone()) {
                Some(Tok::Op(cmp)) => {
                    let op = (self.tokens[self.pos].start, self.tokens[self.pos].end);
                    self.pos += 1;
                    self.condition(&word, (token.start, token.end), cmp, op)
                }
                _ => Ok(Expr::Cond(Cond::Text(word))),
            },
        }
    }

    fn condition(
        &mut self,
        field: &str,
        at: (usize, usize),
        cmp: Cmp,
        op: (usize, usize),
    ) -> Result<Expr, ParseError> {
        let value = match self.peek() {
            Some(Token {
                tok: Tok::Word(v) | Tok::Quoted(v),
                start,
                end,
            }) => (v.clone(), *start, *end),
            _ => return fail(format!("expected a value after “{field}”"), op.0, op.1),
        };
        self.pos += 1;
        let (v, start, end) = (value.0.as_str(), value.1, value.2);
        let exact = |cond: Cond| -> Result<Cond, ParseError> {
            if cmp == Cmp::Eq {
                Ok(cond)
            } else {
                fail(
                    format!("use {field}:value, this field cannot be compared"),
                    op.0,
                    op.1,
                )
            }
        };
        let cond = match field.to_lowercase().as_str() {
            "value" | "amount" => match money(v) {
                Some(cents) => Cond::Value(cmp, cents),
                None => {
                    return fail(
                        format!("“{v}” is not an amount, try 50k or 1.2m"),
                        start,
                        end,
                    )
                }
            },
            "stage" => exact(Cond::Stage(v.to_owned()))?,
            "owner" => exact(Cond::Owner(
                (!v.eq_ignore_ascii_case("me")).then(|| v.to_owned()),
            ))?,
            "company" => exact(Cond::Company(v.to_owned()))?,
            "status" => exact(Cond::Status(match v.to_lowercase().as_str() {
                "open" => StageKind::Open,
                "won" => StageKind::Won,
                "lost" => StageKind::Lost,
                _ => return fail("status is open, won or lost", start, end),
            }))?,
            "closes" | "close" => match date(v, self.today) {
                Some(day) => Cond::Closes(cmp, day),
                None => {
                    return fail(
                        "dates look like 2026-12-31, today, 30d, 2w or 3m",
                        start,
                        end,
                    )
                }
            },
            "age" | "quiet" => match days(v) {
                Some(n) if field.eq_ignore_ascii_case("age") => Cond::Age(cmp, n),
                Some(n) => Cond::Quiet(cmp, n),
                None => return fail(format!("“{v}” is not a number of days"), start, end),
            },
            _ => return fail(format!("unknown field “{field}”, try {FIELDS}"), at.0, at.1),
        };
        Ok(Expr::Cond(cond))
    }
}

fn money(raw: &str) -> Option<i64> {
    let text = raw.trim_start_matches('$').replace(',', "").to_lowercase();
    let (number, scale) = match text.chars().last()? {
        'k' => (&text[..text.len() - 1], 1e3),
        'm' => (&text[..text.len() - 1], 1e6),
        _ => (text.as_str(), 1.0),
    };
    let cents = number.parse::<f64>().ok()? * scale * 100.0;
    (cents.is_finite() && (0.0..MAX_CENTS).contains(&cents)).then_some(cents.round() as i64)
}

fn offset(raw: &str) -> Option<(i64, char)> {
    let unit = raw.chars().last()?;
    let digits = raw.strip_suffix(unit)?;
    Some((digits.parse().ok()?, unit.to_ascii_lowercase()))
}

fn date(raw: &str, today: NaiveDate) -> Option<NaiveDate> {
    if raw.eq_ignore_ascii_case("today") {
        return Some(today);
    }
    if let Ok(day) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Some(day);
    }
    let (n, unit) = offset(raw)?;
    let forward = n >= 0;
    let n = u32::try_from(n.unsigned_abs()).ok()?;
    match (unit, forward) {
        ('d', true) => today.checked_add_days(Days::new(n.into())),
        ('d', false) => today.checked_sub_days(Days::new(n.into())),
        ('w', true) => today.checked_add_days(Days::new(u64::from(n) * 7)),
        ('w', false) => today.checked_sub_days(Days::new(u64::from(n) * 7)),
        ('m', true) => today.checked_add_months(Months::new(n)),
        ('m', false) => today.checked_sub_months(Months::new(n)),
        _ => None,
    }
}

fn days(raw: &str) -> Option<i32> {
    if let Ok(n) = raw.parse::<i32>() {
        return (n >= 0).then_some(n);
    }
    let (n, unit) = offset(raw)?;
    let factor = match unit {
        'd' => 1,
        'w' => 7,
        'm' => 30,
        _ => return None,
    };
    i32::try_from(n.checked_mul(factor)?)
        .ok()
        .filter(|n| *n >= 0)
}

pub fn parse(input: &str, today: NaiveDate) -> Result<Option<Expr>, ParseError> {
    let chars: Vec<char> = input.chars().collect();
    if chars.len() > MAX_CHARS {
        return fail(
            format!("keep the filter under {MAX_CHARS} characters"),
            MAX_CHARS,
            chars.len(),
        );
    }
    let tokens = lex(&chars)?;
    if tokens.is_empty() {
        return Ok(None);
    }
    let mut parser = Parser {
        tokens,
        pos: 0,
        depth: 0,
        today,
        len: chars.len(),
    };
    let expr = parser.or()?;
    match parser.peek() {
        Some(token) => fail("nothing to close here", token.start, token.end),
        None => Ok(Some(expr)),
    }
}

impl Cmp {
    fn sql(self) -> &'static str {
        match self {
            Self::Eq => "=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        }
    }
}

struct Compiler {
    params: Params,
    me: Uuid,
}

impl Compiler {
    fn bind<'q, T: 'q + Encode<'q, Postgres> + Type<Postgres>>(
        &mut self,
        value: T,
    ) -> AppResult<String> {
        let placeholder = self.params.placeholder();
        self.params = std::mem::take(&mut self.params).bind(value)?;
        Ok(placeholder)
    }

    fn expr(&mut self, expr: &Expr) -> AppResult<String> {
        Ok(match expr {
            Expr::Cond(cond) => format!("coalesce({}, false)", self.cond(cond)?),
            Expr::Not(inner) => format!("NOT {}", self.expr(inner)?),
            Expr::And(items) | Expr::Or(items) => {
                let joiner = if matches!(expr, Expr::And(_)) {
                    " AND "
                } else {
                    " OR "
                };
                let parts = items
                    .iter()
                    .map(|e| self.expr(e))
                    .collect::<AppResult<Vec<_>>>()?;
                format!("({})", parts.join(joiner))
            }
        })
    }

    fn cond(&mut self, cond: &Cond) -> AppResult<String> {
        let days_since =
            |column: &str| format!("floor(extract(epoch FROM now() - {column}) / 86400)");
        Ok(match cond {
            Cond::Text(text) => format!("title ILIKE {}", self.bind(like_pattern(text))?),
            Cond::Value(cmp, cents) => format!("value_cents {} {}", cmp.sql(), self.bind(*cents)?),
            Cond::Stage(name) => {
                let prefix = like_pattern(name)[1..].to_owned();
                format!("stage_name ILIKE {}", self.bind(prefix)?)
            }
            Cond::Owner(None) => {
                let me = self.me;
                format!("owner_id = {}", self.bind(me)?)
            }
            Cond::Owner(Some(name)) => {
                format!("owner_name ILIKE {}", self.bind(like_pattern(name))?)
            }
            Cond::Company(name) => format!("company_name ILIKE {}", self.bind(like_pattern(name))?),
            Cond::Status(kind) => format!("stage_kind = {}", self.bind(*kind)?),
            Cond::Closes(cmp, day) => {
                format!("expected_close_date {} {}", cmp.sql(), self.bind(*day)?)
            }
            Cond::Age(cmp, n) => format!(
                "{} {} {}",
                days_since("created_at"),
                cmp.sql(),
                self.bind(*n)?
            ),
            Cond::Quiet(cmp, n) => {
                format!(
                    "{} {} {}",
                    days_since("last_activity_at"),
                    cmp.sql(),
                    self.bind(*n)?
                )
            }
        })
    }
}

pub fn compile(expr: Option<&Expr>, params: Params, me: Uuid) -> AppResult<(String, Params)> {
    let Some(expr) = expr else {
        return Ok(("true".to_owned(), params));
    };
    let mut compiler = Compiler { params, me };
    let sql = compiler.expr(expr)?;
    Ok((sql, compiler.params))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 21).unwrap()
    }

    fn ok(input: &str) -> Expr {
        parse(input, today()).unwrap().unwrap()
    }

    fn err(input: &str) -> (String, usize, usize) {
        let e = parse(input, today()).unwrap_err();
        (e.message, e.start, e.end)
    }

    fn text(t: &str) -> Expr {
        Expr::Cond(Cond::Text(t.into()))
    }

    #[test]
    fn words_are_title_searches_joined_by_and() {
        assert_eq!(
            ok("fleet renewal"),
            Expr::And(vec![text("fleet"), text("renewal")])
        );
        assert_eq!(ok("\"fleet renewal\""), text("fleet renewal"));
        assert_eq!(parse("   ", today()), Ok(None));
    }

    #[test]
    fn and_binds_tighter_than_or() {
        assert_eq!(
            ok("a b OR c"),
            Expr::Or(vec![Expr::And(vec![text("a"), text("b")]), text("c")])
        );
        assert_eq!(
            ok("a (b OR c)"),
            Expr::And(vec![text("a"), Expr::Or(vec![text("b"), text("c")])])
        );
        assert_eq!(ok("or"), text("or"), "only upper-case OR is an operator");
    }

    #[test]
    fn negation_with_minus_or_not() {
        let lost = Expr::Not(Box::new(Expr::Cond(Cond::Status(StageKind::Lost))));
        assert_eq!(ok("-status:lost"), lost);
        assert_eq!(ok("NOT status:lost"), lost);
        assert_eq!(
            ok("follow-up"),
            text("follow-up"),
            "a dash inside a word is text"
        );
    }

    #[test]
    fn typed_fields() {
        assert_eq!(
            ok("value>=50k"),
            Expr::Cond(Cond::Value(Cmp::Ge, 5_000_000))
        );
        assert_eq!(
            ok("amount<$1.5m"),
            Expr::Cond(Cond::Value(Cmp::Lt, 150_000_000))
        );
        assert_eq!(
            ok("value:12,500"),
            Expr::Cond(Cond::Value(Cmp::Eq, 1_250_000))
        );
        assert_eq!(ok("owner:me"), Expr::Cond(Cond::Owner(None)));
        assert_eq!(
            ok("owner:\"Priya R\""),
            Expr::Cond(Cond::Owner(Some("Priya R".into())))
        );
        assert_eq!(ok("stage:prop"), Expr::Cond(Cond::Stage("prop".into())));
        assert_eq!(ok("quiet>2w"), Expr::Cond(Cond::Quiet(Cmp::Gt, 14)));
        assert_eq!(ok("age<=30"), Expr::Cond(Cond::Age(Cmp::Le, 30)));
    }

    #[test]
    fn dates_are_absolute_or_relative_to_today() {
        let on = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(
            ok("closes<30d"),
            Expr::Cond(Cond::Closes(Cmp::Lt, on(2026, 11, 20)))
        );
        assert_eq!(
            ok("closes>-1w"),
            Expr::Cond(Cond::Closes(Cmp::Gt, on(2026, 10, 14)))
        );
        assert_eq!(
            ok("closes<=3m"),
            Expr::Cond(Cond::Closes(Cmp::Le, on(2027, 1, 21)))
        );
        assert_eq!(
            ok("closes:2026-12-31"),
            Expr::Cond(Cond::Closes(Cmp::Eq, on(2026, 12, 31)))
        );
        assert_eq!(
            ok("closes<today"),
            Expr::Cond(Cond::Closes(Cmp::Lt, today()))
        );
    }

    #[test]
    fn errors_point_at_the_problem() {
        assert_eq!(
            err("fleet valu>5"),
            (format!("unknown field “valu”, try {FIELDS}"), 6, 10)
        );
        assert_eq!(err("value>lots").1..err("value>lots").2, 6..10);
        assert_eq!(
            err("stage>3").0,
            "use stage:value, this field cannot be compared"
        );
        assert_eq!(err("status:maybe").0, "status is open, won or lost");
        assert_eq!(
            err("(a b"),
            ("this parenthesis is never closed".into(), 0, 1)
        );
        assert_eq!(err("a b)"), ("nothing to close here".into(), 3, 4));
        assert_eq!(err("a OR"), ("expected a search term here".into(), 4, 4));
        assert_eq!(
            err("value>"),
            ("expected a value after “value”".into(), 5, 6)
        );
        assert_eq!(err("\"open"), ("this quote is never closed".into(), 0, 5));
        assert_eq!(err(">5").0, "a comparison needs a field before it");
        assert_eq!(err(&"(".repeat(20)).0, "too many nested parentheses");
        assert!(err(&"x ".repeat(300))
            .0
            .starts_with("keep the filter under"));
    }

    #[test]
    fn spans_count_characters_not_bytes() {
        assert_eq!(err("сделка valu:1").1, 7);
    }

    #[test]
    fn compiles_to_parameterised_sql() {
        let me = Uuid::nil();
        let expr = ok("-status:lost (value>50k OR owner:me) \"it's\"");
        let start = Params::default().bind(me).unwrap();
        let (sql, params) = compile(Some(&expr), start, me).unwrap();
        assert_eq!(
            sql,
            "(NOT coalesce(stage_kind = $2, false) AND (coalesce(value_cents > $3, false) \
             OR coalesce(owner_id = $4, false)) AND coalesce(title ILIKE $5, false))"
        );
        assert_eq!(params.placeholder(), "$6");
        assert!(!sql.contains("it's"), "values never reach the SQL text");
        assert_eq!(compile(None, Params::default(), me).unwrap().0, "true");
    }
}
