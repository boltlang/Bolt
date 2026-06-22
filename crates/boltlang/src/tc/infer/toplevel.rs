use crate::{
    Diagnostic,
    SourceElement,
    SourceFile
};
use super::{
    Constraints,
    InferContext,
    TypeEnvId,
};

impl InferContext {

    pub fn infer_element(&mut self, element: &SourceElement, is_toplevel: bool, env: TypeEnvId) -> (Constraints, Vec<Diagnostic>) {
        match element {
            SourceElement::VarDecl(decl) => self.infer_var_decl(decl, is_toplevel, env),
            SourceElement::FuncDecl(decl) => self.infer_func_decl(decl, env),
            SourceElement::Expr(expr) => {
                let (_ty, cs, ds) = self.infer_expr(expr, env);
                (cs, ds)
            }
        }
    }

    pub fn infer_source_file(&mut self, node: &SourceFile) -> (Constraints, Vec<Diagnostic>) {
        let env = self.fork_env(self.global_env());
        let mut cs = Constraints::new();
        let mut ds = Vec::new();
        for element in node.elements() {
            let (el_cs, el_ds) = self.infer_element(&element, true, env.id());
            cs.extend(el_cs);
            ds.extend(el_ds);
        }
        self.drop_env(env);
        (cs, ds)
    }

}
