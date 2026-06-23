use crate::{
    SourceElement,
    SourceFile, tc::infer::GenOut
};
use super::{
    InferContext,
    TypeEnvId,
};

impl InferContext {

    pub fn infer_element(&mut self, element: &SourceElement, is_toplevel: bool, env: TypeEnvId) -> GenOut {
        match element {
            SourceElement::VarDecl(decl) => self.infer_var_decl(decl, is_toplevel, env),
            SourceElement::FuncDecl(decl) => self.infer_func_decl(decl, env),
            SourceElement::Expr(expr) => self.infer_expr(expr, env).0,
        }
    }

    pub fn infer_source_file(&mut self, node: &SourceFile) -> GenOut {
        let env = self.fork_env(self.global_env());
        let mut out = GenOut::new();
        for element in node.elements() {
            let el_out = self.infer_element(&element, true, env.id());
            out.extend(el_out);
        }
        self.drop_env(env);
        out
    }

}
