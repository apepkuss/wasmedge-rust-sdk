//! Defines WasmEdge Instance and other relevant types.

#[cfg(all(feature = "async", target_os = "linux"))]
use crate::{
    async_wasi::{wasi_impls, WasiFunc},
    instance::function::wrap_async_fn,
    BoxedAsyncFn, WasiCtx, ASYNC_HOST_FUNCS,
};
use crate::{
    ffi,
    instance::{
        function::{wrap_fn, FuncType, Function, InnerFunc},
        global::{Global, GlobalType, InnerGlobal},
        memory::{InnerMemory, MemType, Memory},
        table::{InnerTable, Table, TableType},
    },
    types::WasmEdgeString,
    BoxedFn, WasmEdgeResult, WasmValue, HOST_FUNCS, HOST_FUNC_FOOTPRINTS,
};
use parking_lot::Mutex;
use rand::Rng;
#[cfg(all(feature = "async", target_os = "linux"))]
use std::path::PathBuf;
use std::sync::Arc;
use wasmedge_types::error::{FuncError, InstanceError, WasmEdgeError};

/// An [Instance] represents an instantiated module. In the instantiation process, An [Instance] is created from al[Module](crate::Module). From an [Instance] the exported [functions](crate::Function), [tables](crate::Table), [memories](crate::Memory), and [globals](crate::Global) can be fetched.
#[derive(Debug)]
pub struct Instance {
    pub(crate) inner: Arc<Mutex<InnerInstance>>,
    pub(crate) registered: bool,
}
impl Drop for Instance {
    fn drop(&mut self) {
        if self.registered {
            self.inner.lock().0 = std::ptr::null_mut();
        } else if Arc::strong_count(&self.inner) == 1 && !self.inner.lock().0.is_null() {
            unsafe {
                ffi::WasmEdge_ModuleInstanceDelete(self.inner.lock().0);
            }
        }
    }
}
impl Instance {
    /// Returns the name of this exported [module instance](crate::Instance).
    ///
    /// If this module instance is an active module instance, then None is returned.
    pub fn name(&self) -> Option<String> {
        let name =
            unsafe { ffi::WasmEdge_ModuleInstanceGetModuleName(self.inner.lock().0 as *const _) };

        let name: String = name.into();
        if name.is_empty() {
            return None;
        }

        Some(name)
    }

    /// Returns the exported [function instance](crate::Function) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [function instance](crate::Function).
    ///
    /// # Error
    ///
    /// If fail to find the target [function](crate::Function), then an error is returned.
    pub fn get_func(&self, name: impl AsRef<str>) -> WasmEdgeResult<Function> {
        let func_name: WasmEdgeString = name.as_ref().into();
        let func_ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindFunction(
                self.inner.lock().0 as *const _,
                func_name.as_raw(),
            )
        };
        match func_ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundFunc(name.as_ref().to_string()),
            ))),
            false => Ok(Function {
                inner: Arc::new(Mutex::new(InnerFunc(func_ctx))),
                registered: true,
            }),
        }
    }

    /// Returns the exported [table instance](crate::Table) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [table instance](crate::Table).
    ///
    /// # Error
    ///
    /// If fail to find the target [table instance](crate::Table), then an error is returned.
    pub fn get_table(&self, name: impl AsRef<str>) -> WasmEdgeResult<Table> {
        let table_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindTable(
                self.inner.lock().0 as *const _,
                table_name.as_raw(),
            )
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundTable(name.as_ref().to_string()),
            ))),
            false => Ok(Table {
                inner: Arc::new(Mutex::new(InnerTable(ctx))),
                registered: true,
            }),
        }
    }

    /// Returns the exported [memory instance](crate::Memory) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [memory instance](crate::Memory).
    ///
    /// # Error
    ///
    /// If fail to find the target [memory instance](crate::Memory), then an error is returned.
    pub fn get_memory(&self, name: impl AsRef<str>) -> WasmEdgeResult<Memory> {
        let mem_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindMemory(
                self.inner.lock().0 as *const _,
                mem_name.as_raw(),
            )
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundMem(name.as_ref().to_string()),
            ))),
            false => Ok(Memory {
                inner: Arc::new(Mutex::new(InnerMemory(ctx))),
                registered: true,
            }),
        }
    }

    /// Returns the exported [global instance](crate::Global) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [global instance](crate::Global).
    ///
    /// # Error
    ///
    /// If fail to find the target [global instance](crate::Global), then an error is returned.
    pub fn get_global(&self, name: impl AsRef<str>) -> WasmEdgeResult<Global> {
        let global_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindGlobal(
                self.inner.lock().0 as *const _,
                global_name.as_raw(),
            )
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundGlobal(name.as_ref().to_string()),
            ))),
            false => Ok(Global {
                inner: Arc::new(Mutex::new(InnerGlobal(ctx))),
                registered: true,
            }),
        }
    }

    /// Returns the length of the exported [function instances](crate::Function) in this module instance.
    pub fn func_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListFunctionLength(self.inner.lock().0) }
    }

    /// Returns the names of the exported [function instances](crate::Function) in this module instance.
    pub fn func_names(&self) -> Option<Vec<String>> {
        let len_func_names = self.func_len();
        match len_func_names > 0 {
            true => {
                let mut func_names = Vec::with_capacity(len_func_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListFunction(
                        self.inner.lock().0,
                        func_names.as_mut_ptr(),
                        len_func_names,
                    );
                    func_names.set_len(len_func_names as usize);
                }

                let names = func_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [table instances](crate::Table) in this module instance.
    pub fn table_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListTableLength(self.inner.lock().0) }
    }

    /// Returns the names of the exported [table instances](crate::Table) in this module instance.
    pub fn table_names(&self) -> Option<Vec<String>> {
        let len_table_names = self.table_len();
        match len_table_names > 0 {
            true => {
                let mut table_names = Vec::with_capacity(len_table_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListTable(
                        self.inner.lock().0,
                        table_names.as_mut_ptr(),
                        len_table_names,
                    );
                    table_names.set_len(len_table_names as usize);
                }

                let names = table_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [memory instances](crate::Memory) in this module instance.
    pub fn mem_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListMemoryLength(self.inner.lock().0) }
    }

    /// Returns the names of all exported [memory instances](crate::Memory) in this module instance.
    pub fn mem_names(&self) -> Option<Vec<String>> {
        let len_mem_names = self.mem_len();
        match len_mem_names > 0 {
            true => {
                let mut mem_names = Vec::with_capacity(len_mem_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListMemory(
                        self.inner.lock().0,
                        mem_names.as_mut_ptr(),
                        len_mem_names,
                    );
                    mem_names.set_len(len_mem_names as usize);
                }

                let names = mem_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [global instances](crate::Global) in this module instance.
    pub fn global_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListGlobalLength(self.inner.lock().0) }
    }

    /// Returns the names of the exported [global instances](crate::Global) in this module instance.
    pub fn global_names(&self) -> Option<Vec<String>> {
        let len_global_names = self.global_len();
        match len_global_names > 0 {
            true => {
                let mut global_names = Vec::with_capacity(len_global_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListGlobal(
                        self.inner.lock().0,
                        global_names.as_mut_ptr(),
                        len_global_names,
                    );
                    global_names.set_len(len_global_names as usize);
                }

                let names = global_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the host data held by the module instance.
    pub fn host_data<T: Send + Sync + Clone>(&mut self) -> Option<&mut T> {
        let ctx = unsafe { ffi::WasmEdge_ModuleInstanceGetHostData(self.inner.lock().0) };

        match ctx.is_null() {
            true => None,
            false => {
                let ctx = unsafe { &mut *(ctx as *mut T) };
                Some(ctx)
            }
        }
    }

    /// Provides a raw pointer to the inner module instance context.
    #[cfg(feature = "ffi")]
    pub fn as_ptr(&self) -> *const ffi::WasmEdge_ModuleInstanceContext {
        self.inner.lock().0 as *const _
    }
}
impl Clone for Instance {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            registered: self.registered,
        }
    }
}

#[derive(Debug)]
pub(crate) struct InnerInstance(pub(crate) *mut ffi::WasmEdge_ModuleInstanceContext);
unsafe impl Send for InnerInstance {}
unsafe impl Sync for InnerInstance {}

/// The object as an module instance is required to implement this trait.
pub trait AsInstance {
    /// Returns the exported [function instance](crate::Function) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [function instance](crate::Function).
    ///
    /// # Error
    ///
    /// If fail to find the target [function](crate::Function), then an error is returned.
    fn get_func(&self, name: impl AsRef<str>) -> WasmEdgeResult<Function>;

    /// Returns the length of the exported [function instances](crate::Function) in this module instance.
    fn func_len(&self) -> u32;

    /// Returns the names of the exported [function instances](crate::Function) in this module instance.
    fn func_names(&self) -> Option<Vec<String>>;

    /// Returns the exported [table instance](crate::Table) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [table instance](crate::Table).
    ///
    /// # Error
    ///
    /// If fail to find the target [table instance](crate::Table), then an error is returned.
    fn get_table(&self, name: impl AsRef<str>) -> WasmEdgeResult<Table>;

    /// Returns the length of the exported [table instances](crate::Table) in this module instance.
    fn table_len(&self) -> u32;

    /// Returns the names of the exported [table instances](crate::Table) in this module instance.
    fn table_names(&self) -> Option<Vec<String>>;

    /// Returns the exported [memory instance](crate::Memory) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [memory instance](crate::Memory).
    ///
    /// # Error
    ///
    /// If fail to find the target [memory instance](crate::Memory), then an error is returned.
    fn get_memory(&self, name: impl AsRef<str>) -> WasmEdgeResult<Memory>;

    /// Returns the length of the exported [memory instances](crate::Memory) in this module instance.
    fn mem_len(&self) -> u32;

    /// Returns the names of all exported [memory instances](crate::Memory) in this module instance.
    fn mem_names(&self) -> Option<Vec<String>>;

    /// Returns the exported [global instance](crate::Global) by name.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the target exported [global instance](crate::Global).
    ///
    /// # Error
    ///
    /// If fail to find the target [global instance](crate::Global), then an error is returned.
    fn get_global(&self, name: impl AsRef<str>) -> WasmEdgeResult<Global>;

    /// Returns the length of the exported [global instances](crate::Global) in this module instance.
    fn global_len(&self) -> u32;

    /// Returns the names of the exported [global instances](crate::Global) in this module instance.
    fn global_names(&self) -> Option<Vec<String>>;
}

/// An [ImportModule] represents a host module with a name. A host module consists of one or more host [function](crate::Function), [table](crate::Table), [memory](crate::Memory), and [global](crate::Global) instances,  which are defined outside wasm modules and fed into wasm modules as imports.
#[derive(Debug, Clone)]
pub struct ImportModule<T: Send + Sync + Clone> {
    pub(crate) inner: Arc<InnerInstance>,
    pub(crate) registered: bool,
    name: String,
    funcs: Vec<Function>,
    host_data: Option<Box<T>>,
}
impl<T: Send + Sync + Clone> Drop for ImportModule<T> {
    fn drop(&mut self) {
        if !self.registered && Arc::strong_count(&self.inner) == 1 && !self.inner.0.is_null() {
            // free the module instance
            unsafe {
                ffi::WasmEdge_ModuleInstanceDelete(self.inner.0);
            }

            // drop the registered host functions
            self.funcs.drain(..);
        }
    }
}
impl<T: Send + Sync + Clone> ImportModule<T> {
    /// Creates a module instance which is used to import host functions, tables, memories, and globals into a wasm module.
    ///
    /// # Argument
    ///
    /// * `name` - The name of the import module instance.
    ///
    /// * `host_data` - The host data to be stored in the module instance.
    ///
    /// # Error
    ///
    /// If fail to create the import module instance, then an error is returned.
    pub fn create(name: impl AsRef<str>, host_data: Option<Box<T>>) -> WasmEdgeResult<Self> {
        let raw_name = WasmEdgeString::from(name.as_ref());

        let mut import = Self {
            inner: std::sync::Arc::new(InnerInstance(std::ptr::null_mut())),
            registered: false,
            name: name.as_ref().to_string(),
            funcs: Vec::new(),
            host_data,
        };

        let ctx = match &mut import.host_data {
            Some(boxed_data) => {
                let p = boxed_data.as_mut() as *mut T as *mut std::ffi::c_void;
                unsafe { ffi::WasmEdge_ModuleInstanceCreateWithData(raw_name.as_raw(), p, None) }
            }
            None => unsafe { ffi::WasmEdge_ModuleInstanceCreate(raw_name.as_raw()) },
        };

        if ctx.is_null() {
            return Err(Box::new(WasmEdgeError::Instance(
                InstanceError::CreateImportModule,
            )));
        }

        import.inner = std::sync::Arc::new(InnerInstance(ctx));

        Ok(import)
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn add_func_new(
        &mut self,
        name: impl AsRef<str>,
        ty: &FuncType,
        real_fn: BoxedFn,
        cost: u64,
    ) -> WasmEdgeResult<()> {
        // create host function
        let func = {
            let data = match &mut self.host_data {
                Some(boxed_data) => boxed_data.as_mut() as *mut T as *mut std::ffi::c_void,
                None => std::ptr::null_mut(),
            };

            let mut map_host_func = HOST_FUNCS.write();

            // generate key for the coming host function
            let mut rng = rand::thread_rng();
            let mut key: usize = rng.gen();
            while map_host_func.contains_key(&key) {
                key = rng.gen();
            }
            map_host_func.insert(key, Arc::new(Mutex::new(real_fn)));
            drop(map_host_func);

            let ctx = unsafe {
                ffi::WasmEdge_FunctionInstanceCreateBinding(
                    ty.inner.0,
                    Some(wrap_fn),
                    key as *const usize as *mut std::ffi::c_void,
                    data,
                    cost,
                )
            };

            // create a footprint for the host function
            let footprint = ctx as usize;
            let mut footprint_to_id = HOST_FUNC_FOOTPRINTS.lock();
            footprint_to_id.insert(footprint, key);

            if ctx.is_null() {
                return Err(Box::new(WasmEdgeError::Func(FuncError::Create)));
            }

            Function {
                inner: Arc::new(Mutex::new(InnerFunc(ctx))),
                registered: false,
            }
        };

        self.funcs.push(func);
        let f = self.funcs.last_mut().unwrap();

        // add host function to the import module instance
        let func_name: WasmEdgeString = name.into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddFunction(
                self.inner.0,
                func_name.as_raw(),
                f.inner.lock().0,
            );
        }

        // ! Notice that, `f.inner.lock().0` is not set to null here as the pointer will be used in `Function::drop`.

        Ok(())
    }

    #[cfg(all(feature = "async", target_os = "linux"))]
    pub fn add_async_func(
        &mut self,
        name: impl AsRef<str>,
        ty: &FuncType,
        real_fn: BoxedAsyncFn,
        cost: u64,
    ) -> WasmEdgeResult<()> {
        // create host function
        let func = {
            let data = match &mut self.host_data {
                Some(boxed_data) => boxed_data.as_mut() as *mut T as *mut std::ffi::c_void,
                None => std::ptr::null_mut(),
            };

            let mut map_host_func = ASYNC_HOST_FUNCS.write();

            // generate key for the coming host function
            let mut rng = rand::thread_rng();
            let mut key: usize = rng.gen();
            while map_host_func.contains_key(&key) {
                key = rng.gen();
            }
            map_host_func.insert(key, Arc::new(Mutex::new(real_fn)));
            drop(map_host_func);

            let ctx = unsafe {
                ffi::WasmEdge_FunctionInstanceCreateBinding(
                    ty.inner.0,
                    Some(wrap_async_fn),
                    key as *const usize as *mut std::ffi::c_void,
                    data,
                    cost,
                )
            };

            // create a footprint for the host function
            let footprint = ctx as usize;
            let mut footprint_to_id = HOST_FUNC_FOOTPRINTS.lock();
            footprint_to_id.insert(footprint, key);

            if ctx.is_null() {
                return Err(Box::new(WasmEdgeError::Func(FuncError::Create)));
            }

            Function {
                inner: Arc::new(Mutex::new(InnerFunc(ctx))),
                registered: false,
            }
        };

        self.funcs.push(func);
        let f = self.funcs.last_mut().unwrap();

        // add host function to the import module instance
        let func_name: WasmEdgeString = name.into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddFunction(
                self.inner.0,
                func_name.as_raw(),
                f.inner.lock().0,
            );
        }

        // ! Notice that, `f.inner.lock().0` is not set to null here as the pointer will be used in `Function::drop`.

        Ok(())
    }

    pub fn add_table_new(&mut self, name: impl AsRef<str>, ty: &TableType) -> WasmEdgeResult<()> {
        // create Table instance
        let table = Table::create(ty)?;

        // add table to the import module instance
        let table_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddTable(
                self.inner.0,
                table_name.as_raw(),
                table.inner.lock().0,
            );
        }

        table.inner.lock().0 = std::ptr::null_mut();

        Ok(())
    }

    pub fn add_table_with_data(
        &mut self,
        name: impl AsRef<str>,
        ty: &TableType,
        idx: u32,
        data: WasmValue,
    ) -> WasmEdgeResult<()> {
        // create Table instance
        let mut table = Table::create(ty)?;

        // set data at the given index
        table.set_data(data, idx)?;

        // add table to the import module instance
        let table_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddTable(
                self.inner.0,
                table_name.as_raw(),
                table.inner.lock().0,
            );
        }

        table.inner.lock().0 = std::ptr::null_mut();

        Ok(())
    }

    pub fn add_memory_new(&mut self, name: impl AsRef<str>, ty: &MemType) -> WasmEdgeResult<()> {
        // create Memory instance
        let memory = Memory::create(ty)?;

        // add memory to the import module instance
        let mem_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddMemory(
                self.inner.0,
                mem_name.as_raw(),
                memory.inner.lock().0,
            );
        }
        memory.inner.lock().0 = std::ptr::null_mut();

        Ok(())
    }

    pub fn add_global_new(
        &mut self,
        name: impl AsRef<str>,
        ty: &GlobalType,
        val: WasmValue,
    ) -> WasmEdgeResult<()> {
        // create Global instance
        let global = Global::create(ty, val)?;

        // add global to the import module instance
        let global_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddGlobal(
                self.inner.0,
                global_name.as_raw(),
                global.inner.lock().0,
            );
        }
        global.inner.lock().0 = std::ptr::null_mut();

        Ok(())
    }

    /// Provides a raw pointer to the inner module instance context.
    #[cfg(feature = "ffi")]
    pub fn as_ptr(&self) -> *const ffi::WasmEdge_ModuleInstanceContext {
        self.inner.0 as *const _
    }
}
// impl<T: Send + Sync + Clone> AsImport for ImportModule<T> {
//     fn name(&self) -> &str {
//         self.name.as_str()
//     }

//     fn add_func(&mut self, name: impl AsRef<str>, func: Function) {
//         self.funcs.push(func);
//         let f = self.funcs.last_mut().unwrap();

//         let func_name: WasmEdgeString = name.into();
//         unsafe {
//             ffi::WasmEdge_ModuleInstanceAddFunction(
//                 self.inner.0,
//                 func_name.as_raw(),
//                 f.inner.lock().0,
//             );
//         }

//         // ! Notice that, `f.inner.lock().0` is not set to null here as the pointer will be used in `Function::drop`.
//     }

//     fn add_table(&mut self, name: impl AsRef<str>, table: Table) {
//         let table_name: WasmEdgeString = name.as_ref().into();
//         unsafe {
//             ffi::WasmEdge_ModuleInstanceAddTable(
//                 self.inner.0,
//                 table_name.as_raw(),
//                 table.inner.lock().0,
//             );
//         }

//         table.inner.lock().0 = std::ptr::null_mut();
//     }

//     fn add_memory(&mut self, name: impl AsRef<str>, memory: Memory) {
//         let mem_name: WasmEdgeString = name.as_ref().into();
//         unsafe {
//             ffi::WasmEdge_ModuleInstanceAddMemory(
//                 self.inner.0,
//                 mem_name.as_raw(),
//                 memory.inner.lock().0,
//             );
//         }
//         memory.inner.lock().0 = std::ptr::null_mut();
//     }

//     fn add_global(&mut self, name: impl AsRef<str>, global: Global) {
//         let global_name: WasmEdgeString = name.as_ref().into();
//         unsafe {
//             ffi::WasmEdge_ModuleInstanceAddGlobal(
//                 self.inner.0,
//                 global_name.as_raw(),
//                 global.inner.lock().0,
//             );
//         }
//         global.inner.lock().0 = std::ptr::null_mut();
//     }
// }

/// A [WasiModule] is a module instance for the WASI specification.
#[cfg(not(feature = "async"))]
#[derive(Debug, Clone)]
pub struct WasiModule {
    pub(crate) inner: Arc<InnerInstance>,
    pub(crate) registered: bool,
    funcs: Vec<Function>,
}
#[cfg(not(feature = "async"))]
impl Drop for WasiModule {
    fn drop(&mut self) {
        if !self.registered && Arc::strong_count(&self.inner) == 1 && !self.inner.0.is_null() {
            // free the module instance
            unsafe {
                ffi::WasmEdge_ModuleInstanceDelete(self.inner.0);
            }

            // drop the registered host functions
            self.funcs.drain(..);
        }
    }
}
#[cfg(not(feature = "async"))]
impl WasiModule {
    /// Creates a WASI host module which contains the WASI host functions, and initializes it with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `args` - The commandline arguments. The first argument is the program name.
    ///
    /// * `envs` - The environment variables in the format `ENV_VAR_NAME=VALUE`.
    ///
    /// * `preopens` - The directories to pre-open. The required format is `DIR1:DIR2`.
    ///
    /// # Error
    ///
    /// If fail to create a host module, then an error is returned.
    pub fn create(
        args: Option<Vec<&str>>,
        envs: Option<Vec<&str>>,
        preopens: Option<Vec<&str>>,
    ) -> WasmEdgeResult<Self> {
        // parse args
        let cstr_args: Vec<_> = match args {
            Some(args) => args
                .iter()
                .map(|&x| std::ffi::CString::new(x).unwrap())
                .collect(),
            None => vec![],
        };
        let mut p_args: Vec<_> = cstr_args.iter().map(|x| x.as_ptr()).collect();
        let p_args_len = p_args.len();
        p_args.push(std::ptr::null());

        // parse envs
        let cstr_envs: Vec<_> = match envs {
            Some(envs) => envs
                .iter()
                .map(|&x| std::ffi::CString::new(x).unwrap())
                .collect(),
            None => vec![],
        };
        let mut p_envs: Vec<_> = cstr_envs.iter().map(|x| x.as_ptr()).collect();
        let p_envs_len = p_envs.len();
        p_envs.push(std::ptr::null());

        // parse preopens
        let cstr_preopens: Vec<_> = match preopens {
            Some(preopens) => preopens
                .iter()
                .map(|&x| std::ffi::CString::new(x).unwrap())
                .collect(),
            None => vec![],
        };
        let mut p_preopens: Vec<_> = cstr_preopens.iter().map(|x| x.as_ptr()).collect();
        let p_preopens_len = p_preopens.len();
        p_preopens.push(std::ptr::null());

        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceCreateWASI(
                p_args.as_ptr(),
                p_args_len as u32,
                p_envs.as_ptr(),
                p_envs_len as u32,
                p_preopens.as_ptr(),
                p_preopens_len as u32,
            )
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::ImportObjCreate)),
            false => Ok(Self {
                inner: std::sync::Arc::new(InnerInstance(ctx)),
                registered: false,
                funcs: Vec::new(),
            }),
        }
    }

    /// Initializes the WASI host module with the given parameters.
    ///
    /// # Arguments
    ///
    /// * `args` - The commandline arguments. The first argument is the program name.
    ///
    /// * `envs` - The environment variables in the format `ENV_VAR_NAME=VALUE`.
    ///
    /// * `preopens` - The directories to pre-open. The required format is `DIR1:DIR2`.
    pub fn init_wasi(
        &mut self,
        args: Option<Vec<&str>>,
        envs: Option<Vec<&str>>,
        preopens: Option<Vec<&str>>,
    ) {
        // parse args
        let cstr_args: Vec<_> = match args {
            Some(args) => args
                .iter()
                .map(|&x| std::ffi::CString::new(x).unwrap())
                .collect(),
            None => vec![],
        };
        let mut p_args: Vec<_> = cstr_args.iter().map(|x| x.as_ptr()).collect();
        let p_args_len = p_args.len();
        p_args.push(std::ptr::null());

        // parse envs
        let cstr_envs: Vec<_> = match envs {
            Some(envs) => envs
                .iter()
                .map(|&x| std::ffi::CString::new(x).unwrap())
                .collect(),
            None => vec![],
        };
        let mut p_envs: Vec<_> = cstr_envs.iter().map(|x| x.as_ptr()).collect();
        let p_envs_len = p_envs.len();
        p_envs.push(std::ptr::null());

        // parse preopens
        let cstr_preopens: Vec<_> = match preopens {
            Some(preopens) => preopens
                .iter()
                .map(|&x| std::ffi::CString::new(x).unwrap())
                .collect(),
            None => vec![],
        };
        let mut p_preopens: Vec<_> = cstr_preopens.iter().map(|x| x.as_ptr()).collect();
        let p_preopens_len = p_preopens.len();
        p_preopens.push(std::ptr::null());

        unsafe {
            ffi::WasmEdge_ModuleInstanceInitWASI(
                self.inner.0,
                p_args.as_ptr(),
                p_args_len as u32,
                p_envs.as_ptr(),
                p_envs_len as u32,
                p_preopens.as_ptr(),
                p_preopens_len as u32,
            )
        };
    }

    /// Returns the WASI exit code.
    ///
    /// The WASI exit code can be accessed after running the "_start" function of a `wasm32-wasi` program.
    pub fn exit_code(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceWASIGetExitCode(self.inner.0 as *const _) }
    }

    /// Returns the native handler from the mapped FD/Handler.
    ///
    /// # Argument
    ///
    /// * `fd` - The WASI mapped Fd.
    ///
    /// # Error
    ///
    /// If fail to get the native handler, then an error is returned.
    pub fn get_native_handler(&self, fd: i32) -> WasmEdgeResult<u64> {
        let mut handler: u64 = 0;
        let code: u32 = unsafe {
            ffi::WasmEdge_ModuleInstanceWASIGetNativeHandler(
                self.inner.0 as *const _,
                fd,
                &mut handler as *mut u64,
            )
        };

        match code {
            0 => Ok(handler),
            _ => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundMappedFdHandler,
            ))),
        }
    }

    /// Provides a raw pointer to the inner module instance context.
    #[cfg(feature = "ffi")]
    pub fn as_ptr(&self) -> *const ffi::WasmEdge_ModuleInstanceContext {
        self.inner.0 as *const _
    }
}
#[cfg(not(feature = "async"))]
impl AsInstance for WasiModule {
    fn get_func(&self, name: impl AsRef<str>) -> WasmEdgeResult<Function> {
        let func_name: WasmEdgeString = name.as_ref().into();
        let func_ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindFunction(self.inner.0 as *const _, func_name.as_raw())
        };
        match func_ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundFunc(name.as_ref().to_string()),
            ))),
            false => Ok(Function {
                inner: Arc::new(Mutex::new(InnerFunc(func_ctx))),
                registered: true,
            }),
        }
    }

    fn get_table(&self, name: impl AsRef<str>) -> WasmEdgeResult<Table> {
        let table_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindTable(self.inner.0 as *const _, table_name.as_raw())
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundTable(name.as_ref().to_string()),
            ))),
            false => Ok(Table {
                inner: Arc::new(Mutex::new(InnerTable(ctx))),
                registered: true,
            }),
        }
    }

    fn get_memory(&self, name: impl AsRef<str>) -> WasmEdgeResult<Memory> {
        let mem_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindMemory(self.inner.0 as *const _, mem_name.as_raw())
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundMem(name.as_ref().to_string()),
            ))),
            false => Ok(Memory {
                inner: Arc::new(Mutex::new(InnerMemory(ctx))),
                registered: true,
            }),
        }
    }

    fn get_global(&self, name: impl AsRef<str>) -> WasmEdgeResult<Global> {
        let global_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindGlobal(self.inner.0 as *const _, global_name.as_raw())
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundGlobal(name.as_ref().to_string()),
            ))),
            false => Ok(Global {
                inner: Arc::new(Mutex::new(InnerGlobal(ctx))),
                registered: true,
            }),
        }
    }

    /// Returns the length of the exported [function instances](crate::Function) in this module instance.
    fn func_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListFunctionLength(self.inner.0) }
    }

    /// Returns the names of the exported [function instances](crate::Function) in this module instance.
    fn func_names(&self) -> Option<Vec<String>> {
        let len_func_names = self.func_len();
        match len_func_names > 0 {
            true => {
                let mut func_names = Vec::with_capacity(len_func_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListFunction(
                        self.inner.0,
                        func_names.as_mut_ptr(),
                        len_func_names,
                    );
                    func_names.set_len(len_func_names as usize);
                }

                let names = func_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [table instances](crate::Table) in this module instance.
    fn table_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListTableLength(self.inner.0) }
    }

    /// Returns the names of the exported [table instances](crate::Table) in this module instance.
    fn table_names(&self) -> Option<Vec<String>> {
        let len_table_names = self.table_len();
        match len_table_names > 0 {
            true => {
                let mut table_names = Vec::with_capacity(len_table_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListTable(
                        self.inner.0,
                        table_names.as_mut_ptr(),
                        len_table_names,
                    );
                    table_names.set_len(len_table_names as usize);
                }

                let names = table_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [memory instances](crate::Memory) in this module instance.
    fn mem_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListMemoryLength(self.inner.0) }
    }

    /// Returns the names of all exported [memory instances](crate::Memory) in this module instance.
    fn mem_names(&self) -> Option<Vec<String>> {
        let len_mem_names = self.mem_len();
        match len_mem_names > 0 {
            true => {
                let mut mem_names = Vec::with_capacity(len_mem_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListMemory(
                        self.inner.0,
                        mem_names.as_mut_ptr(),
                        len_mem_names,
                    );
                    mem_names.set_len(len_mem_names as usize);
                }

                let names = mem_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [global instances](crate::Global) in this module instance.
    fn global_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListGlobalLength(self.inner.0) }
    }

    /// Returns the names of the exported [global instances](crate::Global) in this module instance.
    fn global_names(&self) -> Option<Vec<String>> {
        let len_global_names = self.global_len();
        match len_global_names > 0 {
            true => {
                let mut global_names = Vec::with_capacity(len_global_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListGlobal(
                        self.inner.0,
                        global_names.as_mut_ptr(),
                        len_global_names,
                    );
                    global_names.set_len(len_global_names as usize);
                }

                let names = global_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }
}
#[cfg(not(feature = "async"))]
impl AsImport for WasiModule {
    fn name(&self) -> &str {
        "wasi_snapshot_preview1"
    }

    fn add_func(&mut self, name: impl AsRef<str>, func: Function) {
        self.funcs.push(func);
        let f = self.funcs.last_mut().unwrap();

        let func_name: WasmEdgeString = name.into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddFunction(
                self.inner.0,
                func_name.as_raw(),
                f.inner.lock().0,
            );
        }
    }

    fn add_table(&mut self, name: impl AsRef<str>, table: Table) {
        let table_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddTable(
                self.inner.0,
                table_name.as_raw(),
                table.inner.lock().0,
            );
        }

        table.inner.lock().0 = std::ptr::null_mut();
    }

    fn add_memory(&mut self, name: impl AsRef<str>, memory: Memory) {
        let mem_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddMemory(
                self.inner.0,
                mem_name.as_raw(),
                memory.inner.lock().0,
            );
        }

        memory.inner.lock().0 = std::ptr::null_mut();
    }

    fn add_global(&mut self, name: impl AsRef<str>, global: Global) {
        let global_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddGlobal(
                self.inner.0,
                global_name.as_raw(),
                global.inner.lock().0,
            );
        }

        global.inner.lock().0 = std::ptr::null_mut();
    }
}

/// A [AsyncWasiModule] is a module instance for the WASI specification and used in the `async` scenario.
#[cfg(all(feature = "async", target_os = "linux"))]
#[derive(Debug, Clone)]
pub struct AsyncWasiModule {
    pub(crate) inner: Arc<InnerInstance>,
    pub(crate) registered: bool,
    name: String,
    wasi_ctx: Arc<Mutex<WasiCtx>>,
    funcs: Vec<Function>,
}
#[cfg(all(feature = "async", target_os = "linux"))]
impl Drop for AsyncWasiModule {
    fn drop(&mut self) {
        if !self.registered && Arc::strong_count(&self.inner) == 1 && !self.inner.0.is_null() {
            // free the module instance
            unsafe {
                ffi::WasmEdge_ModuleInstanceDelete(self.inner.0);
            }

            // drop the registered host functions
            self.funcs.drain(..);
        }
    }
}
#[cfg(all(feature = "async", target_os = "linux"))]
impl AsyncWasiModule {
    pub fn create(
        args: Option<Vec<&str>>,
        envs: Option<Vec<(&str, &str)>>,
        preopens: Option<Vec<(PathBuf, PathBuf)>>,
    ) -> WasmEdgeResult<Self> {
        // create wasi context
        let mut wasi_ctx = WasiCtx::new();
        if let Some(args) = args {
            wasi_ctx.push_args(args.iter().map(|x| x.to_string()).collect());
        }
        if let Some(envs) = envs {
            wasi_ctx.push_envs(envs.iter().map(|(k, v)| format!("{}={}", k, v)).collect());
        }
        if let Some(preopens) = preopens {
            for (host_dir, guest_dir) in preopens {
                wasi_ctx.push_preopen(host_dir, guest_dir)
            }
        }

        // create wasi module
        let name = "wasi_snapshot_preview1";
        let raw_name = WasmEdgeString::from(name);
        let ctx = unsafe { ffi::WasmEdge_ModuleInstanceCreate(raw_name.as_raw()) };
        if ctx.is_null() {
            return Err(Box::new(WasmEdgeError::Instance(
                InstanceError::CreateImportModule,
            )));
        }
        let mut async_wasi_module = Self {
            inner: std::sync::Arc::new(InnerInstance(ctx)),
            registered: false,
            name: name.to_string(),
            wasi_ctx: Arc::new(Mutex::new(wasi_ctx)),
            funcs: Vec::new(),
        };

        // add sync/async host functions to the module
        for wasi_func in wasi_impls() {
            match wasi_func {
                WasiFunc::SyncFn(name, (ty_args, ty_rets), real_fn) => {
                    let func_ty = crate::FuncType::create(ty_args, ty_rets)?;
                    let func = Function::create_wasi_func(
                        &func_ty,
                        real_fn,
                        Some(&mut async_wasi_module.wasi_ctx.lock()),
                        0,
                    )?;
                    async_wasi_module.add_wasi_func(name, func);
                }
                WasiFunc::AsyncFn(name, (ty_args, ty_rets), real_async_fn) => {
                    let func_ty = crate::FuncType::create(ty_args, ty_rets)?;
                    let func = Function::create_async_wasi_func(
                        &func_ty,
                        real_async_fn,
                        Some(&mut async_wasi_module.wasi_ctx.lock()),
                        0,
                    )?;
                    async_wasi_module.add_wasi_func(name, func);
                }
            }
        }

        Ok(async_wasi_module)
    }

    fn add_wasi_func(&mut self, name: impl AsRef<str>, func: Function) {
        let func_name: WasmEdgeString = name.into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddFunction(
                self.inner.0,
                func_name.as_raw(),
                func.inner.lock().0,
            );
        }

        func.inner.lock().0 = std::ptr::null_mut();
    }

    pub fn init_wasi(
        &mut self,
        args: Option<Vec<&str>>,
        envs: Option<Vec<(&str, &str)>>,
        preopens: Option<Vec<(PathBuf, PathBuf)>>,
    ) -> WasmEdgeResult<()> {
        // create wasi context
        let mut wasi_ctx = WasiCtx::new();
        if let Some(args) = args {
            wasi_ctx.push_args(args.iter().map(|x| x.to_string()).collect());
        }
        if let Some(envs) = envs {
            wasi_ctx.push_envs(envs.iter().map(|(k, v)| format!("{}={}", k, v)).collect());
        }
        if let Some(preopens) = preopens {
            for (host_dir, guest_dir) in preopens {
                wasi_ctx.push_preopen(host_dir, guest_dir)
            }
        }

        self.wasi_ctx = Arc::new(Mutex::new(wasi_ctx));

        // add sync/async host functions to the module
        for wasi_func in wasi_impls() {
            match wasi_func {
                WasiFunc::SyncFn(name, (ty_args, ty_rets), real_fn) => {
                    let func_ty = crate::FuncType::create(ty_args, ty_rets)?;
                    let func = Function::create_wasi_func(
                        &func_ty,
                        real_fn,
                        Some(&mut self.wasi_ctx.lock()),
                        0,
                    )?;
                    self.add_wasi_func(name, func);
                }
                WasiFunc::AsyncFn(name, (ty_args, ty_rets), real_async_fn) => {
                    let func_ty = crate::FuncType::create(ty_args, ty_rets)?;
                    let func = Function::create_async_wasi_func(
                        &func_ty,
                        real_async_fn,
                        Some(&mut self.wasi_ctx.lock()),
                        0,
                    )?;
                    self.add_wasi_func(name, func);
                }
            }
        }

        Ok(())
    }

    /// Returns the WASI exit code.
    ///
    /// The WASI exit code can be accessed after running the "_start" function of a `wasm32-wasi` program.
    pub fn exit_code(&self) -> u32 {
        self.wasi_ctx.lock().exit_code
    }
}
#[cfg(all(feature = "async", target_os = "linux"))]
impl AsInstance for AsyncWasiModule {
    fn get_func(&self, name: impl AsRef<str>) -> WasmEdgeResult<Function> {
        let func_name: WasmEdgeString = name.as_ref().into();
        let func_ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindFunction(self.inner.0 as *const _, func_name.as_raw())
        };
        match func_ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundFunc(name.as_ref().to_string()),
            ))),
            false => Ok(Function {
                inner: Arc::new(Mutex::new(InnerFunc(func_ctx))),
                registered: true,
            }),
        }
    }

    fn get_table(&self, name: impl AsRef<str>) -> WasmEdgeResult<Table> {
        let table_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindTable(self.inner.0 as *const _, table_name.as_raw())
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundTable(name.as_ref().to_string()),
            ))),
            false => Ok(Table {
                inner: Arc::new(Mutex::new(InnerTable(ctx))),
                registered: true,
            }),
        }
    }

    fn get_memory(&self, name: impl AsRef<str>) -> WasmEdgeResult<Memory> {
        let mem_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindMemory(self.inner.0 as *const _, mem_name.as_raw())
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundMem(name.as_ref().to_string()),
            ))),
            false => Ok(Memory {
                inner: Arc::new(Mutex::new(InnerMemory(ctx))),
                registered: true,
            }),
        }
    }

    fn get_global(&self, name: impl AsRef<str>) -> WasmEdgeResult<Global> {
        let global_name: WasmEdgeString = name.as_ref().into();
        let ctx = unsafe {
            ffi::WasmEdge_ModuleInstanceFindGlobal(self.inner.0 as *const _, global_name.as_raw())
        };
        match ctx.is_null() {
            true => Err(Box::new(WasmEdgeError::Instance(
                InstanceError::NotFoundGlobal(name.as_ref().to_string()),
            ))),
            false => Ok(Global {
                inner: Arc::new(Mutex::new(InnerGlobal(ctx))),
                registered: true,
            }),
        }
    }

    /// Returns the length of the exported [function instances](crate::Function) in this module instance.
    fn func_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListFunctionLength(self.inner.0) }
    }

    /// Returns the names of the exported [function instances](crate::Function) in this module instance.
    fn func_names(&self) -> Option<Vec<String>> {
        let len_func_names = self.func_len();
        match len_func_names > 0 {
            true => {
                let mut func_names = Vec::with_capacity(len_func_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListFunction(
                        self.inner.0,
                        func_names.as_mut_ptr(),
                        len_func_names,
                    );
                    func_names.set_len(len_func_names as usize);
                }

                let names = func_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [table instances](crate::Table) in this module instance.
    fn table_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListTableLength(self.inner.0) }
    }

    /// Returns the names of the exported [table instances](crate::Table) in this module instance.
    fn table_names(&self) -> Option<Vec<String>> {
        let len_table_names = self.table_len();
        match len_table_names > 0 {
            true => {
                let mut table_names = Vec::with_capacity(len_table_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListTable(
                        self.inner.0,
                        table_names.as_mut_ptr(),
                        len_table_names,
                    );
                    table_names.set_len(len_table_names as usize);
                }

                let names = table_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [memory instances](crate::Memory) in this module instance.
    fn mem_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListMemoryLength(self.inner.0) }
    }

    /// Returns the names of all exported [memory instances](crate::Memory) in this module instance.
    fn mem_names(&self) -> Option<Vec<String>> {
        let len_mem_names = self.mem_len();
        match len_mem_names > 0 {
            true => {
                let mut mem_names = Vec::with_capacity(len_mem_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListMemory(
                        self.inner.0,
                        mem_names.as_mut_ptr(),
                        len_mem_names,
                    );
                    mem_names.set_len(len_mem_names as usize);
                }

                let names = mem_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }

    /// Returns the length of the exported [global instances](crate::Global) in this module instance.
    fn global_len(&self) -> u32 {
        unsafe { ffi::WasmEdge_ModuleInstanceListGlobalLength(self.inner.0) }
    }

    /// Returns the names of the exported [global instances](crate::Global) in this module instance.
    fn global_names(&self) -> Option<Vec<String>> {
        let len_global_names = self.global_len();
        match len_global_names > 0 {
            true => {
                let mut global_names = Vec::with_capacity(len_global_names as usize);
                unsafe {
                    ffi::WasmEdge_ModuleInstanceListGlobal(
                        self.inner.0,
                        global_names.as_mut_ptr(),
                        len_global_names,
                    );
                    global_names.set_len(len_global_names as usize);
                }

                let names = global_names
                    .into_iter()
                    .map(|x| x.into())
                    .collect::<Vec<String>>();
                Some(names)
            }
            false => None,
        }
    }
}
#[cfg(all(feature = "async", target_os = "linux"))]
impl AsImport for AsyncWasiModule {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn add_func(&mut self, name: impl AsRef<str>, func: Function) {
        self.funcs.push(func);
        let f = self.funcs.last_mut().unwrap();

        let func_name: WasmEdgeString = name.into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddFunction(
                self.inner.0,
                func_name.as_raw(),
                f.inner.lock().0,
            );
        }
    }

    fn add_table(&mut self, name: impl AsRef<str>, table: Table) {
        let table_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddTable(
                self.inner.0,
                table_name.as_raw(),
                table.inner.lock().0,
            );
        }
        table.inner.lock().0 = std::ptr::null_mut();
    }

    fn add_memory(&mut self, name: impl AsRef<str>, memory: Memory) {
        let mem_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddMemory(
                self.inner.0,
                mem_name.as_raw(),
                memory.inner.lock().0,
            );
        }
        memory.inner.lock().0 = std::ptr::null_mut();
    }

    fn add_global(&mut self, name: impl AsRef<str>, global: Global) {
        let global_name: WasmEdgeString = name.as_ref().into();
        unsafe {
            ffi::WasmEdge_ModuleInstanceAddGlobal(
                self.inner.0,
                global_name.as_raw(),
                global.inner.lock().0,
            );
        }
        global.inner.lock().0 = std::ptr::null_mut();
    }
}

/// The object to be registered via the the [Executor::register_import_object](crate::Executor::register_import_object) function is required to implement this trait.
pub trait AsImport {
    /// Returns the name of the module instance.
    fn name(&self) -> &str;

    /// Imports a [host function instance](crate::Function).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the host function instance to import.
    ///
    /// * `func` - The host function instance to import.
    fn add_func(&mut self, name: impl AsRef<str>, func: Function);

    /// Imports a [table instance](crate::Table).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the host table instance to import.
    ///
    /// * `table` - The host table instance to import.
    fn add_table(&mut self, name: impl AsRef<str>, table: Table);

    /// Imports a [memory instance](crate::Memory).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the host memory instance to import.
    ///
    /// * `memory` - The host memory instance to import.
    fn add_memory(&mut self, name: impl AsRef<str>, memory: Memory);

    /// Imports a [global instance](crate::Global).
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the host global instance to import.
    ///
    /// * `global` - The host global instance to import.
    fn add_global(&mut self, name: impl AsRef<str>, global: Global);
}

/// Defines three types of module instances that can be imported into a WasmEdge [Store](crate::Store) instance.
#[derive(Debug, Clone)]
pub enum ImportObject<T: Send + Sync + Clone> {
    /// Defines the import module instance of ImportModule type.
    Import(ImportModule<T>),
    /// Defines the import module instance of WasiModule type.
    #[cfg(not(feature = "async"))]
    Wasi(WasiModule),
    /// Defines the import module instance of AsyncWasiModule type.
    #[cfg(all(feature = "async", target_os = "linux"))]
    AsyncWasi(AsyncWasiModule),
}
impl<T: Send + Sync + Clone> ImportObject<T> {
    /// Returns the name of the import object.
    pub fn name(&self) -> &str {
        match self {
            ImportObject::Import(import) => import.name(),
            #[cfg(not(feature = "async"))]
            ImportObject::Wasi(wasi) => wasi.name(),
            #[cfg(all(feature = "async", target_os = "linux"))]
            ImportObject::AsyncWasi(async_wasi) => async_wasi.name(),
        }
    }

    /// Returns the raw pointer to the inner `WasmEdge_ModuleInstanceContext`.
    #[cfg(feature = "ffi")]
    pub fn as_raw_ptr(&self) -> *const ffi::WasmEdge_ModuleInstanceContext {
        match self {
            ImportObject::Import(import) => import.inner.0,
            #[cfg(not(feature = "async"))]
            ImportObject::Wasi(wasi) => wasi.inner.0,
            #[cfg(all(feature = "async", target_os = "linux"))]
            ImportObject::AsyncWasi(async_wasi) => async_wasi.inner.0,
        }
    }
}

pub(crate) unsafe extern "C" fn host_data_finalizer<T: Sized + Send>(
    raw: *mut ::std::os::raw::c_void,
) {
    let host_data: Box<T> = Box::from_raw(raw as *mut T);
    drop(host_data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallingFrame, Config, Executor, FuncType, GlobalType, ImportModule, MemType, Store,
        TableType, WasmValue, HOST_FUNCS, HOST_FUNC_FOOTPRINTS,
    };
    #[cfg(not(feature = "async"))]
    use std::sync::{Arc, Mutex};
    use std::thread;
    use wasmedge_macro::sys_host_function;
    use wasmedge_types::{error::HostFuncError, Mutability, NeverType, RefType, ValType};

    #[test]
    // #[cfg(not(feature = "async"))]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_add_instance() {
        assert_eq!(HOST_FUNCS.read().len(), 0);
        assert_eq!(HOST_FUNC_FOOTPRINTS.lock().len(), 0);

        let host_name = "extern";

        // create an import module
        let result = ImportModule::<NeverType>::create(host_name, None);
        assert!(result.is_ok());
        let mut import = result.unwrap();

        // create a host function
        let result = FuncType::create([ValType::ExternRef, ValType::I32], [ValType::I32]);
        assert!(result.is_ok());
        let func_ty = result.unwrap();

        assert_eq!(HOST_FUNCS.read().len(), 0);
        assert_eq!(HOST_FUNC_FOOTPRINTS.lock().len(), 0);

        // add the host function
        let result = import.add_func_new("func-add", &func_ty, Box::new(real_add), 0);
        assert!(result.is_ok());

        assert_eq!(HOST_FUNCS.read().len(), 1);
        assert_eq!(HOST_FUNC_FOOTPRINTS.lock().len(), 1);

        // create a table
        let result = TableType::create(RefType::FuncRef, 10, Some(20));
        assert!(result.is_ok());
        let table_ty = result.unwrap();
        // add the table
        let result = import.add_table_new("table", &table_ty);
        assert!(result.is_ok());

        // create a memory
        let result = MemType::create(1, Some(2), false);
        assert!(result.is_ok());
        let mem_ty = result.unwrap();
        // add the memory
        let result = import.add_memory_new("memory", &mem_ty);
        assert!(result.is_ok());

        // create a global
        let result = GlobalType::create(ValType::I32, Mutability::Const);
        assert!(result.is_ok());
        let global_ty = result.unwrap();
        // add the global
        let result = import.add_global_new("global_i32", &global_ty, WasmValue::from_i32(666));
        assert!(result.is_ok());
    }

    #[test]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_import_module_send() {
        let host_name = "extern";

        // create an ImportModule instance
        let result = ImportModule::<NeverType>::create(host_name, None);
        assert!(result.is_ok());
        let import = result.unwrap();

        let handle = thread::spawn(move || {
            assert!(!import.inner.0.is_null());
            println!("{:?}", import.inner);
        });

        handle.join().unwrap();
    }

    #[test]
    #[cfg(not(feature = "async"))]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_import_module_sync() {
        let host_name = "extern";

        // create an ImportModule instance
        let result = ImportModule::<NeverType>::create(host_name, None);
        assert!(result.is_ok());
        let mut import = result.unwrap();

        // add host function
        let result = FuncType::create(vec![ValType::I32; 2], vec![ValType::I32]);
        assert!(result.is_ok());
        let func_ty = result.unwrap();
        let result = import.add_func_new("add", &func_ty, Box::new(real_add), 0);
        assert!(result.is_ok());

        // add table
        let result = TableType::create(RefType::FuncRef, 0, Some(u32::MAX));
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_table_new("table", &ty);
        assert!(result.is_ok());

        // add memory
        let result = MemType::create(10, Some(20), false);
        assert!(result.is_ok());
        let mem_ty = result.unwrap();
        let result = import.add_memory_new("memory", &mem_ty);
        assert!(result.is_ok());

        // add globals
        let result = GlobalType::create(ValType::F32, Mutability::Const);
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_global_new("global", &ty, WasmValue::from_f32(3.5));
        assert!(result.is_ok());

        let import = ImportObject::Import(import);
        let import = Arc::new(Mutex::new(import));
        let import_cloned = Arc::clone(&import);
        let handle = thread::spawn(move || {
            let result = import_cloned.lock();
            assert!(result.is_ok());
            let import = result.unwrap();

            // create a store
            let result = Store::create();
            assert!(result.is_ok());
            let mut store = result.unwrap();
            assert!(!store.inner.0.is_null());
            assert!(!store.registered);

            // create an executor
            let result = Config::create();
            assert!(result.is_ok());
            let config = result.unwrap();
            let result = Executor::create(Some(&config), None);
            assert!(result.is_ok());
            let mut executor = result.unwrap();

            // register import object into store
            let result = executor.register_import_object(&mut store, &import);
            assert!(result.is_ok());

            // get the exported module by name
            let result = store.module("extern");
            assert!(result.is_ok());
            let instance = result.unwrap();

            // get the exported function by name
            let result = instance.get_func("add");
            assert!(result.is_ok());

            // get the exported global by name
            let result = instance.get_global("global");
            assert!(result.is_ok());
            let global = result.unwrap();
            assert!(!global.inner.lock().0.is_null() && global.registered);
            let val = global.get_value();
            assert_eq!(val.to_f32(), 3.5);

            // get the exported memory by name
            let result = instance.get_memory("memory");
            assert!(result.is_ok());
            let memory = result.unwrap();
            let result = memory.ty();
            assert!(result.is_ok());
            let ty = result.unwrap();
            assert_eq!(ty.min(), 10);
            assert_eq!(ty.max(), Some(20));

            // get the exported table by name
            let result = instance.get_table("table");
            assert!(result.is_ok());
        });

        handle.join().unwrap();
    }

    #[cfg(all(not(feature = "async"), target_family = "unix"))]
    #[test]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_wasi() {
        // create a wasi module instance
        {
            let result = WasiModule::create(None, None, None);
            assert!(result.is_ok());

            let result = WasiModule::create(
                Some(vec!["arg1", "arg2"]),
                Some(vec!["ENV1=VAL1", "ENV1=VAL2", "ENV3=VAL3"]),
                Some(vec![
                    "apiTestData",
                    "Makefile",
                    "CMakeFiles",
                    "ssvmAPICoreTests",
                    ".:.",
                ]),
            );
            assert!(result.is_ok());

            let result = WasiModule::create(
                None,
                Some(vec!["ENV1=VAL1", "ENV1=VAL2", "ENV3=VAL3"]),
                Some(vec![
                    "apiTestData",
                    "Makefile",
                    "CMakeFiles",
                    "ssvmAPICoreTests",
                    ".:.",
                ]),
            );
            assert!(result.is_ok());
            let wasi_import = result.unwrap();
            assert_eq!(wasi_import.exit_code(), 0);
        }
    }

    #[test]
    #[cfg(not(feature = "async"))]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_find_xxx() -> Result<(), Box<dyn std::error::Error>> {
        let module_name = "extern_module";

        // create ImportModule instance
        let result = ImportModule::<NeverType>::create(module_name, None);
        assert!(result.is_ok());
        let mut import = result.unwrap();

        // add host function
        let result = FuncType::create(vec![ValType::I32; 2], vec![ValType::I32]);
        assert!(result.is_ok());
        let func_ty = result.unwrap();
        let result = import.add_func_new("add", &func_ty, Box::new(real_add), 0);
        assert!(result.is_ok());

        // add table
        let result = TableType::create(RefType::FuncRef, 0, Some(u32::MAX));
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_table_new("table", &ty);
        assert!(result.is_ok());

        // add memory
        let result = MemType::create(0, Some(u32::MAX), false);
        assert!(result.is_ok());
        let mem_ty = result.unwrap();
        let result: Result<(), Box<WasmEdgeError>> = import.add_memory_new("mem", &mem_ty);
        assert!(result.is_ok());

        // add global
        let result = GlobalType::create(ValType::F32, Mutability::Const);
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_global_new("global", &ty, WasmValue::from_f32(3.5));
        assert!(result.is_ok());

        // create an executor
        let mut executor = Executor::create(None, None)?;

        // create a store
        let mut store = Store::create()?;

        let import_obj = ImportObject::Import(import);
        executor.register_import_object(&mut store, &import_obj)?;

        // get the module named "extern"
        let result = store.module("extern_module");
        assert!(result.is_ok());
        let instance = result.unwrap();

        // check the name of the module
        assert!(instance.name().is_some());
        assert_eq!(instance.name().unwrap(), "extern_module");

        // get the exported function named "fib"
        let result = instance.get_func("add");
        assert!(result.is_ok());
        let func = result.unwrap();

        // check the type of the function
        let result = func.ty();
        assert!(result.is_ok());
        let ty = result.unwrap();

        // check the parameter types
        let param_types = ty.params_type_iter().collect::<Vec<ValType>>();
        assert_eq!(param_types, [ValType::I32, ValType::I32]);

        // check the return types
        let return_types = ty.returns_type_iter().collect::<Vec<ValType>>();
        assert_eq!(return_types, [ValType::I32]);

        // get the exported table named "table"
        let result = instance.get_table("table");
        assert!(result.is_ok());
        let table = result.unwrap();

        // check the type of the table
        let result = table.ty();
        assert!(result.is_ok());
        let ty = result.unwrap();
        assert_eq!(ty.elem_ty(), RefType::FuncRef);
        assert_eq!(ty.min(), 0);
        assert_eq!(ty.max(), Some(u32::MAX));

        // get the exported memory named "mem"
        let result = instance.get_memory("mem");
        assert!(result.is_ok());
        let memory = result.unwrap();

        // check the type of the memory
        let result = memory.ty();
        assert!(result.is_ok());
        let ty = result.unwrap();
        assert_eq!(ty.min(), 0);
        assert_eq!(ty.max(), Some(u32::MAX));

        // get the exported global named "global"
        let result = instance.get_global("global");
        assert!(result.is_ok());
        let global = result.unwrap();

        // check the type of the global
        let result = global.ty();
        assert!(result.is_ok());
        let global = result.unwrap();
        assert_eq!(global.value_type(), ValType::F32);
        assert_eq!(global.mutability(), Mutability::Const);

        Ok(())
    }

    #[test]
    #[cfg(not(feature = "async"))]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_find_names() -> Result<(), Box<dyn std::error::Error>> {
        let module_name = "extern_module";

        // create ImportModule instance
        let result = ImportModule::<NeverType>::create(module_name, None);
        assert!(result.is_ok());
        let mut import = result.unwrap();

        // add host function
        let result = FuncType::create(vec![ValType::I32; 2], vec![ValType::I32]);
        assert!(result.is_ok());
        let func_ty = result.unwrap();
        let result = import.add_func_new("add", &func_ty, Box::new(real_add), 0);
        assert!(result.is_ok());

        // add table
        let result = TableType::create(RefType::FuncRef, 0, Some(u32::MAX));
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_table_new("table", &ty);
        assert!(result.is_ok());

        // add memory
        let result = MemType::create(0, Some(u32::MAX), false);
        assert!(result.is_ok());
        let mem_ty = result.unwrap();
        let result = import.add_memory_new("mem", &mem_ty);
        assert!(result.is_ok());

        // add global
        let result = GlobalType::create(ValType::F32, Mutability::Const);
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_global_new("global", &ty, WasmValue::from_f32(3.5));
        assert!(result.is_ok());

        // create an executor
        let mut executor = Executor::create(None, None)?;

        // create a store
        let mut store = Store::create()?;

        let import_obj = ImportObject::Import(import);
        executor.register_import_object(&mut store, &import_obj)?;

        // get the module named "extern"
        let result = store.module("extern_module");
        assert!(result.is_ok());
        let instance = result.unwrap();

        // check the name of the module
        assert!(instance.name().is_some());
        assert_eq!(instance.name().unwrap(), "extern_module");

        assert_eq!(instance.func_len(), 1);
        let result = instance.func_names();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), ["add"]);

        assert_eq!(instance.table_len(), 1);
        let result = instance.table_names();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), ["table"]);

        assert_eq!(instance.mem_len(), 1);
        let result = instance.mem_names();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), ["mem"]);

        assert_eq!(instance.global_len(), 1);
        let result = instance.global_names();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), ["global"]);

        Ok(())
    }

    #[test]
    #[cfg(not(feature = "async"))]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_get() {
        let module_name = "extern_module";

        let result = Store::create();
        assert!(result.is_ok());
        let mut store = result.unwrap();
        assert!(!store.inner.0.is_null());
        assert!(!store.registered);

        // check the length of registered module list in store before instantiation
        assert_eq!(store.module_len(), 0);
        assert!(store.module_names().is_none());

        // create ImportObject instance
        let result = ImportModule::<NeverType>::create(module_name, None);
        assert!(result.is_ok());
        let mut import = result.unwrap();

        // add host function
        let result = FuncType::create(vec![ValType::I32; 2], vec![ValType::I32]);
        assert!(result.is_ok());
        let func_ty = result.unwrap();
        let result = import.add_func_new("add", &func_ty, Box::new(real_add), 0);
        assert!(result.is_ok());

        // add table
        let result = TableType::create(RefType::FuncRef, 0, Some(u32::MAX));
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_table_new("table", &ty);
        assert!(result.is_ok());

        // add memory
        let result = MemType::create(10, Some(20), false);
        assert!(result.is_ok());
        let mem_ty = result.unwrap();
        let result = import.add_memory_new("mem", &mem_ty);
        assert!(result.is_ok());

        // add globals
        let result = GlobalType::create(ValType::F32, Mutability::Const);
        assert!(result.is_ok());
        let ty = result.unwrap();
        let result = import.add_global_new("global", &ty, WasmValue::from_f32(3.5));
        assert!(result.is_ok());

        let result = Config::create();
        assert!(result.is_ok());
        let config = result.unwrap();
        let result = Executor::create(Some(&config), None);
        assert!(result.is_ok());
        let mut executor = result.unwrap();

        let import = ImportObject::Import(import);
        let result = executor.register_import_object(&mut store, &import);
        assert!(result.is_ok());

        let result = store.module(module_name);
        assert!(result.is_ok());
        let mut instance = result.unwrap();

        // get the exported memory
        let result = instance.get_memory("mem");
        assert!(result.is_ok());
        let memory = result.unwrap();
        let result = memory.ty();
        assert!(result.is_ok());
        let ty = result.unwrap();
        assert_eq!(ty.min(), 10);
        assert_eq!(ty.max(), Some(20));

        // get host data
        assert!(instance.host_data::<NeverType>().is_none());
    }

    #[sys_host_function]
    fn real_add(
        _frame: CallingFrame,
        inputs: Vec<WasmValue>,
    ) -> Result<Vec<WasmValue>, HostFuncError> {
        if inputs.len() != 2 {
            return Err(HostFuncError::User(1));
        }

        let a = if inputs[0].ty() == ValType::I32 {
            inputs[0].to_i32()
        } else {
            return Err(HostFuncError::User(2));
        };

        let b = if inputs[1].ty() == ValType::I32 {
            inputs[1].to_i32()
        } else {
            return Err(HostFuncError::User(3));
        };

        let c = a + b;

        Ok(vec![WasmValue::from_i32(c)])
    }

    #[cfg(not(feature = "async"))]
    #[test]
    #[allow(clippy::assertions_on_result_states)]
    fn test_instance_clone() {
        // clone of ImportModule
        {
            let host_name = "extern";

            // create an import module
            let result = ImportModule::<NeverType>::create(host_name, None);
            assert!(result.is_ok());
            let mut import = result.unwrap();

            // create a host function
            let result = FuncType::create([ValType::ExternRef, ValType::I32], [ValType::I32]);
            assert!(result.is_ok());
            let func_ty = result.unwrap();
            // add the host function
            let result = import.add_func_new("func-add", &func_ty, Box::new(real_add), 0);
            assert!(result.is_ok());

            // create a table
            let result = TableType::create(RefType::FuncRef, 10, Some(20));
            assert!(result.is_ok());
            let table_ty = result.unwrap();
            // add the table
            let result = import.add_table_new("table", &table_ty);
            assert!(result.is_ok());

            // create a memory
            let result = MemType::create(1, Some(2), false);
            assert!(result.is_ok());
            let mem_ty = result.unwrap();
            // add the memory
            let result = import.add_memory_new("memory", &mem_ty);
            assert!(result.is_ok());

            // create a global
            let result = GlobalType::create(ValType::I32, Mutability::Const);
            assert!(result.is_ok());
            let global_ty = result.unwrap();
            // add the global
            let result = import.add_global_new("global_i32", &global_ty, WasmValue::from_i32(666));
            assert!(result.is_ok());
            assert_eq!(Arc::strong_count(&import.inner), 1);

            // clone the import module
            let import_clone = import.clone();
            assert_eq!(Arc::strong_count(&import.inner), 2);

            drop(import);
            assert_eq!(Arc::strong_count(&import_clone.inner), 1);
            drop(import_clone);
        }

        // clone of WasiModule
        {
            let result = WasiModule::create(None, None, None);
            assert!(result.is_ok());

            let result = WasiModule::create(
                Some(vec!["arg1", "arg2"]),
                Some(vec!["ENV1=VAL1", "ENV1=VAL2", "ENV3=VAL3"]),
                Some(vec![
                    "apiTestData",
                    "Makefile",
                    "CMakeFiles",
                    "ssvmAPICoreTests",
                    ".:.",
                ]),
            );
            assert!(result.is_ok());

            let result = WasiModule::create(
                None,
                Some(vec!["ENV1=VAL1", "ENV1=VAL2", "ENV3=VAL3"]),
                Some(vec![
                    "apiTestData",
                    "Makefile",
                    "CMakeFiles",
                    "ssvmAPICoreTests",
                    ".:.",
                ]),
            );
            assert!(result.is_ok());
            let wasi_import = result.unwrap();
            assert_eq!(wasi_import.exit_code(), 0);
            assert_eq!(std::sync::Arc::strong_count(&wasi_import.inner), 1);

            // clone
            let wasi_import_clone = wasi_import.clone();
            assert_eq!(std::sync::Arc::strong_count(&wasi_import.inner), 2);

            drop(wasi_import);
            assert_eq!(std::sync::Arc::strong_count(&wasi_import_clone.inner), 1);
            drop(wasi_import_clone);
        }
    }

    #[test]
    fn test_instance_create_import_with_data() {
        let module_name = "extern_module";

        // define host data
        #[derive(Clone, Debug)]
        struct Circle {
            radius: i32,
        }

        fn real_add<T: core::fmt::Debug + Send + Sync + Clone>(
            frame: CallingFrame,
            input: Vec<WasmValue>,
            _data: *mut std::ffi::c_void,
        ) -> Result<Vec<WasmValue>, HostFuncError> {
            println!("[real_add] Rust: Entering Rust function real_add");

            let mut instance = frame
                .module_instance()
                .expect("failed to get module instance");
            let host_data = instance.host_data::<T>().expect("failed to get host data");
            println!("[real_add] host_data: {:?}", host_data);

            if input.len() != 2 {
                return Err(HostFuncError::User(1));
            }

            let a = if input[0].ty() == ValType::I32 {
                input[0].to_i32()
            } else {
                return Err(HostFuncError::User(2));
            };

            let b = if input[1].ty() == ValType::I32 {
                input[1].to_i32()
            } else {
                return Err(HostFuncError::User(3));
            };

            let c = a + b;
            println!("[real_add] Rust: calcuating in real_add c: {c:?}");

            println!("[real_add] Rust: Leaving Rust function real_add");
            Ok(vec![WasmValue::from_i32(c)])
        }

        fn hello(
            frame: CallingFrame,
            _input: Vec<WasmValue>,
            _data: *mut std::ffi::c_void,
        ) -> Result<Vec<WasmValue>, HostFuncError> {
            println!("[hello] hello");

            let mut instance = frame
                .module_instance()
                .expect("failed to get module instance");
            let circle = instance
                .host_data::<Circle>()
                .expect("failed to get host data");
            println!("[hello] radius: {}", circle.radius);

            Ok(vec![])
        }

        let circle = Circle { radius: 10 };

        // create an import module
        let result = ImportModule::create(module_name, Some(Box::new(circle)));
        assert!(result.is_ok());
        let mut import = result.unwrap();

        // add function to the import module
        let result = FuncType::create(vec![ValType::I32; 2], vec![ValType::I32]);
        assert!(result.is_ok());
        let func_ty = result.unwrap();
        let result = import.add_func_new("add", &func_ty, Box::new(real_add::<Circle>), 0);
        assert!(result.is_ok());

        let result = FuncType::create(vec![], vec![]);
        assert!(result.is_ok());
        let hello_ty = result.unwrap();
        let result = import.add_func_new("hello", &hello_ty, Box::new(hello), 0);
        assert!(result.is_ok());

        let result = Config::create();
        assert!(result.is_ok());
        let config = result.unwrap();
        let result = Executor::create(Some(&config), None);
        assert!(result.is_ok());
        let mut executor = result.unwrap();

        let result = Store::create();
        assert!(result.is_ok());
        let mut store = result.unwrap();

        let import = ImportObject::Import(import);
        let result = executor.register_import_object(&mut store, &import);
        assert!(result.is_ok());

        let result = store.module(module_name);
        assert!(result.is_ok());
        let mut instance = result.unwrap();

        let result = instance.host_data::<Circle>();
        assert!(result.is_some());
        let host_data = result.unwrap();
        assert_eq!(host_data.radius, 10);

        let fn_add = instance.get_func("add").unwrap();
        let fn_hello = instance.get_func("hello").unwrap();

        let result = executor.call_func(
            &fn_add,
            vec![WasmValue::from_i32(1), WasmValue::from_i32(2)],
        );
        assert!(result.is_ok());
        let returns = result.unwrap();
        println!("returns: {:?}", returns);

        let result = executor.call_func(&fn_hello, vec![]);
        assert!(result.is_ok());
    }
}
