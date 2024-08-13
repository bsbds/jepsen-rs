use crate::{cljeval, generator::Generator, CljCore};

struct Gen;

impl Gen {
    fn new() -> Self {
        let clj = CljCore::new();
        let r = clj.require("elle.rw-register");
        let gen = r.var("gen").invoke0();
        cljeval!(
            (let)
        )

        Self {}
    }
}

impl Generator for Gen {
    fn op(&self, ctx: Option<()>, test_ctx: Option<()>) -> Instance {
        todo!()
    }
}
