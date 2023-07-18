use wasmedge_macro::sys_host_function;
use wasmedge_sys::{CallingFrame, FuncType, ImportModule, WasmValue};
use wasmedge_types::{error::HostFuncError, NeverType, ValType};

pub fn create_extern_module(name: impl AsRef<str>) -> ImportModule<NeverType> {
    // create an import module
    let result = ImportModule::<NeverType>::create(name, None);
    assert!(result.is_ok());
    let mut import = result.unwrap();

    // add host function: "func-add"
    let result = FuncType::create(vec![ValType::ExternRef, ValType::I32], vec![ValType::I32]);
    let func_ty = result.unwrap();
    let result = import.add_func_new("func-add", &func_ty, Box::new(extern_add), 0);
    assert!(result.is_ok());

    // add host function: "func-sub"
    let result = FuncType::create(vec![ValType::ExternRef, ValType::I32], vec![ValType::I32]);
    let func_ty = result.unwrap();
    let result = import.add_func_new("func-sub", &func_ty, Box::new(extern_sub), 0);
    assert!(result.is_ok());

    // add host function: "func-mul"
    let result = FuncType::create(vec![ValType::ExternRef, ValType::I32], vec![ValType::I32]);
    let func_ty = result.unwrap();
    let result = import.add_func_new("func-mul", &func_ty, Box::new(extern_mul), 0);
    assert!(result.is_ok());

    // add host function: "func-div"
    let result = FuncType::create(vec![ValType::ExternRef, ValType::I32], vec![ValType::I32]);
    let func_ty = result.unwrap();
    let result = import.add_func_new("func-div", &func_ty, Box::new(extern_div), 0);
    assert!(result.is_ok());

    // add host function: "func-term"
    let result = FuncType::create([], [ValType::I32]);
    assert!(result.is_ok());
    let func_ty = result.unwrap();
    let result = import.add_func_new("func-term", &func_ty, Box::new(extern_term), 0);
    assert!(result.is_ok());

    // add host function: "func-fail"
    let result = FuncType::create([], [ValType::I32]);
    assert!(result.is_ok());
    let func_ty = result.unwrap();
    let result = import.add_func_new("func-fail", &func_ty, Box::new(extern_fail), 0);
    assert!(result.is_ok());

    import
}

#[sys_host_function]
fn extern_add(
    _frame: CallingFrame,
    inputs: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, HostFuncError> {
    let val1 = if inputs[0].ty() == ValType::ExternRef {
        inputs[0]
    } else {
        return Err(HostFuncError::User(2));
    };
    let val1 = val1
        .extern_ref::<i32>()
        .expect("fail to get i32 from an ExternRef");

    let val2 = if inputs[1].ty() == ValType::I32 {
        inputs[1].to_i32()
    } else {
        return Err(HostFuncError::User(3));
    };

    Ok(vec![WasmValue::from_i32(val1 + val2)])
}

#[sys_host_function]
fn extern_sub(
    _frame: CallingFrame,
    inputs: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, HostFuncError> {
    let val1 = if inputs[0].ty() == ValType::ExternRef {
        inputs[0]
    } else {
        return Err(HostFuncError::User(2));
    };

    let val1 = val1
        .extern_ref::<i32>()
        .expect("fail to get i32 from an ExternRef");

    let val2 = if inputs[1].ty() == ValType::I32 {
        inputs[1].to_i32()
    } else {
        return Err(HostFuncError::User(3));
    };

    Ok(vec![WasmValue::from_i32(val1 - val2)])
}

#[sys_host_function]
fn extern_mul(
    _frame: CallingFrame,
    inputs: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, HostFuncError> {
    let val1 = if inputs[0].ty() == ValType::ExternRef {
        inputs[0]
    } else {
        return Err(HostFuncError::User(2));
    };
    let val1 = val1
        .extern_ref::<i32>()
        .expect("fail to get i32 from an ExternRef");

    let val2 = if inputs[1].ty() == ValType::I32 {
        inputs[1].to_i32()
    } else {
        return Err(HostFuncError::User(3));
    };

    Ok(vec![WasmValue::from_i32(val1 * val2)])
}

#[sys_host_function]
fn extern_div(
    _frame: CallingFrame,
    inputs: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, HostFuncError> {
    let val1 = if inputs[0].ty() == ValType::ExternRef {
        inputs[0]
    } else {
        return Err(HostFuncError::User(2));
    };
    let val1 = val1
        .extern_ref::<i32>()
        .expect("fail to get i32 from an ExternRef");

    let val2 = if inputs[1].ty() == ValType::I32 {
        inputs[1].to_i32()
    } else {
        return Err(HostFuncError::User(3));
    };

    Ok(vec![WasmValue::from_i32(val1 / val2)])
}

#[sys_host_function]
fn extern_term(
    _frame: CallingFrame,
    _inputs: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, HostFuncError> {
    Ok(vec![WasmValue::from_i32(1234)])
}

#[sys_host_function]
fn extern_fail(
    _frame: CallingFrame,
    _inputs: Vec<WasmValue>,
) -> Result<Vec<WasmValue>, HostFuncError> {
    Err(HostFuncError::User(2))
}
