// ┌──────────────────────────────────────────────────────────┐
// │  MORPHIC PARSER                                          │
// │  Parses .morph specification files into AST               │
// └──────────────────────────────────────────────────────────┘

use super::ast::*;
use std::collections::HashMap;
use std::num::ParseIntError;

/// Parse a Morphic source string into a Spec AST
pub fn parse(source: &str) -> Result<Spec, ParseError> {
    Parser::new(source).parse_spec()
}

/// Parse ONE constraint from bare text — the whole input
/// must be that constraint (trailing whitespace/comments
/// allowed). This is the entry the verifier's loop-
/// invariant machinery uses (Phase 96): an IR loop's
/// `invariant` string is spec-syntax constraint text over
/// the loop's environment, and it is parsed by exactly the
/// constraint machinery the spec parser applies to a
/// module's `constraint:` lines — no second dialect.
pub fn parse_constraint_text(source: &str) -> Result<Constraint, ParseError> {
    let mut parser = Parser::new(source);
    let constraint = parser.parse_constraint()?;
    parser.skip_whitespace_and_comments();
    if !parser.is_eof() {
        return Err(ParseError {
            message: "trailing input after constraint".to_string(),
            line: parser.line,
            column: parser.col,
            snippet: source.to_string(),
        });
    }
    Ok(constraint)
}

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error at line {}, column {}: {}\n  {}",
            self.line, self.column, self.message, self.snippet
        )
    }
}

impl std::error::Error for ParseError {}

// ── Internal Parser State ─────────────────────────────────

struct Parser<'a> {
    source: &'a str,
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn parse_spec(&mut self) -> Result<Spec, ParseError> {
        self.skip_whitespace_and_comments();
        let name = self.parse_identifier()?;
        let mut imports = Vec::new();
        let mut functions = Vec::new();
        let mut types = Vec::new();
        let mut invariants = Vec::new();

        self.skip_whitespace_and_comments();
        self.expect_char('{')?;

        let mut closed = false;
        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                self.advance();
                closed = true;
                break;
            }

            let keyword = self.parse_identifier()?;
            match keyword.as_str() {
                "import" => {
                    let path = self.parse_string_literal()?;
                    let alias = if self.peek() == Some('a') || self.peek() == Some('i') {
                        let ahead = self.peek_n(2);
                        if ahead == "as" {
                            self.eat_keyword("as")?;
                            Some(self.parse_identifier()?)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    self.expect_char(';')?;
                    imports.push(Import { path, alias });
                }

                "spec" => {
                    let func = self.parse_function_spec()?;
                    functions.push(func);
                }

                "type" => {
                    let td = self.parse_type_def()?;
                    types.push(td);
                }

                "invariant" => {
                    let name = self.parse_identifier()?;
                    self.expect_char(':')?;
                    let constraint = self.parse_constraint()?;
                    self.expect_char(';')?;
                    invariants.push(GlobalInvariant { name, constraint });
                }

                other => {
                    return self.error(format!("Unexpected '{}'. Expected 'import', 'spec', 'type', or 'invariant'.", other));
                }
            }
        }

        if !closed {
            return self.error(format!(
                "Unexpected end of file: missing '}}' to close module '{}'",
                name
            ));
        }

        Ok(Spec { name, imports, functions, types, invariants })
    }

    fn parse_function_spec(&mut self) -> Result<FunctionSpec, ParseError> {
        // spec name<generics...>
        let name = self.parse_identifier()?;

        // Parse optional generics: <T, U, V where ...>
        let generics = if self.peek() == Some('<') {
            self.parse_generics()?
        } else {
            Vec::new()
        };

        self.skip_whitespace_and_comments();
        self.expect_char('{')?;

        let mut doc = None;
        let mut params = Vec::new();
        let mut return_type = TypeRef::Unit;
        let mut preconditions = Vec::new();
        let mut postconditions = Vec::new();
        let mut invariants = Vec::new();
        let mut complexity = None;
        let mut resource = None;
        let mut tests = Vec::new();
        let mut annotations = HashMap::new();

        let mut closed = false;
        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                self.advance();
                closed = true;
                break;
            }

            // Doc comment
            if self.peek_n(3) == "///" {
                self.eat_line();
                continue;
            }

            let keyword = self.parse_identifier()?;
            match keyword.as_str() {
                "doc" => {
                    self.expect_char(':')?;
                    doc = Some(self.parse_string_literal_value()?);
                }

                "input" | "params" => {
                    self.expect_char(':')?;
                    // Parse parameter list: name: Type, name: Type
                    params = self.parse_param_list()?;
                }

                "output" | "returns" => {
                    self.expect_char(':')?;
                    return_type = self.parse_type_ref()?;
                }

                "require" | "precondition" => {
                    self.expect_char(':')?;
                    let c = self.parse_constraint()?;
                    preconditions.push(c);
                }

                "ensure" | "postcondition" => {
                    self.expect_char(':')?;
                    let c = self.parse_constraint()?;
                    postconditions.push(c);
                }

                "constraint" => {
                    self.expect_char(':')?;
                    let c = self.parse_constraint()?;
                    postconditions.push(c); // constraint = postcondition in shorthand
                }

                "invariant" => {
                    self.expect_char(':')?;
                    let c = self.parse_constraint()?;
                    invariants.push(Invariant { name: None, constraint: c });
                }

                "optimize" | "opt" => {
                    self.expect_char(':')?;
                    let dim = self.parse_identifier()?;
                    self.skip_whitespace_and_comments();
                    // Parse < O(bound)
                    let has_lt = self.peek() == Some('<');
                    if has_lt { self.advance(); }
                    self.skip_whitespace_and_comments();
                    let bound = self.parse_big_o()?;

                    let dimension = match dim.as_str() {
                        "time" => ComplexityDimension::Time,
                        "space" => ComplexityDimension::Space,
                        "amortized" => ComplexityDimension::AmortizedTime,
                        "communication" => ComplexityDimension::Communication,
                        d => return self.error(format!("Unknown complexity dimension: {}. Use time/space/amortized/communication.", d)),
                    };

                    complexity = Some(ComplexityBound {
                        dimension,
                        bound,
                        condition: None,
                    });
                }

                "resource" => {
                    self.expect_char(':')?;
                    let res_name = self.parse_identifier()?;
                    self.skip_whitespace_and_comments();
                    let mut max = None;
                    let mut predicate = None;
                    if self.peek() == Some('<') {
                        self.advance();
                        self.skip_whitespace_and_comments();
                        if self.peek() == Some('O') && self.peek_ahead() == Some('(') {
                            // Big-O bound, e.g. `allocations < O(n)`
                            // (LANGUAGE_REFERENCE section 8). Not a
                            // plain amount, so keep it as the predicate.
                            let bound = self.parse_big_o()?;
                            predicate = Some(bound.to_string());
                        } else {
                            let n = self.parse_integer()?;
                            // Optional unit suffix (section 8):
                            // KB / MB / GB scale the amount.
                            self.skip_whitespace_and_comments();
                            let multiplier = if self.is_ident_start(self.peek()) {
                                match self.parse_identifier()?.as_str() {
                                    "KB" => 1024u64,
                                    "MB" => 1024 * 1024,
                                    "GB" => 1024 * 1024 * 1024,
                                    u => {
                                        return self.error(format!(
                                            "Unknown resource unit: {}. Use KB/MB/GB.",
                                            u
                                        ))
                                    }
                                }
                            } else {
                                1
                            };
                            max = Some((n as u64) * multiplier);
                        }
                        // No closing '>': in the section 8 grammar the
                        // '<' is a comparison, not a bracket.
                    }

                    let resource_kind = match res_name.as_str() {
                        "memory" => Resource::MemoryBytes,
                        "allocations" => Resource::Allocations,
                        "syscalls" => Resource::Syscalls,
                        "net_io" => Resource::NetworkIO,
                        "disk_io" => Resource::DiskIO,
                        r => return self.error(format!("Unknown resource: {}", r)),
                    };

                    resource = Some(ResourceBound {
                        resource: resource_kind,
                        max_amount: max,
                        predicate,
                    });
                }

                "test" => {
                    let test = self.parse_test()?;
                    tests.push(test);
                }

                "@" => {
                    // Annotation: @key = "value"
                    let key = self.parse_identifier()?;
                    self.expect_char('=')?;
                    let value = self.parse_string_literal_value()?;
                    annotations.insert(key, value);
                }

                _ => {
                    return self.error(format!("Unexpected '{}' in spec body.", keyword));
                }
            }
        }

        if !closed {
            return self.error(format!(
                "Unexpected end of file: missing '}}' to close spec '{}'",
                name
            ));
        }

        Ok(FunctionSpec {
            name,
            doc,
            generics,
            params,
            return_type,
            preconditions,
            postconditions,
            invariants,
            complexity,
            resource,
            tests,
            annotations,
        })
    }

    // ── Parsing Helpers ──────────────────────────────────

    fn parse_constraint(&mut self) -> Result<Constraint, ParseError> {
        let lhs = self.parse_simple_constraint()?;
        // `a implies b` — documented in LANGUAGE_REFERENCE section 6
        // and supported by the AST, type checker, verifier and
        // synthesis engine, but never produced by the parser.
        // Implication binds loosest and associates to the right.
        self.skip_whitespace_and_comments();
        if self.peek() == Some('i') && self.peek_n(7) == "implies" {
            self.eat_keyword("implies")?;
            let rhs = self.parse_constraint()?;
            return Ok(Constraint::Implies(Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn parse_simple_constraint(&mut self) -> Result<Constraint, ParseError> {
        self.skip_whitespace_and_comments();

        if self.peek() == Some('t') {
            if self.peek_n(4) == "true" {
                self.eat_ident("true");
                return Ok(Constraint::True);
            }
        }

        if self.peek() == Some('f') {
            let ahead = self.peek_n(6);
            if ahead.starts_with("forall") {
                self.eat_ident("forall");
                return self.parse_forall();
            }
        }

        if self.peek() == Some('e') {
            if self.peek_n(6).starts_with("exists") {
                self.eat_ident("exists");
                return self.parse_exists();
            }
        }

        if self.peek() == Some('!') {
            self.advance();
            // Negation binds tighter than implication, so the operand
            // is a simple constraint, not a full one.
            let inner = self.parse_simple_constraint()?;
            return Ok(Constraint::Not(Box::new(inner)));
        }

        if self.peek() == Some('(') {
            self.advance();
            let mut parts = vec![self.parse_constraint()?];
            while {
                self.skip_whitespace_and_comments();
                self.peek() == Some('&') || self.peek() == Some('|')
            } {
                let is_and = self.peek() == Some('&');
                self.advance();
                if is_and { self.advance(); } // consume &&
                if !is_and { self.advance(); } // consume ||
                parts.push(self.parse_constraint()?);
            }
            self.expect_char(')')?;
            // Return the appropriate composite
            if parts.len() == 1 {
                return Ok(parts.into_iter().next().unwrap());
            }
            // This is a simplified heuristic — in production we'd track operators
            return Ok(Constraint::And(parts));
        }

        // Parse an expression and then see what follows
        let expr = self.parse_expr()?;
        self.skip_whitespace_and_comments();

        // Check for comparison operators
        if self.peek() == Some('=') {
            self.eat_str("==")?;
            let rhs = self.parse_expr()?;
            return Ok(Constraint::Eq(expr, rhs));
        }

        if self.peek() == Some('<') {
            self.advance();
            let eq = self.peek() == Some('=');
            if eq { self.advance(); }
            let rhs = self.parse_expr()?;
            return Ok(if eq {
                Constraint::Order(OrderOp::LessThanOrEqual, expr, rhs)
            } else {
                Constraint::Order(OrderOp::LessThan, expr, rhs)
            });
        }

        if self.peek() == Some('>') {
            self.advance();
            let eq = self.peek() == Some('=');
            if eq { self.advance(); }
            let rhs = self.parse_expr()?;
            return Ok(if eq {
                Constraint::Order(OrderOp::GreaterThanOrEqual, expr, rhs)
            } else {
                Constraint::Order(OrderOp::GreaterThan, expr, rhs)
            });
        }

        // If it looks like a predicate call: is_sorted(list)
        if let Expr::Call(name, args) = &expr {
            return Ok(Constraint::Predicate(name.clone(), args.clone()));
        }

        // Treat as boolean expression
        Ok(Constraint::Expr(expr))
    }

    fn parse_forall(&mut self) -> Result<Constraint, ParseError> {
        self.skip_whitespace_and_comments();
        let mut vars = Vec::new();
        // Parse: name: Type, name: Type in collection
        loop {
            let vname = self.parse_identifier()?;
            self.expect_char(':')?;
            let vtype = self.parse_type_ref()?;
            vars.push((vname, vtype));
            if self.peek() == Some(',') {
                self.advance();
                continue;
            }
            break;
        }
        self.skip_whitespace_and_comments();
        self.eat_keyword("in")?;
        let _collection = self.parse_expr()?;
        self.expect_char(':')?;
        let body = self.parse_constraint()?;
        Ok(Constraint::Forall { vars, body: Box::new(body) })
    }

    fn parse_exists(&mut self) -> Result<Constraint, ParseError> {
        self.skip_whitespace_and_comments();
        let mut vars = Vec::new();
        loop {
            let vname = self.parse_identifier()?;
            self.expect_char(':')?;
            let vtype = self.parse_type_ref()?;
            vars.push((vname, vtype));
            if self.peek() == Some(',') {
                self.advance();
                continue;
            }
            break;
        }
        self.skip_whitespace_and_comments();
        self.eat_keyword("in")?;
        let _collection = self.parse_expr()?;
        self.expect_char(':')?;
        let body = self.parse_constraint()?;
        Ok(Constraint::Exists { vars, body: Box::new(body) })
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace_and_comments();
        self.parse_expr_or()
    }

    fn parse_expr_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_and()?;
        loop {
            self.skip_whitespace_and_comments();
            if self.peek_n(2) != "||" {
                break;
            }
            self.eat_str("||")?;
            let right = self.parse_expr_and()?;
            left = Expr::BinOp(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_expr_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_compare()?;
        loop {
            self.skip_whitespace_and_comments();
            if self.peek_n(2) != "&&" {
                break;
            }
            self.eat_str("&&")?;
            let right = self.parse_expr_compare()?;
            left = Expr::BinOp(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_expr_compare(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_add()?;
        self.skip_whitespace_and_comments();
        if self.peek() == Some('<')
            || self.peek() == Some('>')
            || self.peek() == Some('=')
            || self.peek() == Some('!')
        {
            let op = match self.peek_n(2).as_str() {
                "<=" => { self.eat_str("<=")?; BinOp::Le }
                ">=" => { self.eat_str(">=")?; BinOp::Ge }
                "==" => { self.eat_str("==")?; BinOp::Eq }
                "!=" => { self.eat_str("!=")?; BinOp::Neq }
                _ => {
                    let c = self.peek().unwrap();
                    self.advance();
                    match c {
                        '<' => BinOp::Lt,
                        '>' => BinOp::Gt,
                        _ => return Err(self.simple_error("Invalid comparison operator")),
                    }
                }
            };
            let right = self.parse_expr_add()?;
            left = Expr::BinOp(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_expr_add(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_mul()?;
        loop {
            self.skip_whitespace_and_comments();
            // `++` (BinOp::Concat) shares the additive level and
            // must be probed BEFORE `+`: it is the two-character
            // operator at this level, and a single-`+` probe
            // would consume its first character as Add and leave
            // the second as a parse error. Concat is the
            // additive operation on the operands the layers
            // below define it for (Strings, Lists — see the
            // interpreter and verifier), so it groups with
            // `+`/`-`: left-associative, operands at the
            // multiplicative level, binding tighter than the
            // comparisons handled by the caller.
            if self.peek_n(2) == "++" {
                self.eat_str("++")?;
                let right = self.parse_expr_mul()?;
                left = Expr::BinOp(BinOp::Concat, Box::new(left), Box::new(right));
                continue;
            }
            match self.peek() {
                Some('+') => {
                    self.advance();
                    let right = self.parse_expr_mul()?;
                    left = Expr::BinOp(BinOp::Add, Box::new(left), Box::new(right));
                }
                Some('-') => {
                    self.advance();
                    let right = self.parse_expr_mul()?;
                    left = Expr::BinOp(BinOp::Sub, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_expr_mul(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_atom()?;
        loop {
            self.skip_whitespace_and_comments();
            match self.peek() {
                Some('*') => {
                    self.advance();
                    let right = self.parse_expr_atom()?;
                    left = Expr::BinOp(BinOp::Mul, Box::new(left), Box::new(right));
                }
                Some('/') => {
                    self.advance();
                    let right = self.parse_expr_atom()?;
                    left = Expr::BinOp(BinOp::Div, Box::new(left), Box::new(right));
                }
                Some('%') => {
                    self.advance();
                    let right = self.parse_expr_atom()?;
                    left = Expr::BinOp(BinOp::Mod, Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    /// Call argument list; the `(` is already consumed.
    /// Extracted verbatim from the identifier arm's inline
    /// loop (Phase 73) so method calls parse their arguments
    /// with exactly the call grammar.
    fn parse_call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some(')') { self.advance(); break; }
            args.push(self.parse_expr()?);
            if self.peek() == Some(',') { self.advance(); continue; }
        }
        Ok(args)
    }

    /// Postfix chain (Phase 73): `.field`, `.method(args)` and
    /// `[index]` compose after ANY primary — a call result, a
    /// parenthesized expression, a literal, a record literal —
    /// not only after a bare variable. The pre-73 loop ran
    /// only on the variable path, so `output.unwrap().tokens`
    /// was unparseable (nothing after a call result could
    /// postfix) and `output.is_err()` parsed `.is_err` as a
    /// field and stranded on the `()`. A `.name` followed by
    /// `(` is a method call (Expr::MethodCall, decision D1a);
    /// a `.name` not followed by `(` is a field access.
    fn parse_postfix(&mut self, mut expr: Expr) -> Result<Expr, ParseError> {
        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('.') {
                self.advance();
                let name = self.parse_identifier()?;
                self.skip_whitespace_and_comments();
                if self.peek() == Some('(') {
                    self.advance();
                    let args = self.parse_call_args()?;
                    expr = Expr::MethodCall {
                        receiver: Box::new(expr),
                        name,
                        args,
                    };
                } else {
                    expr = Expr::Field(Box::new(expr), name);
                }
            } else if self.peek() == Some('[') {
                self.advance();
                let idx = self.parse_expr()?;
                self.expect_char(']')?;
                expr = Expr::Index(Box::new(expr), Box::new(idx));
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_expr_atom(&mut self) -> Result<Expr, ParseError> {
        self.skip_whitespace_and_comments();

        if self.peek() == Some('-') && !self.is_digit(self.peek_ahead()) {
            self.advance();
            let inner = self.parse_expr_atom()?;
            return Ok(Expr::UnaryOp(UnaryOp::Neg, Box::new(inner)));
        }

        if self.peek() == Some('!') {
            self.advance();
            let inner = self.parse_expr_atom()?;
            return Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(inner)));
        }

        // Integer literal
        if self.is_digit(self.peek()) {
            let n = self.parse_integer()?;
            return self.parse_postfix(Expr::IntLit(n));
        }

        // String literal
        if self.peek() == Some('"') {
            let s = self.parse_string_literal_value()?;
            return self.parse_postfix(Expr::StringLit(s));
        }

        // Char literal (Phase 45, finding F4): the reference
        // has always documented `'a'` / `'\n'` (section 1)
        // and the layers below have always carried Char —
        // only this arm was missing.
        if self.peek() == Some('\'') {
            let c = self.parse_char_literal_value()?;
            return self.parse_postfix(Expr::CharLit(c));
        }

        // Boolean literal
        if self.peek() == Some('t') {
            if self.peek_n(4) == "true" {
                self.eat_ident("true");
                return self.parse_postfix(Expr::BoolLit(true));
            }
        }
        if self.peek() == Some('f') {
            if self.peek_n(5) == "false" {
                self.eat_ident("false");
                return self.parse_postfix(Expr::BoolLit(false));
            }
        }

        // List literal: [1, 2, 3]
        if self.peek() == Some('[') {
            self.advance();
            let mut items = Vec::new();
            loop {
                self.skip_whitespace_and_comments();
                if self.peek() == Some(']') { self.advance(); break; }
                items.push(self.parse_expr()?);
                self.skip_whitespace_and_comments();
                if self.peek() == Some(',') { self.advance(); continue; }
            }
            // Return as Call to List constructor
            return self.parse_postfix(Expr::Call("List".into(), items));
        }

        // Parenthesized expression or tuple
        if self.peek() == Some('(') {
            self.advance();
            let inner = self.parse_expr()?;
            self.skip_whitespace_and_comments();
            if self.peek() == Some(',') {
                // Tuple
                let mut parts = vec![inner];
                while self.peek() == Some(',') {
                    self.advance();
                    parts.push(self.parse_expr()?);
                }
                self.expect_char(')')?;
                return self.parse_postfix(Expr::Call("__tuple__".into(), parts));
            }
            self.expect_char(')')?;
            return self.parse_postfix(inner);
        }

        // Lambda: |x, y| -> body
        if self.peek() == Some('|') {
            self.advance();
            let mut params = Vec::new();
            loop {
                let pname = self.parse_identifier()?;
                params.push((pname, TypeRef::Named("_".into())));
                if self.peek() == Some(',') { self.advance(); continue; }
                break;
            }
            self.expect_char('|')?;
            self.skip_whitespace_and_comments();
            // The arrow is optional: LANGUAGE_REFERENCE section 11
            // documents both `|x, y| x + y` and `|a, b| -> a > b`.
            if self.peek_n(2) == "->" {
                self.eat_str("->")?;
            }
            let body = self.parse_expr()?;
            return Ok(Expr::Lambda(params, Box::new(body)));
        }

        // Identifier — could be a variable, a function call,
        // or a record literal; all three take the shared
        // postfix chain (field / method / index) afterwards.
        if self.is_ident_start(self.peek()) {
            let ident = self.parse_identifier()?;

            // Check for function call: ident(args...)
            self.skip_whitespace_and_comments();
            if self.peek() == Some('(') {
                self.advance();
                let args = self.parse_call_args()?;
                return self.parse_postfix(Expr::Call(ident, args));
            }

            // Record literal: TypeName { field: value, ... }
            // (Phase 73, decision D3a — named construction).
            // In expression position an identifier followed by
            // `{` has no other reading in this grammar: blocks
            // are not expressions, and the spec / type / test
            // braces are consumed by their own parsers before
            // any expression is parsed. The bare `{ ... }`
            // form (the unratified D3b) still falls to the
            // "Expected expression" error below — a leading
            // `{` never reaches this arm.
            if self.peek() == Some('{') {
                self.advance();
                let mut fields = Vec::new();
                loop {
                    self.skip_whitespace_and_comments();
                    if self.peek() == Some('}') { self.advance(); break; }
                    let fname = self.parse_identifier()?;
                    self.expect_char(':')?;
                    let value = self.parse_expr()?;
                    fields.push((fname, value));
                    self.skip_whitespace_and_comments();
                    if self.peek() == Some(',') { self.advance(); continue; }
                }
                return self.parse_postfix(Expr::RecordLit {
                    type_name: ident,
                    fields,
                });
            }

            return self.parse_postfix(Expr::Var(ident));
        }

        Err(self.simple_error("Expected expression"))
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, ParseError> {
        self.skip_whitespace_and_comments();
        let name = self.parse_identifier()?;

        match name.as_str() {
            "fn" => {
                // Function type: fn(T1, T2) -> R
                // (LANGUAGE_REFERENCE section 4)
                self.expect_char('(')?;
                self.skip_whitespace_and_comments();
                let args = if self.peek() == Some(')') {
                    Vec::new()
                } else {
                    self.parse_type_args()?
                };
                self.expect_char(')')?;
                self.skip_whitespace_and_comments();
                self.eat_str("->")?;
                let ret = self.parse_type_ref()?;
                Ok(TypeRef::Function(args, Box::new(ret)))
            }

            "Int" => Ok(TypeRef::Int),
            "Float" => Ok(TypeRef::Float),
            "Bool" => Ok(TypeRef::Bool),
            "String" => Ok(TypeRef::String),
            "Char" => Ok(TypeRef::Char),
            "Unit" | "()" => Ok(TypeRef::Unit),
            "I8" => Ok(TypeRef::I8),
            "I16" => Ok(TypeRef::I16),
            "I32" => Ok(TypeRef::I32),
            "I64" => Ok(TypeRef::I64),
            "U8" => Ok(TypeRef::U8),
            "U16" => Ok(TypeRef::U16),
            "U32" => Ok(TypeRef::U32),
            "U64" => Ok(TypeRef::U64),

            other => {
                // Check for generic types: List<T>, Map<K,V>, etc.
                self.skip_whitespace_and_comments();
                if self.peek() == Some('<') {
                    self.advance();
                    let args = self.parse_type_args()?;
                    self.expect_char('>')?;

                    match other {
                        "List" => Ok(TypeRef::List(Box::new(args.into_iter().next().unwrap()))),
                        "Set" => Ok(TypeRef::Set(Box::new(args.into_iter().next().unwrap()))),
                        "Map" => {
                            let mut iter = args.into_iter();
                            Ok(TypeRef::Map(
                                Box::new(iter.next().unwrap()),
                                Box::new(iter.next().unwrap()),
                            ))
                        }
                        "Option" => Ok(TypeRef::Option(Box::new(args.into_iter().next().unwrap()))),
                        "Result" => {
                            let mut iter = args.into_iter();
                            Ok(TypeRef::Result(
                                Box::new(iter.next().unwrap()),
                                Box::new(iter.next().unwrap()),
                            ))
                        }
                        "Stream" => Ok(TypeRef::Stream(Box::new(args.into_iter().next().unwrap()))),
                        "Array" => {
                            let mut iter = args.into_iter();
                            let ty = iter.next().unwrap();
                            // Need size... take first numeric arg
                            Ok(TypeRef::Array(Box::new(ty), 0)) // size filled by typeck
                        }
                        _ => Ok(TypeRef::Named(other.into())),
                    }
                } else {
                    Ok(TypeRef::Named(other.into()))
                }
            }
        }
    }

    fn parse_type_args(&mut self) -> Result<Vec<TypeRef>, ParseError> {
        let mut args = vec![self.parse_type_ref()?];
        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some(',') {
                self.advance();
                args.push(self.parse_type_ref()?);
            } else {
                break;
            }
        }
        Ok(args)
    }

    fn parse_param_list(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some(',') { self.advance(); continue; }
            if !self.is_ident_start(self.peek()) { break; }

            let pname = self.parse_identifier()?;
            self.expect_char(':')?;
            let ptype = self.parse_type_ref()?;
            params.push(Param { name: pname, ty: ptype, annotations: HashMap::new() });

            self.skip_whitespace_and_comments();
            if self.peek() != Some(',') { break; }
            self.advance();
        }
        Ok(params)
    }

    fn parse_big_o(&mut self) -> Result<BigO, ParseError> {
        self.skip_whitespace_and_comments();
        // Expect O(...)
        let name = self.parse_identifier()?;
        if name != "O" {
            return Err(self.simple_error("Expected O(...) for complexity bound"));
        }
        self.expect_char('(')?;
        // Read the bound token verbatim up to ')': forms like `1`,
        // `n^2` and `2^n` are not identifiers, so parse_identifier
        // rejected them and the match arms below were unreachable.
        let mut inner = String::new();
        while let Some(c) = self.peek() {
            if c == ')' {
                break;
            }
            inner.push(c);
            self.advance();
        }
        let inner = inner.trim().to_string();
        if inner.is_empty() {
            return Err(self.simple_error("Expected complexity bound inside O(...)"));
        }
        self.expect_char(')')?;

        match inner.as_str() {
            "1" => Ok(BigO::Constant),
            "log" | "log_n" => Ok(BigO::Logarithmic),
            "n" => Ok(BigO::Linear),
            "n_log_n" | "nlogn" | "n_log" => Ok(BigO::Linearithmic),
            "n2" | "n_squared" | "n^2" => Ok(BigO::Quadratic),
            "n3" | "n_cubed" | "n^3" => Ok(BigO::Cubic),
            other => {
                if other.starts_with("n^") {
                    let exp: f64 = other[2..].parse().unwrap_or(2.0);
                    Ok(BigO::Polynomial(exp))
                } else if other.contains("^n") {
                    let base: f64 = other.replace("^n", "").parse().unwrap_or(2.0);
                    Ok(BigO::Exponential(base))
                } else {
                    Ok(BigO::Custom(other.into()))
                }
            }
        }
    }

    fn parse_test(&mut self) -> Result<Test, ParseError> {
        self.skip_whitespace_and_comments();
        let name = if self.peek() == Some('"') {
            Some(self.parse_string_literal_value()?)
        } else if self.peek() == Some('{') {
            None
        } else {
            let n = self.parse_identifier()?;
            self.expect_char(':')?;
            Some(n)
        };

        self.skip_whitespace_and_comments();
        self.expect_char('{')?;

        let mut input = None;
        let mut expected_output = None;
        let mut timeout_ms = None;
        let mut property = false;

        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') { self.advance(); break; }
            // Optional ';' field separator, as used in the
            // LANGUAGE_REFERENCE examples (section 2 and 3).
            if self.peek() == Some(';') { self.advance(); continue; }

            let key = self.parse_identifier()?;
            self.expect_char(':')?;

            match key.as_str() {
                "input" => { input = Some(self.parse_expr()?); }
                "expect" | "expected" | "output" => {
                    expected_output = Some(self.parse_expr()?);
                }
                "timeout" => { timeout_ms = Some(self.parse_integer()? as u64); }
                "property" => { property = true; }
                _ => { return self.error(format!("Unknown test field: {}", key)); }
            }
        }

        Ok(Test {
            name,
            input: input.ok_or_else(|| self.simple_error("Test missing 'input'"))?,
            expected_output: expected_output.ok_or_else(|| self.simple_error("Test missing 'expect'"))?,
            timeout_ms,
            property,
        })
    }

    fn parse_generics(&mut self) -> Result<Vec<GenericParam>, ParseError> {
        self.expect_char('<')?;
        let mut params = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('>') { self.advance(); break; }
            let name = self.parse_identifier()?;
            params.push(GenericParam { name, bounds: Vec::new() });
            self.skip_whitespace_and_comments();
            if self.peek() == Some(',') { self.advance(); }
        }
        Ok(params)
    }

    fn parse_type_def(&mut self) -> Result<TypeDef, ParseError> {
        let name = self.parse_identifier()?;
        let generics = if self.peek() == Some('<') {
            self.parse_generics()?
        } else {
            Vec::new()
        };

        self.skip_whitespace_and_comments();
        self.expect_char('=')?;
        self.skip_whitespace_and_comments();

        let kind = if self.peek() == Some('{') {
            // Record type
            self.advance();
            let mut fields = Vec::new();
            loop {
                self.skip_whitespace_and_comments();
                if self.peek() == Some('}') { self.advance(); break; }
                let fname = self.parse_identifier()?;
                self.expect_char(':')?;
                let ftype = self.parse_type_ref()?;
                fields.push((fname, ftype));
                self.skip_whitespace_and_comments();
                if self.peek() == Some(',') { self.advance(); }
            }
            TypeKind::Record { fields }
        } else {
            // Type alias
            let alias = self.parse_type_ref()?;
            self.expect_char(';')?;
            TypeKind::Alias(alias)
        };

        Ok(TypeDef { name, generics, kind })
    }

    fn parse_string_literal(&mut self) -> Result<String, ParseError> {
        self.parse_string_literal_value()
    }

    /// One char literal: `'a'`, `'\n'`, `'\''`, `'\\'`,
    /// `'\u{1F600}'`, or any single Unicode scalar written
    /// directly. The escape set mirrors the emitter's
    /// `escape_rust_char` world (src/codegen/mod.rs): quote,
    /// backslash, `\n` `\r` `\t`, and `\u{...}` for the
    /// characters the emitter writes that way. Malformed
    /// literals — empty, more than one character,
    /// unterminated, an unknown escape, or a `\u` value that
    /// is not a Unicode scalar — are parse errors.
    fn parse_char_literal_value(&mut self) -> Result<char, ParseError> {
        self.expect_char('\'')?;
        let c = match self.peek() {
            None => {
                return Err(self.simple_error("Unterminated char literal"));
            }
            Some('\'') => {
                return Err(self.simple_error("Empty char literal"));
            }
            Some('\\') => {
                self.advance();
                match self.peek() {
                    Some('n') => { self.advance(); '\n' }
                    Some('r') => { self.advance(); '\r' }
                    Some('t') => { self.advance(); '\t' }
                    Some('\\') => { self.advance(); '\\' }
                    Some('\'') => { self.advance(); '\'' }
                    Some('u') => {
                        self.advance();
                        self.expect_char('{')?;
                        let mut hex = String::new();
                        while self.peek() != Some('}') {
                            match self.peek() {
                                Some(h) if h.is_ascii_hexdigit() => {
                                    hex.push(h);
                                    self.advance();
                                }
                                _ => {
                                    return Err(self.simple_error(
                                        "Invalid \\u{...} escape in char literal",
                                    ));
                                }
                            }
                            if hex.len() > 6 {
                                return Err(self.simple_error(
                                    "Invalid \\u{...} escape in char literal",
                                ));
                            }
                        }
                        self.advance(); // consume '}'
                        if hex.is_empty() {
                            return Err(self.simple_error(
                                "Invalid \\u{...} escape in char literal",
                            ));
                        }
                        let value = u32::from_str_radix(&hex, 16).map_err(|_| {
                            self.simple_error("Invalid \\u{...} escape in char literal")
                        })?;
                        char::from_u32(value).ok_or_else(|| {
                            self.simple_error(
                                "\\u{...} value is not a Unicode scalar value",
                            )
                        })?
                    }
                    _ => {
                        return Err(self.simple_error("Unknown escape in char literal"));
                    }
                }
            }
            Some(c) => {
                self.advance();
                c
            }
        };
        // Exactly one character: the next character must be
        // the closing quote (a second character or EOF makes
        // the literal malformed).
        match self.peek() {
            Some('\'') => {
                self.advance();
                Ok(c)
            }
            _ => Err(self.simple_error(
                "Char literal must contain exactly one character",
            )),
        }
    }

    fn parse_string_literal_value(&mut self) -> Result<String, ParseError> {
        self.expect_char('"')?;
        let mut s = String::new();
        while !self.is_eof() && self.peek() != Some('"') {
            if self.peek() == Some('\\') {
                self.advance();
                match self.peek() {
                    Some('n') => { s.push('\n'); self.advance(); }
                    Some('t') => { s.push('\t'); self.advance(); }
                    Some('\\') => { s.push('\\'); self.advance(); }
                    Some('"') => { s.push('"'); self.advance(); }
                    _ => s.push('\\'),
                }
            } else {
                s.push(self.peek().unwrap());
                self.advance();
            }
        }
        self.expect_char('"')?;
        Ok(s)
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace_and_comments();
        if !self.is_ident_start(self.peek()) {
            return self.error(format!(
                "Expected identifier, found '{}'",
                self.peek().map(|c| c.to_string()).unwrap_or_else(|| "EOF".into())
            ));
        }
        let mut s = String::new();
        while self.is_ident_continue(self.peek()) {
            s.push(self.peek().unwrap());
            self.advance();
        }
        Ok(s)
    }

    fn parse_integer(&mut self) -> Result<i64, ParseError> {
        self.skip_whitespace_and_comments();
        let mut s = String::new();
        if self.peek() == Some('-') {
            s.push('-');
            self.advance();
        }
        while self.is_digit(self.peek()) {
            s.push(self.peek().unwrap());
            self.advance();
        }
        if s.is_empty() || s == "-" {
            return Err(self.simple_error("Expected integer literal"));
        }
        s.parse::<i64>()
            .map_err(|e: ParseIntError| self.simple_error(&format!("Invalid integer: {}", e)))
    }

    // ── Character-level operations ────────────────────────

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_ahead(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn peek_n(&self, n: usize) -> String {
        self.chars[self.pos..]
            .iter()
            .take(n)
            .collect::<String>()
    }

    fn advance(&mut self) {
        if let Some(c) = self.chars.get(self.pos) {
            if *c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        self.pos += 1;
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn is_digit(&self, c: Option<char>) -> bool {
        c.map(|c| c.is_ascii_digit()).unwrap_or(false)
    }

    fn is_ident_start(&self, c: Option<char>) -> bool {
        c.map(|c| c.is_alphabetic() || c == '_').unwrap_or(false)
    }

    fn is_ident_continue(&self, c: Option<char>) -> bool {
        c.map(|c| c.is_alphanumeric() || c == '_').unwrap_or(false)
    }

    fn expect_char(&mut self, expected: char) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        match self.peek() {
            Some(c) if c == expected => { self.advance(); Ok(()) }
            Some(c) => self.error(format!("Expected '{}', found '{}'", expected, c)),
            None => self.error(format!("Expected '{}', found EOF", expected)),
        }
    }

    fn eat_str(&mut self, s: &str) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        let actual: String = self.chars[self.pos..]
            .iter()
            .take(s.len())
            .collect();
        if actual == s {
            for _ in 0..s.len() { self.advance(); }
            Ok(())
        } else {
            self.error(format!("Expected '{}', found '{}'", s, actual))
        }
    }

    fn eat_ident(&mut self, ident: &str) {
        let actual: String = self.chars[self.pos..]
            .iter()
            .take(ident.len())
            .collect();
        if actual == ident {
            for _ in 0..ident.len() { self.advance(); }
        }
    }

    fn eat_keyword(&mut self, kw: &str) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        let actual: String = self.chars[self.pos..]
            .iter()
            .take(kw.len())
            .collect();
        if actual == kw {
            // Ensure not part of longer identifier
            let next = self.chars.get(self.pos + kw.len());
            if next.map_or(true, |c| !c.is_alphanumeric() && *c != '_') {
                for _ in 0..kw.len() { self.advance(); }
                return Ok(());
            }
        }
        self.error(format!("Expected keyword '{}'", kw))
    }

    fn eat_line(&mut self) {
        while !self.is_eof() && self.peek() != Some('\n') {
            self.advance();
        }
        if self.peek() == Some('\n') {
            self.advance();
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Skip whitespace
            while let Some(c) = self.peek() {
                if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
                    self.advance();
                } else {
                    break;
                }
            }
            // Skip line comments
            if self.peek() == Some('/') && self.peek_ahead() == Some('/') {
                self.eat_line();
            } else if self.peek() == Some('/') && self.peek_ahead() == Some('*') {
                // Skip block comments
                self.eat_str("/*").ok();
                while !self.is_eof() && self.peek_n(2) != "*/" {
                    self.advance();
                }
                self.eat_str("*/").ok();
            } else {
                break;
            }
        }
    }

    // ── Error helpers ───────────────────────────────────────

    fn simple_error(&self, msg: &str) -> ParseError {
        ParseError {
            message: msg.into(),
            line: self.line,
            column: self.col,
            snippet: self.context_snippet(),
        }
    }

    fn error<T>(&self, msg: String) -> Result<T, ParseError> {
        Err(ParseError {
            message: msg,
            line: self.line,
            column: self.col,
            snippet: self.context_snippet(),
        })
    }

    fn context_snippet(&self) -> String {
        // `pos` is a char index into `chars`, not a byte index into
        // `source`. Slicing `source` with it panics whenever a
        // multi-byte character straddles the window edge, so build
        // the snippet from `chars` instead.
        let start = self.pos.saturating_sub(20);
        let end = (self.pos + 20).min(self.chars.len());
        let snippet: String = self.chars[start..end].iter().collect();
        let marker = " ".repeat(self.pos - start) + "^";
        format!("{}\n{}", snippet, marker)
    }
}

// ── Tests ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_sort_spec() {
        let src = r#"
        TestModule {
        spec sort<T> {
            input: list: List<T>
            output: List<T>

            constraint: true
            constraint: true

            optimize: time < O(n_log_n)
            optimize: space < O(n)

            test {
                input: list
                expect: list
            }
        }
        }
        "#;

        let result = parse(src);
        assert!(result.is_ok(), "Parse error: {:?}", result.err());
        let spec = result.unwrap();
        assert_eq!(spec.name, "TestModule");
        assert_eq!(spec.functions.len(), 1);
        assert_eq!(spec.functions[0].name, "sort");
        assert_eq!(spec.functions[0].tests.len(), 1);
    }

    #[test]
    fn parse_requires_ensures() {
        let src = r#"
        TestMod {
        spec divide {
            input: a: Int, b: Int
            output: Int

            require: true
            ensure: true
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert_eq!(spec.functions[0].preconditions.len(), 1);
        assert_eq!(spec.functions[0].postconditions.len(), 1);
    }

    #[test]
    fn parse_complex_invariants() {
        let src = r#"
        TestMod {
        spec push<T> {
            input: stack: Stack<T>, item: T
            output: Stack<T>

            invariant: true
            invariant: true
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert_eq!(spec.functions[0].invariants.len(), 2);
    }

    #[test]
    fn error_snippet_survives_unicode_before_error() {
        // Regression: context_snippet() used the char index `pos` as a
        // byte index into the source. With multi-byte characters (the
        // box-drawing comment art in the bundled examples) before the
        // error position, building the error snippet panicked with
        // "byte index is not a char boundary" instead of returning
        // the parse error.
        let dashes = "─".repeat(40);
        let src = format!(
            "// {}\nTestModule {{\n  spec f {{\n    input: x: Int\n    output: Int\n    bogus: true\n  }}\n}}\n",
            dashes
        );
        let result = parse(&src);
        assert!(result.is_err(), "expected a parse error for 'bogus'");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("bogus"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn parse_constant_big_o() {
        // `O(1)` is documented (LANGUAGE_REFERENCE section 7) and
        // used by is_sorted/count_matching/binary_search, but
        // parse_big_o read the bound with parse_identifier, which
        // rejects digits — the `BigO::Constant` match arm was
        // unreachable and every `O(1)` failed to parse.
        let src = r#"
        TestMod {
        spec f {
            input: list: List<Int>
            output: Int

            constraint: true
            optimize: space < O(1)
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let complexity = spec.functions[0]
            .complexity
            .as_ref()
            .expect("complexity bound");
        assert!(
            matches!(complexity.bound, BigO::Constant),
            "expected O(1), got: {:?}",
            complexity.bound
        );
    }

    #[test]
    fn parse_test_with_semicolon_separators() {
        // The reference's own examples write test fields separated by
        // ';' (section 2: `test { input: (2, 3); expect: 5 }`), and
        // count_matching.morph does the same, but parse_test rejected
        // the separator. Accept it as optional.
        let src = r#"
        TestMod {
        spec f {
            input: x: Int
            output: Int

            test "one" { input: 1; expect: 1 }
            test "two" {
                input: 2;
                expect: 2
            }
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert_eq!(spec.functions[0].tests.len(), 2);
    }

    #[test]
    fn parse_resource_bounds() {
        // Documented forms (LANGUAGE_REFERENCE section 8):
        // `resource: memory < 10MB` and `resource: allocations < O(n)`.
        // The parser expected neither the ':' after `resource`, nor a
        // unit suffix, nor a Big-O amount, so both forms failed.
        let src = r#"
        TestMod {
        spec f {
            input: list: List<Int>
            output: List<Int>

            constraint: true
            resource: memory < 10MB
        }

        spec g {
            input: list: List<Int>
            output: List<Int>

            constraint: true
            resource: allocations < O(n)
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let mem = spec.functions[0]
            .resource
            .as_ref()
            .expect("memory resource bound");
        assert_eq!(mem.max_amount, Some(10 * 1024 * 1024));
        assert_eq!(mem.predicate, None);

        let alloc = spec.functions[1]
            .resource
            .as_ref()
            .expect("allocations resource bound");
        assert_eq!(alloc.max_amount, None);
        assert_eq!(alloc.predicate.as_deref(), Some("O(n)"));
    }

    #[test]
    fn parse_function_type_param() {
        // Function types are documented (LANGUAGE_REFERENCE section 4:
        // `fn(T1, T2) -> R`) and TypeRef::Function exists, but
        // parse_type_ref treated `fn` as a plain named type and then
        // choked on the '('. Blocks sort/binary_search, whose specs
        // take a comparator parameter.
        let src = r#"
        TestMod {
        spec f {
            input: list: List<Int>, cmp: fn(Int, Int) -> Bool
            output: List<Int>

            constraint: true
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let params = &spec.functions[0].params;
        assert_eq!(params.len(), 2);
        match &params[1].ty {
            TypeRef::Function(args, ret) => {
                assert_eq!(args.len(), 2);
                assert!(matches!(**ret, TypeRef::Bool));
            }
            other => panic!("expected Function type, got: {:?}", other),
        }
    }

    #[test]
    fn parse_lambda_with_and_without_arrow() {
        // LANGUAGE_REFERENCE section 11 documents both lambda forms:
        // `|x, y| x + y` and `|a, b| -> a > b`. The parser required
        // the arrow, so the arrow-less form used by the bundled
        // examples (sort, binary_search) failed to parse.
        let src = r#"
        TestMod {
        spec f {
            input: list: List<Int>
            output: List<Int>

            test "no arrow" {
                input: ([3, 1, 2], |a, b| a < b)
                expect: [1, 2, 3]
            }

            test "with arrow" {
                input: ([3, 1, 2], |a, b| -> a < b)
                expect: [1, 2, 3]
            }
        }
        }
        "#;

        let spec = parse(src).unwrap();
        assert_eq!(spec.functions[0].tests.len(), 2);
    }

    #[test]
    fn parse_implies_constraint() {
        // `a implies b` is documented (LANGUAGE_REFERENCE section 6)
        // and Constraint::Implies is handled by the type checker, the
        // Z3 verifier and the synthesis engine — but the parser never
        // produced it, so any spec using it failed to parse.
        let src = r#"
        TestMod {
        spec f {
            input: list: List<Int>
            output: Bool

            constraint: is_sorted(list) implies output == true
            constraint: forall i: Int in range(0, len(list)):
                list[i] > 0 implies output == true
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let func = &spec.functions[0];
        assert_eq!(func.postconditions.len(), 2);
        assert!(
            matches!(func.postconditions[0], Constraint::Implies(_, _)),
            "expected Implies, got: {:?}",
            func.postconditions[0]
        );
        match &func.postconditions[1] {
            Constraint::Forall { body, .. } => assert!(
                matches!(**body, Constraint::Implies(_, _)),
                "expected Implies forall body, got: {:?}",
                body
            ),
            other => panic!("expected Forall, got: {:?}", other),
        }
    }

    #[test]
    fn parse_spaced_binary_operators() {
        // The reference writes operators with spaces (section 6:
        // `x > 0 && y > 0`), but the expression operator loops never
        // skipped whitespace before peeking, so a spaced operator only
        // parsed when the left operand happened to be an identifier
        // (whose atom consumes trailing whitespace). Literal operands
        // and spaced && / || / arithmetic failed.
        let src = r#"
        TestMod {
        spec f {
            input: x: Int, y: Int
            output: Int

            constraint: x > 0 && y > 0
            constraint: output == len(list) - 1
            constraint: 1 + 2 == 3
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let func = &spec.functions[0];
        assert_eq!(func.postconditions.len(), 3);
        assert!(
            matches!(
                func.postconditions[0],
                Constraint::Expr(Expr::BinOp(BinOp::And, _, _))
            ),
            "expected And constraint, got: {:?}",
            func.postconditions[0]
        );
        assert!(
            matches!(
                func.postconditions[1],
                Constraint::Expr(Expr::BinOp(
                    BinOp::Eq,
                    _,
                    _
                )) | Constraint::Eq(_, _)
            ),
            "expected equality constraint, got: {:?}",
            func.postconditions[1]
        );
    }

    #[test]
    fn parse_not_equal_operator() {
        // '!=' is documented (LANGUAGE_REFERENCE section 6:
        // `require: b != 0`) and parse_expr_compare has a "!=" arm,
        // but its guard only entered on '<', '>' or '=', so the
        // operator was unreachable and the parse failed on '!'.
        let src = r#"
        TestMod {
        spec divide {
            input: a: Int, b: Int
            output: Int

            require: b != 0
            constraint: output != a
        }
        }
        "#;

        let spec = parse(src).unwrap();
        let func = &spec.functions[0];
        assert_eq!(func.preconditions.len(), 1);
        assert!(
            matches!(
                func.preconditions[0],
                Constraint::Expr(Expr::BinOp(BinOp::Neq, _, _))
            ),
            "expected Neq constraint, got: {:?}",
            func.preconditions[0]
        );
        assert_eq!(func.postconditions.len(), 1);
        assert!(
            matches!(
                func.postconditions[0],
                Constraint::Expr(Expr::BinOp(BinOp::Neq, _, _))
            ),
            "expected Neq constraint, got: {:?}",
            func.postconditions[0]
        );
    }

    #[test]
    fn truncated_spec_body_is_rejected() {
        // Regression: the spec-body loop ends on EOF, so a file cut off
        // inside a spec (both closing braces missing) parsed as Ok.
        let src = "TestModule {\n  spec f {\n    input: x: Int\n";
        let result = parse(src);
        assert!(
            result.is_err(),
            "truncated spec must be a parse error, got: {:?}",
            result.map(|s| s.name)
        );
    }

    #[test]
    fn truncated_module_is_rejected() {
        // Regression: same EOF hole one level up — the spec is complete
        // but the module's closing brace is missing and the file ends
        // exactly on a token boundary.
        let src = "TestModule {\n  spec f {\n    input: x: Int\n    output: Int\n    constraint: true\n  }";
        let result = parse(src);
        assert!(
            result.is_err(),
            "unterminated module must be a parse error, got: {:?}",
            result.map(|s| s.name)
        );
    }

    #[test]
    fn error_snippet_survives_unicode_after_error() {
        // Same regression, opposite direction: the error is at the very
        // start of the file and the multi-byte characters fall inside
        // the forward-looking half of the snippet window.
        let src = format!("{} garbage", "─".repeat(10));
        let result = parse(&src);
        assert!(result.is_err(), "expected a parse error");
    }

    // ── String concatenation `++` (Phase 25, finding F1) ─────
    //
    // BinOp::Concat has existed in the AST, the IR interpreter
    // and the Z3 verifier all along, but the parser never
    // produced it: `+` mapped to BinOp::Add and a second `+`
    // was a parse error, so no spec could state a string
    // assembly. These tests pin the surface syntax: `++` at
    // the additive level, left-associative.

    /// Every binary operator occurring in an expression tree.
    fn expr_ops(expr: &Expr, ops: &mut Vec<BinOp>) {
        match expr {
            Expr::BinOp(op, l, r) => {
                ops.push(*op);
                expr_ops(l, ops);
                expr_ops(r, ops);
            }
            Expr::UnaryOp(_, inner) => expr_ops(inner, ops),
            Expr::Call(_, args) => {
                for a in args {
                    expr_ops(a, ops);
                }
            }
            Expr::Field(base, _) => expr_ops(base, ops),
            Expr::Index(base, idx) => {
                expr_ops(base, ops);
                expr_ops(idx, ops);
            }
            _ => {}
        }
    }

    fn constraint_ops(c: &Constraint, ops: &mut Vec<BinOp>) {
        match c {
            Constraint::Eq(l, r) | Constraint::Order(_, l, r) => {
                expr_ops(l, ops);
                expr_ops(r, ops);
            }
            Constraint::Expr(e) => expr_ops(e, ops),
            Constraint::And(parts) | Constraint::Or(parts) => {
                for p in parts {
                    constraint_ops(p, ops);
                }
            }
            Constraint::Not(inner) => constraint_ops(inner, ops),
            Constraint::Implies(l, r) => {
                constraint_ops(l, ops);
                constraint_ops(r, ops);
            }
            Constraint::Predicate(_, args) => {
                for a in args {
                    expr_ops(a, ops);
                }
            }
            Constraint::Forall { body, .. } | Constraint::Exists { body, .. } => {
                constraint_ops(body, ops)
            }
            Constraint::True => {}
        }
    }

    /// The right-hand side of a spec's first postcondition
    /// (`output == <rhs>`), whichever constraint shape the
    /// parser produced for it.
    fn postcondition_rhs(spec: &Spec, func: usize) -> &Expr {
        match &spec.functions[func].postconditions[0] {
            Constraint::Eq(_, r) => r,
            Constraint::Expr(Expr::BinOp(BinOp::Eq, _, r)) => r,
            other => panic!("postcondition is not an output equation: {:?}", other),
        }
    }

    #[test]
    fn parse_concat_produces_concat_not_add() {
        // The wrap-one template shape from M3: the postcondition
        // assembles a string around a variable part. `++` must
        // lower to BinOp::Concat — the operator the layers below
        // implement — never to BinOp::Add (rejected on strings
        // by the interpreter, unencodable for the verifier).
        let src = r#"
        T {
        spec wrap {
            input: inner: String
            output: String
            constraint: output == "Vec<" ++ inner ++ ">"
            constraint: len("x" ++ inner) > 0
        }
        }
        "#;
        let spec = parse(src).expect("concat constraints must parse");
        let func = &spec.functions[0];
        assert_eq!(func.postconditions.len(), 2);
        for post in &func.postconditions {
            let mut ops = Vec::new();
            constraint_ops(post, &mut ops);
            assert!(
                ops.contains(&BinOp::Concat),
                "no Concat in {:?}",
                post
            );
            assert!(
                !ops.contains(&BinOp::Add),
                "concat lowered to Add: {:?}",
                post
            );
        }
    }

    #[test]
    fn parse_concat_is_left_associative() {
        // `"a" ++ b ++ "c"` groups as `("a" ++ b) ++ "c"`: the
        // top-level Concat's LEFT child is itself a Concat.
        let src = r#"
        T {
        spec f {
            input: b: String
            output: String
            constraint: output == "a" ++ b ++ "c"
        }
        }
        "#;
        let spec = parse(src).unwrap();
        match postcondition_rhs(&spec, 0) {
            Expr::BinOp(BinOp::Concat, left, _) => {
                assert!(
                    matches!(**left, Expr::BinOp(BinOp::Concat, ..)),
                    "concat is not left-associative: {:?}",
                    left
                );
            }
            other => panic!("rhs is not a Concat: {:?}", other),
        }
    }

    #[test]
    fn parse_concat_shares_the_additive_level() {
        // `++` sits alongside `+`/`-`, not above or below them:
        // in `x + y ++ z` the operators group strictly
        // left-to-right, ((x + y) ++ z), exactly as `x + y - z`
        // groups today. And concat binds tighter than `==`:
        // the equation's RHS is the whole assembly (asserted by
        // postcondition_rhs extracting it).
        let src = r#"
        T {
        spec g {
            input: x: String, y: String, z: String
            output: String
            constraint: output == x + y ++ z
        }
        }
        "#;
        let spec = parse(src).unwrap();
        match postcondition_rhs(&spec, 0) {
            Expr::BinOp(BinOp::Concat, left, _) => {
                assert!(
                    matches!(**left, Expr::BinOp(BinOp::Add, ..)),
                    "expected ((x + y) ++ z), got left = {:?}",
                    left
                );
            }
            other => panic!("rhs is not a Concat over an Add: {:?}", other),
        }
    }

    #[test]
    fn parse_plus_still_produces_add() {
        // `+` is unchanged: a single plus is BinOp::Add, and no
        // Concat appears. (Before `++` existed, `1 + 2` was the
        // only reading; it must stay the only reading.)
        let src = r#"
        T {
        spec h {
            input: a: Int, b: Int
            output: Int
            constraint: output == a + b
        }
        }
        "#;
        let spec = parse(src).unwrap();
        let mut ops = Vec::new();
        constraint_ops(&spec.functions[0].postconditions[0], &mut ops);
        assert!(ops.contains(&BinOp::Add), "no Add in {:?}", ops);
        assert!(!ops.contains(&BinOp::Concat), "unexpected Concat in {:?}", ops);
    }

    // ── Char literals (Phase 45, finding F4) ────────────────
    //
    // The layers below the parser have always known Char:
    // TypeRef::Char, IRLiteral::Char, Value::Char, the
    // emitter's Char arm. But parse_expr_atom had no `'` arm,
    // so a char literal in an expression, a constraint or a
    // test was a parse error — and the tokenizer module's
    // Char classifiers (whose constraints ARE char-literal
    // equalities) could not be written at all. These tests
    // pin the surface syntax.

    /// Every char literal occurring in an expression tree,
    /// in traversal order.
    fn expr_char_literals(expr: &Expr, out: &mut Vec<char>) {
        match expr {
            Expr::CharLit(c) => out.push(*c),
            Expr::BinOp(_, l, r) => {
                expr_char_literals(l, out);
                expr_char_literals(r, out);
            }
            Expr::UnaryOp(_, inner) => expr_char_literals(inner, out),
            Expr::Call(_, args) => {
                for a in args {
                    expr_char_literals(a, out);
                }
            }
            Expr::Field(base, _) => expr_char_literals(base, out),
            Expr::Index(base, idx) => {
                expr_char_literals(base, out);
                expr_char_literals(idx, out);
            }
            Expr::Length(inner) => expr_char_literals(inner, out),
            _ => {}
        }
    }

    fn constraint_char_literals(c: &Constraint, out: &mut Vec<char>) {
        match c {
            Constraint::Eq(l, r) | Constraint::Order(_, l, r) => {
                expr_char_literals(l, out);
                expr_char_literals(r, out);
            }
            Constraint::Expr(e) => expr_char_literals(e, out),
            Constraint::And(parts) | Constraint::Or(parts) => {
                for p in parts {
                    constraint_char_literals(p, out);
                }
            }
            Constraint::Not(inner) => constraint_char_literals(inner, out),
            Constraint::Implies(l, r) => {
                constraint_char_literals(l, out);
                constraint_char_literals(r, out);
            }
            Constraint::Predicate(_, args) => {
                for a in args {
                    expr_char_literals(a, out);
                }
            }
            Constraint::Forall { body, .. } | Constraint::Exists { body, .. } => {
                constraint_char_literals(body, out)
            }
            Constraint::True => {}
        }
    }

    #[test]
    fn parse_char_literal_in_constraint_and_tests() {
        // The is_whitespace shape from functional_tokenizer:
        // the constraint is a disjunction of char-literal
        // equalities, and the tests name char values directly.
        let src = r#"
        T {
        spec is_whitespace {
            input: c: Char
            output: Bool
            constraint: output == (c == ' ' || c == '\t' || c == '\n' || c == '\r')
            test "space" { input: ' '; expect: true }
            test "letter" { input: 'a'; expect: false }
            test "newline" { input: '\n'; expect: true }
        }
        }
        "#;
        let spec = parse(src).expect("char literals must parse");
        let func = &spec.functions[0];
        let mut chars = Vec::new();
        for post in &func.postconditions {
            constraint_char_literals(post, &mut chars);
        }
        assert_eq!(chars, vec![' ', '\t', '\n', '\r']);
        assert!(matches!(func.tests[0].input, Expr::CharLit(' ')));
        assert!(matches!(func.tests[1].input, Expr::CharLit('a')));
        assert!(matches!(func.tests[2].input, Expr::CharLit('\n')));
    }

    #[test]
    fn parse_char_literal_escapes_and_unicode() {
        // The escape set mirrors escape_rust_char's world
        // (src/codegen/mod.rs): quote, backslash, \n \r \t,
        // and \u{...} for the control characters the emitter
        // writes that way. A non-ASCII char is one scalar.
        let src = r#"
        T {
        spec f {
            input: c: Char
            output: Bool
            constraint: output == (c == '\'' || c == '\\' || c == 'é' || c == '\u{1F600}' || c == 'A')
            test "quote" { input: '\''; expect: true }
        }
        }
        "#;
        let spec = parse(src).expect("char escapes must parse");
        let func = &spec.functions[0];
        let mut chars = Vec::new();
        for post in &func.postconditions {
            constraint_char_literals(post, &mut chars);
        }
        assert_eq!(chars, vec!['\'', '\\', 'é', '😀', 'A']);
        assert!(matches!(func.tests[0].input, Expr::CharLit('\'')));
    }

    #[test]
    fn parse_tokenizer_classifier_constraint_shapes() {
        // The three Char classifiers of
        // examples/functional_tokenizer.morph, verbatim
        // constraint shapes (the file as a whole still fails
        // its earlier dialect layers — module wrapper, alias
        // `;`, if-constraints — which are S4's re-authoring
        // work; what F4 owns is that these constraints now
        // parse in isolation), plus next_char's `'\n'` inside
        // the Some(...) call its rule compares against.
        let src = r#"
        T {
        spec is_whitespace {
            input: c: Char
            output: Bool
            constraint: output == (c == ' ' || c == '\t' || c == '\n' || c == '\r')
        }
        spec is_ident_start {
            input: c: Char
            output: Bool
            constraint: output == (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || c == '_'
        }
        spec is_ident_continue {
            input: c: Char
            output: Bool
            constraint: output == is_ident_start(c) || (c >= '0' && c <= '9')
        }
        spec next_char_peek {
            input: c: Char
            output: Bool
            constraint: output == (Some(c) == Some('\n'))
        }
        }
        "#;
        let spec = parse(src).expect("classifier constraint shapes must parse");
        assert_eq!(spec.functions.len(), 4);
        let mut chars = Vec::new();
        for func in &spec.functions {
            for post in &func.postconditions {
                constraint_char_literals(post, &mut chars);
            }
        }
        assert_eq!(
            chars,
            vec![' ', '\t', '\n', '\r', 'a', 'z', 'A', 'Z', '_', '0', '9', '\n']
        );
    }

    #[test]
    fn parse_char_literal_rejects_malformed() {
        // Empty, multi-char, unterminated, unknown escapes
        // and non-scalar \u values are parse errors, never a
        // guessed character.
        for lit in [
            "''",
            "'ab'",
            "'a",
            "'\\x'",
            "'\\u{}'",
            "'\\u{110000}'",
            "'\\u{D800}'",
            "'\\u{FFFFFF}'",
        ] {
            let src = format!(
                "T {{\n spec f {{\n input: c: Char\n output: Bool\n test \"t\" {{ input: {}; expect: true }}\n }}\n }}",
                lit
            );
            let result = parse(&src);
            assert!(
                result.is_err(),
                "malformed char literal {} must be a parse error, got: {:?}",
                lit,
                result.map(|s| s.name)
            );
        }
    }

    // ── Method calls + record literals (Phase 73, M1 features) ──
    //
    // The tokenizer's dropped constraints and mirror-only tests
    // (M1_FEATURES_AUDIT sections 1.1 and 3.1, recovered verbatim
    // from commit acda3f9) wait on exactly two expression forms
    // the parser could not produce: method calls (there was no
    // MethodCall AST variant — `output.is_err()` parsed `.is_err`
    // as a field and stranded on the `()`) and record literals
    // (no `{` arm in expression position). Ratified designs:
    // D1a (native MethodCall node, closed builtin table — the
    // table itself is Phase 74) and D3a (named construction
    // `Type { field: value, ... }`). These tests pin the surface
    // syntax; the record literals below are the acda3f9 tests
    // re-authored into the D3a named form (the FORM change D3a
    // prices; the bare `{ ... }` form stays a parse error).

    /// Every method-call node in an expression tree, in
    /// traversal order, as (receiver description, name) — the
    /// receiver description is the variant tag of the receiver
    /// expression, enough to pin the chain shapes.
    fn expr_method_calls(expr: &Expr, out: &mut Vec<String>) {
        match expr {
            Expr::MethodCall { receiver, name, args } => {
                expr_method_calls(receiver, out);
                out.push(name.clone());
                for a in args {
                    expr_method_calls(a, out);
                }
            }
            Expr::BinOp(_, l, r) => {
                expr_method_calls(l, out);
                expr_method_calls(r, out);
            }
            Expr::UnaryOp(_, inner) => expr_method_calls(inner, out),
            Expr::Call(_, args) => {
                for a in args {
                    expr_method_calls(a, out);
                }
            }
            Expr::Field(base, _) => expr_method_calls(base, out),
            Expr::Index(base, idx) => {
                expr_method_calls(base, out);
                expr_method_calls(idx, out);
            }
            Expr::Length(inner) => expr_method_calls(inner, out),
            Expr::RecordLit { fields, .. } => {
                for (_, v) in fields {
                    expr_method_calls(v, out);
                }
            }
            _ => {}
        }
    }

    fn constraint_method_calls(c: &Constraint, out: &mut Vec<String>) {
        match c {
            Constraint::Eq(l, r) | Constraint::Order(_, l, r) => {
                expr_method_calls(l, out);
                expr_method_calls(r, out);
            }
            Constraint::Expr(e) => expr_method_calls(e, out),
            Constraint::And(parts) | Constraint::Or(parts) => {
                for p in parts {
                    constraint_method_calls(p, out);
                }
            }
            Constraint::Not(inner) => constraint_method_calls(inner, out),
            Constraint::Implies(l, r) => {
                constraint_method_calls(l, out);
                constraint_method_calls(r, out);
            }
            Constraint::Predicate(_, args) => {
                for a in args {
                    expr_method_calls(a, out);
                }
            }
            Constraint::Forall { body, .. } | Constraint::Exists { body, .. } => {
                constraint_method_calls(body, out)
            }
            Constraint::True => {}
        }
    }

    #[test]
    fn parse_method_call_constraints_tokenize_one() {
        // The two dropped tokenize_one constraints, verbatim
        // from acda3f9 (audit section 1.1). Before Phase 73 the
        // first stranded the parser on the `()` of `is_err()`.
        let src = r#"
        T {
        type TokenizerState = {
            source: String,
            pos: Int,
            line: Int,
            col: Int,
            tokens: List<Token>
        }
        type Token = {
            kind: Int,
            lexeme: String,
            line: Int,
            col: Int
        }
        type StepResult = Result<TokenizerState, String>;
        spec tokenize_one {
            input: state: TokenizerState
            output: StepResult
            constraint: output.is_err() || len(output.unwrap().tokens) == len(state.tokens) + 1
            constraint: output.is_ok() || state.pos >= len(state.source)
        }
        }
        "#;
        let spec = parse(src).expect("method-call constraints must parse");
        let func = &spec.functions[0];
        assert_eq!(func.postconditions.len(), 2);
        let mut calls = Vec::new();
        for post in &func.postconditions {
            constraint_method_calls(post, &mut calls);
        }
        assert_eq!(calls, vec!["is_err", "unwrap", "is_ok"]);

        // The chain shape, pinned structurally: the `.tokens`
        // field access sits ON the unwrap() method call, and the
        // whole chain is the argument of the free function len.
        let Constraint::Expr(Expr::BinOp(BinOp::Or, lhs, rhs)) = &func.postconditions[0]
        else {
            panic!("constraint 0 must be a disjunction, got {:?}", func.postconditions[0]);
        };
        assert!(
            matches!(&**lhs, Expr::MethodCall { name, .. } if name == "is_err"),
            "left disjunct must be output.is_err(), got {:?}", lhs
        );
        let Expr::BinOp(BinOp::Eq, eq_l, _) = &**rhs else {
            panic!("right disjunct must be an equality, got {:?}", rhs);
        };
        let Expr::Call(len_name, len_args) = &**eq_l else {
            panic!("equality left side must be a len(...) call, got {:?}", eq_l);
        };
        assert_eq!(len_name, "len");
        assert_eq!(len_args.len(), 1);
        let Expr::Field(field_base, field_name) = &len_args[0] else {
            panic!("len argument must be a field access, got {:?}", len_args[0]);
        };
        assert_eq!(field_name, "tokens");
        assert!(
            matches!(&**field_base, Expr::MethodCall { name, receiver, .. }
                if name == "unwrap"
                    && matches!(&**receiver, Expr::Var(v) if v == "output")),
            "field base must be output.unwrap(), got {:?}",
            field_base
        );
    }

    #[test]
    fn parse_method_call_on_unwrapped_list_tokenize_all() {
        // tokenize_all's dropped constraint (acda3f9): a method
        // call (.len()) chained on another method call
        // (output.unwrap()) — postfix composition after a call
        // result, the shape the old parser could not produce at
        // all (nothing after a call result could postfix).
        let src = r#"
        T {
        spec tokenize_all {
            input: state: TokenizerState
            output: Result<List<Token>, String>
            constraint: output.is_err() || output.unwrap().len() >= len(state.tokens)
        }
        }
        "#;
        let spec = parse(src).expect("chained method calls must parse");
        let func = &spec.functions[0];
        let Constraint::Expr(Expr::BinOp(BinOp::Or, _, rhs)) = &func.postconditions[0]
        else {
            panic!("constraint must be a disjunction, got {:?}", func.postconditions[0]);
        };
        let Expr::BinOp(BinOp::Ge, ge_l, _) = &**rhs else {
            panic!("right disjunct must be a >= comparison, got {:?}", rhs);
        };
        assert!(
            matches!(&**ge_l, Expr::MethodCall { name, receiver, .. }
                if name == "len"
                    && matches!(&**receiver, Expr::MethodCall { name: inner, .. } if inner == "unwrap")),
            "left side must be output.unwrap().len(), got {:?}",
            ge_l
        );
    }

    #[test]
    fn parse_postfix_chains_after_calls_and_parens() {
        // Postfix composition is uniform after Phase 73: field,
        // index and method postfixes chain after call results
        // and parenthesized expressions, not only after bare
        // variables (the pre-73 postfix loop ran only on the
        // variable path).
        let src = r#"
        T {
        spec f {
            input: state: TokenizerState
            output: Int
            constraint: output == peek(state).len() + (state.tokens)[0].kind + make(state).pos
        }
        }
        "#;
        let spec = parse(src).expect("postfix chains after calls/parens must parse");
        let func = &spec.functions[0];
        let Constraint::Expr(Expr::BinOp(BinOp::Eq, _, rhs)) = &func.postconditions[0]
        else {
            panic!("constraint must be an equality, got {:?}", func.postconditions[0]);
        };
        // rhs: (peek(state).len() + (state.tokens)[0].kind) + make(state).pos
        let Expr::BinOp(BinOp::Add, add_l, add_r) = &**rhs else {
            panic!("rhs must be an addition chain, got {:?}", rhs);
        };
        assert!(
            matches!(&**add_r, Expr::Field(base, f) if f == "pos"
                && matches!(&**base, Expr::Call(n, _) if n == "make")),
            "last term must be make(state).pos, got {:?}",
            add_r
        );
        let Expr::BinOp(BinOp::Add, mid_l, mid_r) = &**add_l else {
            panic!("left of the outer + must be an addition, got {:?}", add_l);
        };
        assert!(
            matches!(&**mid_l, Expr::MethodCall { name, receiver, .. } if name == "len"
                && matches!(&**receiver, Expr::Call(n, _) if n == "peek")),
            "first term must be peek(state).len(), got {:?}",
            mid_l
        );
        assert!(
            matches!(&**mid_r, Expr::Field(base, f) if f == "kind"
                && matches!(&**base, Expr::Index(..))),
            "middle term must be (state.tokens)[0].kind, got {:?}",
            mid_r
        );
    }

    #[test]
    fn parse_record_literals_named_form() {
        // The 9 mirror-only tests of acda3f9, re-authored into
        // the D3a named form (three of them here — the shapes
        // are the full set: state literal as input, state
        // literal inside Ok(...) as expected output, Token
        // literals nested in a list inside a state literal, and
        // Token literals in a bare list inside Ok(...)).
        let src = r#"
        T {
        type Token = {
            kind: Int,
            lexeme: String,
            line: Int,
            col: Int
        }
        type TokenizerState = {
            source: String,
            pos: Int,
            line: Int,
            col: Int,
            tokens: List<Token>
        }
        type StepResult = Result<TokenizerState, String>;
        spec peek {
            input: state: TokenizerState
            output: Option<Char>
            test "non-empty" { input: TokenizerState { source: "hi", pos: 0, line: 1, col: 1, tokens: [] }; expect: Some('h') }
        }
        spec tokenize_one {
            input: state: TokenizerState
            output: StepResult
            test "keyword-spec" {
                input: TokenizerState { source: "spec", pos: 0, line: 1, col: 1, tokens: [] };
                expect: Ok(TokenizerState { source: "spec", pos: 4, line: 1, col: 5, tokens: [
                    Token { kind: 0, lexeme: "spec", line: 1, col: 1 }
                ] })
            }
        }
        spec tokenize_all {
            input: state: TokenizerState
            output: Result<List<Token>, String>
            test "simple-spec" {
                input: TokenizerState { source: "spec sort", pos: 0, line: 1, col: 1, tokens: [] };
                expect: Ok([
                    Token { kind: 0, lexeme: "spec", line: 1, col: 1 },
                    Token { kind: 0, lexeme: "sort", line: 1, col: 6 }
                ])
            }
        }
        }
        "#;
        let spec = parse(src).expect("named record literals must parse");

        // peek's input: the named state literal, fields in
        // written order, the empty list as the parser's List
        // pseudo-call.
        let peek_test = &spec.functions[0].tests[0];
        let Expr::RecordLit { type_name, fields } = &peek_test.input else {
            panic!("peek input must be a record literal, got {:?}", peek_test.input);
        };
        assert_eq!(type_name, "TokenizerState");
        let field_names: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(field_names, vec!["source", "pos", "line", "col", "tokens"]);
        assert!(matches!(&fields[0].1, Expr::StringLit(s) if s == "hi"));
        assert!(matches!(&fields[4].1, Expr::Call(n, args) if n == "List" && args.is_empty()));

        // tokenize_one's expected output: Ok(TokenizerState {
        // ..., tokens: [Token { ... }] }) — the nested Token
        // literal inside the list inside the state literal.
        let one_test = &spec.functions[1].tests[0];
        let Expr::Call(ok_name, ok_args) = &one_test.expected_output else {
            panic!("expected output must be an Ok(...) call, got {:?}", one_test.expected_output);
        };
        assert_eq!(ok_name, "Ok");
        let Expr::RecordLit { type_name, fields } = &ok_args[0] else {
            panic!("Ok argument must be a record literal, got {:?}", ok_args[0]);
        };
        assert_eq!(type_name, "TokenizerState");
        let Expr::Call(list_name, token_lits) = &fields[4].1 else {
            panic!("tokens field must be a list literal, got {:?}", fields[4].1);
        };
        assert_eq!(list_name, "List");
        assert_eq!(token_lits.len(), 1);
        let Expr::RecordLit { type_name, fields } = &token_lits[0] else {
            panic!("list element must be a Token record literal, got {:?}", token_lits[0]);
        };
        assert_eq!(type_name, "Token");
        assert!(matches!(&fields[1].1, Expr::StringLit(s) if s == "spec"));

        // tokenize_all's expected output: Ok([Token, Token]).
        let all_test = &spec.functions[2].tests[0];
        let Expr::Call(_, ok_args) = &all_test.expected_output else {
            panic!("expected output must be an Ok(...) call, got {:?}", all_test.expected_output);
        };
        let Expr::Call(_, token_lits) = &ok_args[0] else {
            panic!("Ok argument must be a list literal, got {:?}", ok_args[0]);
        };
        assert_eq!(token_lits.len(), 2);
        assert!(
            matches!(&token_lits[1], Expr::RecordLit { type_name, fields }
                if type_name == "Token"
                    && matches!(&fields[3].1, Expr::IntLit(6))),
            "second token must stamp col 6 (START, D4a), got {:?}",
            token_lits[1]
        );
    }

    #[test]
    fn parse_record_literal_in_constraint_position() {
        // Record literals are a general expression form, not
        // test-only sugar (audit section 3.1: synthesized bodies
        // must construct records — next_char's output equation
        // is the shape).
        let src = r#"
        T {
        spec next_char {
            input: state: TokenizerState
            output: TokenizerState
            constraint: output == TokenizerState { source: "ab", pos: 1, line: 1, col: 2, tokens: [] }
        }
        }
        "#;
        let spec = parse(src).expect("record literal in a constraint must parse");
        let func = &spec.functions[0];
        let Constraint::Expr(Expr::BinOp(BinOp::Eq, _, rhs)) = &func.postconditions[0]
        else {
            panic!("constraint must be an equality, got {:?}", func.postconditions[0]);
        };
        assert!(
            matches!(&**rhs, Expr::RecordLit { type_name, .. } if type_name == "TokenizerState"),
            "rhs must be a TokenizerState record literal, got {:?}",
            rhs
        );
    }

    #[test]
    fn parse_bare_record_literal_still_rejected() {
        // D3a ratified the NAMED form only. The bare
        // `{ field: value }` form of acda3f9 (design D3b) was
        // NOT ratified: a leading `{` in expression position
        // stays a parse error, never a guessed record type.
        let src = r#"
        T {
        spec peek {
            input: state: TokenizerState
            output: Option<Char>
            test "t" { input: { source: "hi", pos: 0, line: 1, col: 1, tokens: [] }; expect: Some('h') }
        }
        }
        "#;
        let result = parse(src);
        assert!(
            result.is_err(),
            "bare record literal must stay a parse error under D3a, got: {:?}",
            result.map(|s| s.name)
        );
    }
}
