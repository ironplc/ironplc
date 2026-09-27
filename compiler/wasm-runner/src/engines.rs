//! The two engines behind one trait.
//!
//! Both provide the `plc_rt` imports: `now_ns` returns the virtual time set
//! by the runner, `log` and `debug_hook` do nothing.

use crate::{err, Error, Trap};

/// An instantiated module, whatever the engine.
pub(crate) trait Instance {
    fn abi_version(&mut self) -> Result<i32, Error>;
    fn set_now_ns(&mut self, now: i64);
    fn init(&mut self) -> Result<i32, Trap>;
    fn run(&mut self, task: i32) -> Result<i32, Trap>;
    fn read(&self, addr: u32, len: u32) -> Result<Vec<u8>, Error>;
    fn write(&mut self, addr: u32, bytes: &[u8]) -> Result<(), Error>;
    /// Sets `plc_fuel` when the module exports it (ABI-070).
    fn set_fuel(&mut self, fuel: i64);
}

/// The host state: the virtual clock.
#[derive(Default)]
struct Host {
    now_ns: i64,
}

const RT: &str = "plc_rt";
const IMPORTS: [&str; 3] = ["now_ns", "log", "debug_hook"];

fn check_import(module: &str, name: &str) -> Result<(), Error> {
    if module == RT && IMPORTS.contains(&name) {
        Ok(())
    } else {
        Err(Error(format!("forbidden import {module}.{name}")))
    }
}

fn slice(data: &[u8], addr: u32, len: u32) -> Result<Vec<u8>, Error> {
    let start = addr as usize;
    data.get(start..start + len as usize)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| Error(format!("address {addr}+{len} outside the memory")))
}

fn slice_mut(data: &mut [u8], addr: u32, bytes: &[u8]) -> Result<(), Error> {
    let start = addr as usize;
    data.get_mut(start..start + bytes.len())
        .ok_or_else(|| Error(format!("address {addr} outside the memory")))?
        .copy_from_slice(bytes);
    Ok(())
}

pub(crate) struct WasmtimeInstance {
    store: wasmtime::Store<Host>,
    instance: wasmtime::Instance,
    memory: wasmtime::Memory,
    init: wasmtime::TypedFunc<(), i32>,
    run: wasmtime::TypedFunc<i32, i32>,
}

impl WasmtimeInstance {
    pub(crate) fn new(wasm: &[u8]) -> Result<Self, Error> {
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, wasm).map_err(err)?;
        for import in module.imports() {
            check_import(import.module(), import.name())?;
        }
        let mut store = wasmtime::Store::new(&engine, Host::default());
        let mut linker = wasmtime::Linker::new(&engine);
        linker
            .func_wrap(RT, "now_ns", |c: wasmtime::Caller<'_, Host>| {
                c.data().now_ns
            })
            .map_err(err)?;
        linker
            .func_wrap(
                RT,
                "log",
                |_: wasmtime::Caller<'_, Host>, _: i32, _: i32, _: i32| {},
            )
            .map_err(err)?;
        linker
            .func_wrap(RT, "debug_hook", |_: wasmtime::Caller<'_, Host>, _: i32| {})
            .map_err(err)?;
        let instance = linker.instantiate(&mut store, &module).map_err(err)?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| Error("no memory export".into()))?;
        let init = instance
            .get_typed_func::<(), i32>(&mut store, "plc_init")
            .map_err(err)?;
        let run = instance
            .get_typed_func::<i32, i32>(&mut store, "plc_task_run")
            .map_err(err)?;
        Ok(WasmtimeInstance {
            store,
            instance,
            memory,
            init,
            run,
        })
    }

    fn global(&mut self, name: &str) -> Option<i32> {
        self.instance
            .get_global(&mut self.store, name)
            .and_then(|g| g.get(&mut self.store).i32())
    }

    fn trap(&mut self, e: impl std::fmt::Debug) -> Trap {
        let code = self.global("plc_trap_code").unwrap_or(0);
        let site = (code != 0)
            .then(|| self.global("plc_trap_site").map(|s| s as u32))
            .flatten();
        Trap {
            code,
            site,
            message: format!("{e:?}"),
        }
    }
}

impl Instance for WasmtimeInstance {
    fn abi_version(&mut self) -> Result<i32, Error> {
        self.instance
            .get_typed_func::<(), i32>(&mut self.store, "plc_abi_version")
            .and_then(|f| f.call(&mut self.store, ()))
            .map_err(err)
    }

    fn set_now_ns(&mut self, now: i64) {
        self.store.data_mut().now_ns = now;
    }

    fn init(&mut self) -> Result<i32, Trap> {
        let result = self.init.call(&mut self.store, ());
        result.map_err(|e| self.trap(e))
    }

    fn run(&mut self, task: i32) -> Result<i32, Trap> {
        let result = self.run.call(&mut self.store, task);
        result.map_err(|e| self.trap(e))
    }

    fn read(&self, addr: u32, len: u32) -> Result<Vec<u8>, Error> {
        slice(self.memory.data(&self.store), addr, len)
    }

    fn write(&mut self, addr: u32, bytes: &[u8]) -> Result<(), Error> {
        slice_mut(self.memory.data_mut(&mut self.store), addr, bytes)
    }

    fn set_fuel(&mut self, fuel: i64) {
        if let Some(g) = self.instance.get_global(&mut self.store, "plc_fuel") {
            let _ = g.set(&mut self.store, wasmtime::Val::I64(fuel));
        }
    }
}

pub(crate) struct WasmiInstance {
    store: wasmi::Store<Host>,
    instance: wasmi::Instance,
    memory: wasmi::Memory,
    init: wasmi::TypedFunc<(), i32>,
    run: wasmi::TypedFunc<i32, i32>,
}

impl WasmiInstance {
    pub(crate) fn new(wasm: &[u8]) -> Result<Self, Error> {
        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, wasm).map_err(err)?;
        for import in module.imports() {
            check_import(import.module(), import.name())?;
        }
        let mut store = wasmi::Store::new(&engine, Host::default());
        let mut linker = wasmi::Linker::<Host>::new(&engine);
        linker
            .func_wrap(RT, "now_ns", |c: wasmi::Caller<'_, Host>| c.data().now_ns)
            .map_err(err)?;
        linker
            .func_wrap(
                RT,
                "log",
                |_: wasmi::Caller<'_, Host>, _: i32, _: i32, _: i32| {},
            )
            .map_err(err)?;
        linker
            .func_wrap(RT, "debug_hook", |_: wasmi::Caller<'_, Host>, _: i32| {})
            .map_err(err)?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(err)?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| Error("no memory export".into()))?;
        let init = instance
            .get_typed_func::<(), i32>(&store, "plc_init")
            .map_err(err)?;
        let run = instance
            .get_typed_func::<i32, i32>(&store, "plc_task_run")
            .map_err(err)?;
        Ok(WasmiInstance {
            store,
            instance,
            memory,
            init,
            run,
        })
    }

    fn global(&self, name: &str) -> Option<i32> {
        self.instance
            .get_global(&self.store, name)
            .and_then(|g| g.get(&self.store).i32())
    }

    fn trap(&self, e: impl std::fmt::Debug) -> Trap {
        let code = self.global("plc_trap_code").unwrap_or(0);
        let site = (code != 0)
            .then(|| self.global("plc_trap_site").map(|s| s as u32))
            .flatten();
        Trap {
            code,
            site,
            message: format!("{e:?}"),
        }
    }
}

impl Instance for WasmiInstance {
    fn abi_version(&mut self) -> Result<i32, Error> {
        self.instance
            .get_typed_func::<(), i32>(&self.store, "plc_abi_version")
            .map_err(err)?
            .call(&mut self.store, ())
            .map_err(err)
    }

    fn set_now_ns(&mut self, now: i64) {
        self.store.data_mut().now_ns = now;
    }

    fn init(&mut self) -> Result<i32, Trap> {
        let init = self.init;
        init.call(&mut self.store, ()).map_err(|e| self.trap(e))
    }

    fn run(&mut self, task: i32) -> Result<i32, Trap> {
        let run = self.run;
        run.call(&mut self.store, task).map_err(|e| self.trap(e))
    }

    fn read(&self, addr: u32, len: u32) -> Result<Vec<u8>, Error> {
        slice(self.memory.data(&self.store), addr, len)
    }

    fn write(&mut self, addr: u32, bytes: &[u8]) -> Result<(), Error> {
        slice_mut(self.memory.data_mut(&mut self.store), addr, bytes)
    }

    fn set_fuel(&mut self, fuel: i64) {
        if let Some(g) = self.instance.get_global(&self.store, "plc_fuel") {
            let _ = g.set(&mut self.store, wasmi::Val::I64(fuel));
        }
    }
}
