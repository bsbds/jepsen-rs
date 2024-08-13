//! NOTE: Requires java 21 due to https://github.com/jepsen-io/jepsen/issues/585

mod generator;

mod checker;

mod context;

mod jtests;

pub(crate) mod utils;

use std::{borrow::Borrow, cell::RefCell};

use j4rs::{Instance, InvocationArg, Jvm};

/// Reads data in the edn format
#[macro_export]
macro_rules! cljread {
    ($($char:tt)*) => {
        {
            let mut s = String::new();
            $(
                s += stringify!($char);
            )*
            read(&s)
        }
    };
}

/// Evaluate the string
#[macro_export]
macro_rules! cljeval {
    ($($char:tt)*) => {
        {
            let mut s = String::new();
            $(
                s += stringify!($char);
            )*
            eval(&s)
        }
    };
}

thread_local! {
    static JVM: RefCell<Option<Jvm>> = const { RefCell::new(None) };
}

fn with_jvm<F, R>(f: F) -> R
where
    F: FnOnce(&Jvm) -> R,
{
    JVM.with(|cell| {
        if let Ok(mut jvm) = cell.try_borrow_mut() {
            if jvm.is_none() {
                jvm.replace(Jvm::attach_thread().unwrap());
            }
        }
        f(cell.borrow().as_ref().unwrap())
    })
}

fn read(arg: &str) -> Instance {
    invoke_clojure_class("read", &[InvocationArg::try_from(arg).unwrap()])
}

fn eval(arg: &str) -> Instance {
    let clj = CljCore::new();
    clj.var("load-string")
        .invoke1(InvocationArg::try_from(arg).unwrap())
}

fn invoke_clojure_class(method_name: &str, inv_args: &[impl Borrow<InvocationArg>]) -> Instance {
    with_jvm(|jvm| {
        jvm.invoke(
            &with_jvm(|jvm| jvm.static_class("clojure.java.api.Clojure").unwrap()),
            method_name,
            inv_args,
        )
    })
    .unwrap()
}

pub struct IFn {
    inner: Instance,
}

impl IFn {
    pub fn new(inner: Instance) -> Self {
        Self { inner }
    }

    pub fn invoke0(&self) -> Instance {
        self.invoke(&[] as &[InvocationArg])
    }

    pub fn invoke1(&self, arg: impl Into<InvocationArg>) -> Instance {
        self.invoke(&[arg.into()])
    }

    pub fn invoke(&self, args: &[impl Borrow<InvocationArg>]) -> Instance {
        with_jvm(|jvm| jvm.invoke(&self.inner, "invoke", args)).unwrap()
    }

    pub fn into_inner(self) -> Instance {
        self.inner
    }
}

/// Clojure Namespace
pub struct CljNs {
    ns: String,
}

impl CljNs {
    pub fn new(ns: String) -> Self {
        Self { ns }
    }

    pub fn var(&self, name: &str) -> IFn {
        Self::var_inner(&self.ns, name)
    }

    fn var_inner(ns: &str, name: &str) -> IFn {
        let inner = invoke_clojure_class(
            "var",
            &[
                InvocationArg::try_from(ns).unwrap(),
                InvocationArg::try_from(name).unwrap(),
            ],
        );

        IFn { inner }
    }
}

pub struct CljCore {
    ns: &'static str,
}

impl CljCore {
    pub fn new() -> Self {
        Self { ns: "clojure.core" }
    }

    pub fn require(&self, ns: &str) -> CljNs {
        CljNs::var_inner(self.ns, "require").invoke1(read(ns));
        CljNs::new(ns.to_string())
    }

    pub fn var(&self, name: &str) -> IFn {
        CljNs::var_inner(self.ns, name)
    }
}

impl Default for CljCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use j4rs::JvmBuilder;

    use self::utils::print_lazy;

    use super::*;
    use crate::utils::print;

    #[test]
    fn test_elle_analysis() -> Result<(), Box<dyn std::error::Error>> {
        let _jvm = JvmBuilder::new().build()?;
        let clj = CljCore::new();
        let r = clj.require("elle.rw-register");
        let h = clj.require("jepsen.history");
        let history = cljeval!(
           [{:index 0 :time 0 :type :invoke :process 0 :f :txn :value [[:r 1 nil] [:w 1 2]]}
            {:index 1 :time 1 :type :invoke :process 1 :f :txn :value [[:r 1 nil] [:w 1 3]]}
            {:index 2 :time 2 :type :ok :process 0 :f :txn :value [[:r 1 2] [:w 1 2]]}
            {:index 3 :time 3 :type :ok :process 1 :f :txn :value [[:r 1 2] [:w 1 3]]}]
        );
        let jh = h.var("history").invoke1(history);
        let res = r.var("check").invoke1(jh);
        print(res);

        Ok(())
    }

    #[test]
    fn test_elle_gen() -> Result<(), Box<dyn std::error::Error>> {
        let _jvm = JvmBuilder::new().build()?;
        let clj = CljCore::new();
        let r = clj.require("elle.rw-register");
        let gen = r.var("gen").invoke0();
        let t = clj.var("take").invoke(&[
            InvocationArg::try_from(10).unwrap(),
            InvocationArg::from(gen),
        ]);
        print_lazy(t);

        Ok(())
    }

    #[test]
    fn elle_gen_analysis() -> Result<(), Box<dyn std::error::Error>> {
        let _jvm = JvmBuilder::new().build()?;
        let clj = CljCore::new();
        let r = clj.require("elle.rw-register");
        let g = clj.require("jepsen.generator");
        let h = clj.require("jepsen.history");
        let gen = r.var("gen").invoke0();

        let history = clj.var("take").invoke(&[
            InvocationArg::try_from(2).unwrap(),
            InvocationArg::from(gen),
        ]);

        let assocfn = cljeval!(
            #(assoc % :new-key :new-value)
        );

        let res = clj
            .var("map")
            .invoke(&[InvocationArg::from(assocfn), InvocationArg::from(history)]);

        // let assoc = clj.var("assoc");
        // clj.var("map").invoke(&[
        //     InvocationArg::from(assoc.into_inner()),
        //     Clojure.var("clojure.core", "%"),
        // ]);
        print_lazy(res);

        // let res = r.var("check").invoke1(h.var("history").invoke1(history));
        // print(res);

        Ok(())
    }
}
