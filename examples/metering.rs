//! Wasmer will let you easily run Wasm module in a Rust host.
//!
//! This example illustrates the basics of using Wasmer metering features:
//!
//!   1. How to enable metering in a module
//!   2. How to meter a specific function call
//!   3. How to make execution fails if cost exceeds a given limit
//!
//! You can run the example directly by executing in Wasmer root:
//!
//! ```shell
//! cargo run --example metering --release --features "cranelift"
//! ```
//!
//! Ready?

use anyhow::bail;
use std::sync::Arc;
use wasmer::wasmparser::Operator;
use wasmer::CompilerConfig;
use wasmer::{imports, wat2wasm, Instance, Module, Store};
use wasmer_compiler_cranelift::Cranelift;
use wasmer_engine_jit::JIT;
use wasmer_middlewares::metering::{get_remaining_points, set_remaining_points, Metering};

fn main() -> anyhow::Result<()> {
    // Let's declare the Wasm module.
    //
    // We are using the text representation of the module here but you can also load `.wasm`
    // files using the `include_bytes!` macro.
    let wasm_bytes = wat2wasm(
        br#"
(module
  (type $add_t (func (param i32) (result i32)))
  (func $add_one_f (type $add_t) (param $value i32) (result i32)
    local.get $value
    i32.const 1
    i32.add)
  (export "add_one" (func $add_one_f)))
"#,
    )?;

    // Let's define our cost function.
    //
    // This function will be called for each `Operator` encountered during
    // the Wasm module execution. It should return the cost of the operator
    // that it received as it first argument.
    let cost_function = |operator: &Operator| -> u64 {
        match operator {
            Operator::LocalGet { .. } | Operator::I32Const { .. } => 1,
            Operator::I32Add { .. } => 2,
            _ => 0,
        }
    };

    // Now let's create our metering middleware.
    //
    // `Metering` needs to be configured with a limit (the gas limit) and
    // a cost function.
    //
    // For each `Operator`, the metering middleware will call the cost
    // function and subtract the cost from the gas.
    let metering = Arc::new(Metering::new(10, cost_function));
    let mut compiler_config = Cranelift::default();
    compiler_config.push_middleware(metering);

    // Create a Store.
    //
    // We use our previously create compiler configuration
    // with the JIT engine.
    let store = Store::new(&JIT::new(compiler_config).engine());

    println!("Compiling module...");
    // Let's compile the Wasm module.
    let module = Module::new(&store, wasm_bytes)?;

    // Create an empty import object.
    let import_object = imports! {};

    println!("Instantiating module...");
    // Let's instantiate the Wasm module.
    let instance = Instance::new(&module, &import_object)?;

    // We now have an instance ready to be used.
    //
    // Our module exports a single `add_one`  function. We want to
    // measure the cost of executing this function.
    let add_one = instance
        .exports
        .get_function("add_one")?
        .native::<i32, i32>()?;

    println!("Calling `add_one` function once...");
    add_one.call(1)?;

    // As you can see here, after the first call we have 6 remaining gas points.
    //
    // This is correct, here are the details of how it has been computed:
    // * `local.get $value` is a `Operator::LocalGet` which costs 1 point;
    // * `i32.const` is a `Operator::I32Const` which costs 1 point;
    // * `i32.add` is a `Operator::I32Add` which costs 2 points.
    let remaining_points_after_first_call = get_remaining_points(&instance);
    assert_eq!(remaining_points_after_first_call, 6);

    println!(
        "Remaining points after the first call: {:?}",
        remaining_points_after_first_call
    );

    println!("Calling `add_one` function twice...");
    add_one.call(1)?;

    // We spent 4 more gas points with the second call.
    // We have 2 remaining points.
    let remaining_points_after_second_call = get_remaining_points(&instance);
    assert_eq!(remaining_points_after_second_call, 2);

    println!(
        "Remaining points after the second call: {:?}",
        remaining_points_after_second_call
    );

    // Because calling our `add_one` function consumes 4 gas points,
    // calling it a third time will fail: we already consume 8 gas
    // points, there are only two remaining.
    println!("Calling `add_one` function a third time...");
    match add_one.call(1) {
        Ok(result) => {
            bail!(
                "Expected failure while calling `add_one`, found: {}",
                result
            );
        }
        Err(_) => {
            println!("Calling `add_one` failed: not enough gas points remaining.");
        }
    }

    // Becasue the previous call failed, it did not consume any gas point.
    // We still have 2 remaining points.
    let remaining_points_after_third_call = get_remaining_points(&instance);
    assert_eq!(remaining_points_after_third_call, 2);

    println!(
        "Remaining points after third call: {:?}",
        remaining_points_after_third_call
    );

    // Now let's see how we can set a new limit...
    println!("Set new remaining points points to 10");
    let new_limit = 10;
    set_remaining_points(&instance, new_limit);

    let remaining_points = get_remaining_points(&instance);
    assert_eq!(remaining_points, new_limit);

    println!("Remaining points: {:?}", remaining_points);

    Ok(())
}

#[test]
fn test_metering() -> anyhow::Result<()> {
    main()
}
