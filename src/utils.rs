#[cfg(test)]
use j4rs::{Instance, InvocationArg};

#[cfg(test)]
use crate::{with_jvm, CljCore};

#[cfg(test)]
pub(crate) fn print(inst: Instance) {
    with_jvm(|jvm| {
        let system_class = jvm.static_class("java.lang.System").unwrap();
        let system_out_field = jvm.field(&system_class, "out").unwrap();
        jvm.invoke(&system_out_field, "println", &[InvocationArg::from(inst)])
            .unwrap();
    })
}

#[cfg(test)]
pub(crate) fn print_lazy(inst: Instance) {
    let clj = CljCore::new();
    let inst = clj.var("pr-str").invoke1(inst);
    print(inst);
}
