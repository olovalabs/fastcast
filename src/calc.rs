//! Instant calculator: safe recursive-descent evaluation, no `eval`.
//!
//! Supports `+ - * / % ^`, parentheses, unary minus, postfix `%`
//! (percentage), constants `pi`/`e`, and functions
//! `sin cos tan asin acos atan sqrt cbrt ln log abs floor ceil round exp`.

/// Evaluate `expr`. Returns `None` for invalid expressions (so the UI can
/// simply show no calculator row) or non-finite results.
pub fn try_eval(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if expr.is_empty() || !looks_like_math(expr) {
        return None;
    }
    let mut parser = Parser::new(expr);
    let value = parser.parse_expr().ok()?;
    parser.skip_ws();
    if parser.pos != parser.chars.len() {
        return None;
    }
    if !value.is_finite() {
        return None;
    }
    Some(format_number(value))
}

/// Heuristic so plain app names never trigger the calculator row.
fn looks_like_math(expr: &str) -> bool {
    let has_digit = expr.chars().any(|c| c.is_ascii_digit());
    let has_op = expr.chars().any(|c| "+-*/%^()".contains(c));
    let is_func = expr
        .trim_start()
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
        && expr.contains('(');
    has_digit && (has_op || is_func)
}

fn format_number(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    let mut s = format!("{v:.10}");
    while s.contains('.') && (s.ends_with('0') || s.ends_with('.')) {
        s.pop();
        if !s.contains('.') {
            break;
        }
    }
    s
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(expr: &str) -> Self {
        Self {
            chars: expr.chars().collect(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn parse_expr(&mut self) -> Result<f64, ()> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.pos += 1;
                    value += self.parse_term()?;
                }
                Some('-') => {
                    self.pos += 1;
                    value -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn parse_term(&mut self) -> Result<f64, ()> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    value *= self.parse_factor()?;
                }
                Some('/') => {
                    self.pos += 1;
                    let rhs = self.parse_factor()?;
                    if rhs == 0.0 {
                        return Err(());
                    }
                    value /= rhs;
                }
                Some('%') => {
                    self.pos += 1;
                    // `a % b` = modulo, trailing `%` (e.g. in `50%`) is handled
                    // as percentage inside parse_factor.
                    if self.next_is_operand() {
                        value %= self.parse_factor()?;
                    } else {
                        value /= 100.0;
                    }
                }
                _ => break,
            }
        }
        Ok(value)
    }

    fn next_is_operand(&mut self) -> bool {
        let mut i = self.pos;
        while i < self.chars.len() && self.chars[i].is_whitespace() {
            i += 1;
        }
        matches!(self.chars.get(i), Some(c) if c.is_ascii_digit() || *c == '(' || *c == '.')
    }

    fn parse_factor(&mut self) -> Result<f64, ()> {
        self.skip_ws();
        let base = self.parse_unary()?;
        self.skip_ws();
        if self.peek() == Some('^') {
            self.pos += 1;
            let exp = self.parse_factor()?;
            return Ok(base.powf(exp));
        }
        // Postfix percent: `50%` -> 0.5, `200+10%` handled as 200 + (200*0.1)?
        // Keep it simple and predictable: postfix % always means /100.
        Ok(base)
    }

    fn parse_unary(&mut self) -> Result<f64, ()> {
        self.skip_ws();
        match self.peek() {
            Some('-') => {
                self.pos += 1;
                Ok(-self.parse_unary()?)
            }
            Some('+') => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, ()> {
        self.skip_ws();
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                let value = self.parse_expr()?;
                self.skip_ws();
                if self.peek() != Some(')') {
                    return Err(());
                }
                self.pos += 1;
                Ok(self.apply_postfix(value))
            }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let value = self.parse_number()?;
                Ok(self.apply_postfix(value))
            }
            Some(c) if c.is_ascii_alphabetic() => self.parse_named(),
            _ => Err(()),
        }
    }

    /// Trailing `%` means percent.
    fn apply_postfix(&mut self, value: f64) -> f64 {
        self.skip_ws();
        if self.peek() == Some('%') && !self.next_is_operand() {
            self.pos += 1;
            return value / 100.0;
        }
        value
    }

    fn parse_number(&mut self) -> Result<f64, ()> {
        let start = self.pos;
        let mut dots = 0;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.pos += 1;
            } else if c == '.' && dots == 0 {
                dots += 1;
                self.pos += 1;
            } else {
                break;
            }
        }
        self.chars[start..self.pos]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| ())
    }

    fn parse_named(&mut self) -> Result<f64, ()> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric()) {
            self.pos += 1;
        }
        let name: String = self.chars[start..self.pos].iter().collect();
        let name = name.to_lowercase();
        match name.as_str() {
            "pi" => Ok(self.apply_postfix(std::f64::consts::PI)),
            "e" => Ok(self.apply_postfix(std::f64::consts::E)),
            "tau" => Ok(self.apply_postfix(std::f64::consts::TAU)),
            _ => {
                self.skip_ws();
                if self.peek() != Some('(') {
                    return Err(());
                }
                self.pos += 1;
                let arg = self.parse_expr()?;
                self.skip_ws();
                if self.peek() != Some(')') {
                    return Err(());
                }
                self.pos += 1;
                let value = match name.as_str() {
                    "sin" => arg.to_radians().sin(),
                    "cos" => arg.to_radians().cos(),
                    "tan" => arg.to_radians().tan(),
                    "asin" => arg.asin().to_degrees(),
                    "acos" => arg.acos().to_degrees(),
                    "atan" => arg.atan().to_degrees(),
                    "sqrt" => arg.sqrt(),
                    "cbrt" => arg.cbrt(),
                    "ln" => arg.ln(),
                    "log" => arg.log10(),
                    "abs" => arg.abs(),
                    "floor" => arg.floor(),
                    "ceil" => arg.ceil(),
                    "round" => arg.round(),
                    "exp" => arg.exp(),
                    _ => return Err(()),
                };
                Ok(self.apply_postfix(value))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::try_eval;

    #[test]
    fn arithmetic() {
        assert_eq!(try_eval("2+2"), Some("4".into()));
        assert_eq!(try_eval("2 + 3 * 4"), Some("14".into()));
        assert_eq!(try_eval("(2+3)*4"), Some("20".into()));
        assert_eq!(try_eval("2^10"), Some("1024".into()));
        assert_eq!(try_eval("7/2"), Some("3.5".into()));
        assert_eq!(try_eval("-5+3"), Some("-2".into()));
    }

    #[test]
    fn percent_and_funcs() {
        assert_eq!(try_eval("50%"), Some("0.5".into()));
        assert_eq!(try_eval("sqrt(16)"), Some("4".into()));
        assert_eq!(try_eval("sin(30)"), Some("0.5".into()));
        assert_eq!(try_eval("2*pi"), Some("6.2831853072".into()));
    }

    #[test]
    fn rejects_non_math() {
        assert_eq!(try_eval("firefox"), None);
        assert_eq!(try_eval(""), None);
        assert_eq!(try_eval("1/0"), None);
        assert_eq!(try_eval("2+"), None);
        assert_eq!(try_eval("hello world"), None);
    }
}
