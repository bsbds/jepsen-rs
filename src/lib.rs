//! NOTE: Requires java 21 due to https://github.com/jepsen-io/jepsen/issues/585

use std::{borrow::Borrow, cell::RefCell};

use j4rs::{errors::J4RsError, Instance, InvocationArg, Jvm, JvmBuilder};

type Result<T> = j4rs::errors::Result<T>;

/// Reads data in the edn format
///
/// Returns an `Instance` of Clojure
macro_rules! cljify {
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

/// Clojure Namespace
struct CljNs {
    ns: String,
}

impl CljNs {
    fn new(ns: String) -> Self {
        Self { ns }
    }

    fn var(&self, name: &str) -> impl Fn(Instance) -> Instance {
        Self::var_inner(&self.ns, name)
    }

    fn var_inner(ns: &str, name: &str) -> impl Fn(Instance) -> Instance {
        let var = invoke_clojure_class(
            "var",
            &[
                InvocationArg::try_from(ns).unwrap(),
                InvocationArg::try_from(name).unwrap(),
            ],
        );

        move |arg| with_jvm(|jvm| jvm.invoke(&var, "invoke", &[InvocationArg::from(arg)])).unwrap()
    }
}

struct CljCore {
    ns: &'static str,
}

impl CljCore {
    fn new() -> Self {
        Self { ns: "clojure.core" }
    }

    fn require(&self, ns: &str) -> CljNs {
        CljNs::var_inner(self.ns, "require")(read(ns));
        CljNs::new(ns.to_string())
    }

    fn var(&self, name: &str) -> impl Fn(Instance) -> Instance {
        CljNs::var_inner(self.ns, name)
    }
}

fn print(inst: Instance) -> Result<()> {
    with_jvm(|jvm| {
        let system_class = jvm.static_class("java.lang.System").unwrap();
        let system_out_field = jvm.field(&system_class, "out").unwrap();
        jvm.invoke(&system_out_field, "println", &[InvocationArg::from(inst)])?;
        Ok::<(), J4RsError>(())
    })
}

#[test]
fn test_elle() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let _jvm = JvmBuilder::new().build()?;
    let clj = CljCore::new();
    let r = clj.require("elle.rw-register");
    let h = clj.require("jepsen.history");
    let history = cljify!(
       [{:index 0 :time 0 :type :invoke :process 0 :f :txn :value [[:r 1 nil] [:w 1 2]]}
        {:index 1 :time 1 :type :invoke :process 1 :f :txn :value [[:r 1 nil] [:w 1 3]]}
        {:index 2 :time 2 :type :ok :process 0 :f :txn :value [[:r 1 2] [:w 1 2]]}
        {:index 3 :time 3 :type :ok :process 1 :f :txn :value [[:r 1 2] [:w 1 3]]}]
    );
    let jh = h.var("history")(history);
    let res = r.var("check")(jh);
    print(res).unwrap();

    Ok(())
}
