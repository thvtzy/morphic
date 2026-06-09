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

        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                self.advance();
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

        while !self.is_eof() {
            self.skip_whitespace_and_comments();
            if self.peek() == Some('}') {
                self.advance();
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
                    let res_name = self.parse_identifier()?;
                    let max = if self.peek() == Some('<') {
                        self.advance();
                        let n = self.parse_integer()?;
                        self.skip_whitespace_and_comments();
                        self.expect_char('>')?;
                        Some(n as u64)
                    } else {
                        None
                    };

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
                        predicate: None,
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
            let inner = self.parse_constraint()?;
            return Ok(Constraint::Not(Box::new(inner)));
        }

        if self.peek() == Some('(') {
            self.advance();
            let mut parts = vec![self.parse_constraint()?];
            while self.peek() == Some('&') || self.peek() == Some('|') {
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
        while self.peek_n(2) == "||" {
            self.eat_str("||")?;
            let right = self.parse_expr_and()?;
            left = Expr::BinOp(BinOp::Or, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_expr_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_compare()?;
        while self.peek_n(2) == "&&" {
            self.eat_str("&&")?;
            let right = self.parse_expr_compare()?;
            left = Expr::BinOp(BinOp::And, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_expr_compare(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_expr_add()?;
        if self.peek() == Some('<') || self.peek() == Some('>') || self.peek() == Some('=') {
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
            return Ok(Expr::IntLit(n));
        }

        // String literal
        if self.peek() == Some('"') {
            let s = self.parse_string_literal_value()?;
            return Ok(Expr::StringLit(s));
        }

        // Boolean literal
        if self.peek() == Some('t') {
            if self.peek_n(4) == "true" {
                self.eat_ident("true");
                return Ok(Expr::BoolLit(true));
            }
        }
        if self.peek() == Some('f') {
            if self.peek_n(5) == "false" {
                self.eat_ident("false");
                return Ok(Expr::BoolLit(false));
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
            return Ok(Expr::Call("List".into(), items));
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
                return Ok(Expr::Call("__tuple__".into(), parts));
            }
            self.expect_char(')')?;
            return Ok(inner);
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
            self.eat_str("->")?;
            let body = self.parse_expr()?;
            return Ok(Expr::Lambda(params, Box::new(body)));
        }

        // Identifier — could be variable, function call, or field access
        if self.is_ident_start(self.peek()) {
            let mut ident = self.parse_identifier()?;

            // Check for function call: ident(args...)
            self.skip_whitespace_and_comments();
            if self.peek() == Some('(') {
                self.advance();
                let mut args = Vec::new();
                loop {
                    self.skip_whitespace_and_comments();
                    if self.peek() == Some(')') { self.advance(); break; }
                    args.push(self.parse_expr()?);
                    if self.peek() == Some(',') { self.advance(); continue; }
                }
                return Ok(Expr::Call(ident, args));
            }

            // Field access: ident.field.subfield
            let mut expr = Expr::Var(ident);
            loop {
                self.skip_whitespace_and_comments();
                if self.peek() == Some('.') {
                    self.advance();
                    let field = self.parse_identifier()?;
                    expr = Expr::Field(Box::new(expr), field);
                } else if self.peek() == Some('[') {
                    self.advance();
                    let idx = self.parse_expr()?;
                    self.expect_char(']')?;
                    expr = Expr::Index(Box::new(expr), Box::new(idx));
                } else {
                    break;
                }
            }
            return Ok(expr);
        }

        Err(self.simple_error("Expected expression"))
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, ParseError> {
        self.skip_whitespace_and_comments();
        let name = self.parse_identifier()?;

        match name.as_str() {
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
        let inner = self.parse_identifier()?;
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
        let start = self.pos.saturating_sub(20);
        let end = (self.pos + 20).min(self.source.len());
        let snippet = &self.source[start..end];
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
}
