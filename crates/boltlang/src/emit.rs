use crate::{ArrowTypeExpr, BlockExpr, CallExpr, Expr, FunExpr, FuncDecl, LitExpr, NamedExpr, NamedPattern, NamedTypeExpr, Param, Pattern, SourceElement, SourceFile, TypeExpr, VarDecl};

pub struct Formatter<'a> {
    after_newline: bool,
    indent_level: u16,
    indent_size: u16,
    writer: &'a mut dyn std::io::Write,
}

impl <'a> Formatter<'a> {
    pub fn new(writer: &'a mut dyn std::io::Write) -> Self {
        Self {
            after_newline: true,
            indent_size: 2,
            indent_level: 0,
            writer,
        }
    }
}

fn is_blank(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}

impl <'a> Formatter<'a> {
    fn write(&mut self, text: &str) -> std::io::Result<()> {
        let mut buf = vec![0; 4];
        for ch in text.chars() {
            if ch != '\r' {
                if !self.after_newline {
                    if ch == '\n' {
                        self.after_newline = true;
                    }
                } else {
                    if !is_blank(ch) {
                        for _ in 0..self.indent_level * self.indent_size {
                            self.writer.write(b" ")?;
                        }
                        self.after_newline = false;
                    }
                }
            }
            self.writer.write(ch.encode_utf8(&mut buf).as_bytes())?;
        }
        Ok(())
    }
}

impl <'a> Formatter<'a> {
    fn indent(&mut self) {
        self.indent_level += 1;
    }
    fn dedent(&mut self) {
        self.indent_level = self.indent_level.checked_sub(1).expect("more dedents than indents");
    }
}

pub trait Emit {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()>;
}

impl Emit for NamedTypeExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        if let Some(name) = self.name() {
            f.write(&format!("{}", name.text()))?;
        }
        Ok(())
    }
}

impl Emit for ArrowTypeExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        for param in self.params() {
            param.emit(f)?;
            f.write(" -> ")?;
        }
        if let Some(type_expr) = self.return_ty() {
            type_expr.emit(f)?;
        }
        Ok(())
    }
}

impl Emit for TypeExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        match self {
            TypeExpr::Named(te) => te.emit(f),
            TypeExpr::Arrow(te) => te.emit(f),
        }
    }
}

impl Emit for NamedPattern {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        if self.mut_keyword().is_some() {
            f.write("mut ")?;
        }
        match self.name() {
            Some(name) =>
                f.write(&format!("{}", name.text())),
            None => f.write("..."),
        }
    }
}

impl Emit for Pattern {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        match self {
            Pattern::Named(p) => p.emit(f),
        }
    }
}

impl Emit for BlockExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        f.write("do")?;
        match self.block() {
            Some(block) => {
                f.write("\n")?;
                f.indent();
                for element in block.elements() {
                    element.emit(f)?;
                    f.write(" ")?;
                }
                f.dedent();
                Ok(())
            }
            None => f.write("..."),
        }
    }
}

impl Emit for CallExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        if let Some(operator) = self.operator() {
            operator.emit(f)?;
            f.write(" ")?;
        }
        for arg in self.args() {
            arg.emit(f)?;
            f.write(" ")?;
        }
        Ok(())
    }
}

impl Emit for LitExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        match self.value() {
            Some(value) => f.write(&format!("{}", value.text())),
            None => f.write("..."),
        }
    }
}

impl Emit for NamedExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        match self.name() {
            Some(name) => f.write(&format!("{}", name.text())),
            None => f.write("..."),
        }
    }
}

impl Emit for FunExpr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        f.write("\\")?;
        for param in self.params() {
            param.emit(f)?;
            f.write(" ")?;
        }
        f.write("-> ")?;
        match self.body() {
            Some(body) => body.emit(f)?,
            None => f.write("...")?,
        };
        Ok(())
    }
}

impl Emit for Expr {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        match self {
            Expr::Named(expr) => expr.emit(f),
            Expr::Lit(expr) => expr.emit(f),
            Expr::Fun(expr) => expr.emit(f),
            Expr::Call(expr) => expr.emit(f),
            Expr::Block(expr) => expr.emit(f),
        }
    }
}

impl Emit for Param {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        if self.type_expr().is_some() || self.default().is_some() {
            f.write("(")?;
        }
        if let Some(pattern) = self.pattern() {
            pattern.emit(f)?;
            if self.type_expr().is_some() || self.default().is_some() {
                f.write(" ")?;
            }
        }
        if let Some(type_expr) = self.type_expr() {
            f.write(": ")?;
            type_expr.emit(f)?;
        }
        if let Some(default) = self.default() {
            f.write(" = ")?;
            default.emit(f)?;
        }
        if self.type_expr().is_some() || self.default().is_some() {
            f.write(")")?;
        }
        Ok(())
    }
}

impl Emit for FuncDecl {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        if self.pub_keyword().is_some() {
            f.write("pub ")?;
        }
        f.write("fn ")?;
        if let Some(name) = self.name() {
            f.write(&format!("{} ", name.text()))?;
        }
        for param in self.params() {
            param.emit(f)?;
            f.write(" ")?;
        }
        if let Some(body) = self.body() {
            f.write("= ")?;
            body.emit(f)?;
        }
        f.write("\n")?;
        Ok(())
    }
}

impl Emit for VarDecl {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        if self.pub_keyword().is_some() {
            f.write("pub ")?;
        }
        f.write("let ")?;
        if let Some(pattern) = self.pattern() {
            pattern.emit(f)?;
        }
        if let Some(type_expr) = self.type_expr() {
            f.write(": ")?;
            type_expr.emit(f)?;
        }
        if let Some(expr) = self.expr() {
            f.write(" = ")?;
            expr.emit(f)?;
        }
        f.write("\n")?;
        Ok(())
    }
}

impl Emit for SourceElement {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        match self {
            SourceElement::Expr(expr) => { expr.emit(f)?; f.write("\n") },
            SourceElement::VarDecl(decl) => decl.emit(f),
            SourceElement::FuncDecl(decl) => decl.emit(f),
        }
    }
}

impl Emit for SourceFile {
    fn emit(&self, f: &mut Formatter<'_>) -> std::io::Result<()> {
        for element in self.elements() {
            element.emit(f)?;
        }
        Ok(())
    }
}
