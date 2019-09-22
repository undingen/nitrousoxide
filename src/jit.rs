use crate::syms::get_syms;

use failure::Error;

use cranelift_llvm;

use cranelift_module::{default_libcall_names, Backend, FuncId, Linkage, Module};
use cranelift_simplejit::{SimpleJITBackend, SimpleJITBuilder};
use std::mem;

use cranelift_codegen::cursor::{Cursor, FuncCursor};
use cranelift_codegen::ir;
use cranelift_codegen::ir::{
    ExternalName, Function, GlobalValueData, Inst, InstBuilder, InstructionData,
};
use cranelift_codegen::settings;
use cranelift_codegen::settings::Configurable;

use std::collections::HashMap;
use std::sync::Mutex;

const VERBOSITY: u32 = 0;

lazy_static! {
    static ref MODULES: Mutex<Vec<cranelift_llvm::Module>> = Mutex::new(Vec::new());
    static ref SYMS: Vec<(String, u64)> = get_syms();
}

pub fn load_bitcode(file_name: String) {
    let llvm_ctx = cranelift_llvm::create_llvm_context();
    let llvm_module = cranelift_llvm::read_llvm(llvm_ctx, &file_name).unwrap();
    let tmodule = cranelift_llvm::translate_module(llvm_module, None).unwrap();

    if VERBOSITY > 0 {
        println! {"loaded {}", file_name}
        println! {"\tfound {} funcs", tmodule.functions.len()}
        println! {"\tfound {} imports", tmodule.imports.len()}
        println! {"\tfound {} data syms", tmodule.data_symbols.len()}
    }

    MODULES.lock().unwrap().push(tmodule);

    // hack access SYMS so they get initalized early...
    assert!(SYMS.len() > 0);
}

fn get_funcref_for_extfunc_name(func: &mut Function, name: &ExternalName) -> Option<ir::FuncRef> {
    //println! {"looking for {:?} in {:?}", name, func.dfg.ext_funcs};
    for e in func.dfg.ext_funcs.iter() {
        if e.1.name == *name {
            return Some(e.0);
        }
    }
    None
}

fn import_externs_into_module<B>(
    module: &mut Module<B>,
    src_module: &cranelift_llvm::Module,
    func: &mut Function,
) -> HashMap<FuncId, String>
where
    B: Backend,
{
    let mut mapping = HashMap::new();
    for ext_func in func.dfg.ext_funcs.iter_mut() {
        let name_str = src_module.strings.get_str(&ext_func.1.name);
        if VERBOSITY > 0 {
            println!("\textern function: {:?}\t{}", ext_func, &name_str);
        }
        if let ExternalName::LibCall(..) = ext_func.1.name {
            continue;
        }
        let new_id = module
            .declare_function(
                &name_str,
                Linkage::Import,
                &func.dfg.signatures[ext_func.1.signature],
            )
            .unwrap();

        mapping.insert(new_id, name_str.to_owned());
        if ext_func.1.name != ExternalName::from(new_id) {
            if VERBOSITY > 0 {
                println!("\t\tremapping to {}", ExternalName::from(new_id));
            }
            ext_func.1.name = ExternalName::from(new_id);
        }
    }

    for gv in func.global_values.values() {
        if let ir::GlobalValueData::Symbol { name, .. } = gv {
            if VERBOSITY > 0 {
                println!("\tgv: {:?}\t{}", name, src_module.strings.get_str(&name),);
            }
            let idx = module
                .declare_data(
                    src_module.strings.get_str(&name),
                    Linkage::Import,
                    false,
                    None,
                )
                .unwrap();
            if ExternalName::from(idx) != *name {
                panic! {"mismatch {} != {}", ExternalName::from(idx), name};
            }
        } else {
            panic! {"not handled"};
        }
    }

    mapping
}

fn inlinefn_into(calleefunc: &mut Function, callsite: Inst, func: &Function) {
    let callsite_data = calleefunc.dfg[callsite].clone();
    let mut pos = FuncCursor::new(calleefunc);
    pos.goto_after_inst(callsite);

    if VERBOSITY > 0 {
        println!(
            "inlining\n{}\n into \n{}\n",
            func.display(None),
            pos.func.display(None)
        );
    }

    let func_args = match callsite_data.analyze_call(&pos.func.dfg.value_lists) {
        ir::instructions::CallInfo::Direct(_, a) => a,
        ir::instructions::CallInfo::Indirect(_, a) => a,
        _ => panic!("not supported"),
    };

    let entry_block_args = func.dfg.ebb_params(func.layout.entry_block().unwrap());

    let mut valuemap: HashMap<ir::Value, ir::Value> = HashMap::new();
    for (index, v) in entry_block_args.iter().enumerate() {
        valuemap.insert(*v, func_args[index]);
    }

    let continue_block = pos.func.dfg.make_ebb();
    let inst_after_call = pos.layout().next_inst(callsite).unwrap();
    pos.layout_mut().split_ebb(continue_block, inst_after_call);

    let ret_val = pos
        .func
        .dfg
        .append_ebb_param(continue_block, func.signature.returns[0].value_type);

    let mut gvmap: HashMap<ir::GlobalValue, ir::GlobalValue> = HashMap::new();
    for gv in func.global_values.iter() {
        gvmap.insert(gv.0, pos.func.create_global_value(gv.1.clone()));
    }
    let mut fnmap: HashMap<ir::FuncRef, ir::FuncRef> = HashMap::new();
    for ef in func.dfg.ext_funcs.iter() {
        let funcref = if let Some(v) = get_funcref_for_extfunc_name(&mut pos.func, &ef.1.name) {
            v
        } else {
            let data = ir::ExtFuncData {
                name: ef.1.name.clone(),
                signature: pos
                    .func
                    .import_signature(func.dfg.signatures[ef.1.signature].clone()),
                colocated: ef.1.colocated,
            };
            pos.func.import_function(data)
        };
        fnmap.insert(ef.0, funcref);
    }

    let mut slmap: HashMap<ir::StackSlot, ir::StackSlot> = HashMap::new();
    // TODO: right now we don't patch the stack slot references
    // so assert in cases where this is important.
    assert!(func.stack_slots.keys().len() == 0 || pos.func.stack_slots.keys().len() == 0);
    for sl in func.stack_slots.iter() {
        let data = sl.1.clone();
        assert!(data.offset.is_none());
        let new_sl = pos.func.create_stack_slot(data);
        slmap.insert(sl.0, new_sl);
    }

    let mut bbmap: HashMap<ir::Ebb, ir::Ebb> = HashMap::new();

    let mut branches_need_fixing = Vec::new();

    for block in func.layout.ebbs() {
        if block != func.layout.entry_block().unwrap() {
            let new_ebb = pos.func.dfg.make_ebb();
            pos.insert_ebb(new_ebb);
            bbmap.insert(block, new_ebb);

            for param in func.dfg.ebb_params(block) {
                let v = pos
                    .func
                    .dfg
                    .append_ebb_param(new_ebb, func.dfg.value_type(*param));
                valuemap.insert(*param, v);
            }
        } else {
            pos.goto_after_inst(callsite);
        }

        for originst in func.layout.ebb_insts(block) {
            let ctrl_typevar = func.dfg.ctrl_typevar(originst);

            if func.dfg[originst].opcode().is_return() {
                let value = func.dfg.inst_args(originst)[0];
                pos.ins()
                    .jump(continue_block, &[*valuemap.get(&value).unwrap()]);
                continue;
            }

            let get_arg = |arg: ir::Value| {
                if !func.dfg.value_is_valid(arg) {
                    return None;
                }
                let origarg = if !func.dfg.value_is_attached(arg) {
                    // mmmh not sure if this is correct or if we should instead recreate the aliases?!?
                    func.dfg.resolve_aliases(arg)
                } else {
                    arg
                };
                valuemap.get(&origarg).copied()
            };

            let mut newinst = func.dfg[originst].clone();
            let valuelist = newinst.take_value_list();
            let inst = pos.func.dfg.make_inst(newinst);
            if let Some(_) = valuelist {
                // todo handle stack_load, stack_store and stack_addr
                for origarg in func.dfg.inst_args(originst) {
                    if let Some(v) = get_arg(*origarg) {
                        pos.func.dfg.append_inst_arg(inst, v);
                    } else {
                        panic! {"need to handle this {}", origarg}
                    }
                }
            }

            //println! {"b {:?}", pos.func.dfg[inst]};
            for arg in pos.func.dfg.inst_fixed_args_mut(inst) {
                if let Some(v) = get_arg(*arg) {
                    *arg = v;
                }
            }

            match pos.func.dfg[inst] {
                InstructionData::Call {
                    ref mut func_ref, ..
                } => *func_ref = fnmap[func_ref],
                InstructionData::UnaryGlobalValue {
                    ref mut global_value,
                    ..
                } => *global_value = gvmap[global_value],
                _ => {}
            }

            if let Some(ebb) = pos.func.dfg[inst].branch_destination_mut() {
                if let Some(newbb) = bbmap.get(ebb) {
                    *ebb = *newbb;
                } else {
                    branches_need_fixing.push(inst);
                }
            }

            pos.insert_inst(inst);
            assert!(func.dfg.inst_results(originst).len() <= 1);
            pos.func.dfg.make_inst_results(inst, ctrl_typevar);

            for (i, r) in func.dfg.inst_results(originst).iter().enumerate() {
                assert!(!valuemap.contains_key(r), "dup!!");
                valuemap.insert(*r, pos.func.dfg.inst_results(inst)[i]);
            }
        }
    }

    // fixup value references in continue block
    let mut pos = FuncCursor::new(pos.func);
    let call_ret_val = pos.func.dfg.inst_results(callsite)[0];
    pos.goto_top(continue_block);
    while let Some(inst) = pos.next_inst() {
        for arg in pos.func.dfg.inst_args_mut(inst) {
            if *arg == call_ret_val {
                *arg = ret_val;
            }
        }
    }

    // fixup branches
    for branch in branches_need_fixing {
        if let Some(ebb) = pos.func.dfg[branch].branch_destination_mut() {
            if let Some(newbb) = bbmap.get(ebb) {
                *ebb = *newbb;
            } else {
                panic!("we should always find the branch dest...");
            }
        }
    }

    // remove call
    pos.layout_mut().remove_inst(callsite);
}

fn should_inline(name: &str) -> bool {
    let ret = match name {
        "fib" => true,
        "PyLong_FromLong" => true,
        "PyLong_AsLong" => true,
        _ => false,
    };
    ret
}

fn find_callsites(func: &Function) -> Vec<(Inst, ir::ExternalName)> {
    let mut callsites = Vec::new();
    for ebb in func.layout.ebbs() {
        for inst in func.layout.ebb_insts(ebb) {
            if let InstructionData::Call { func_ref, .. } = &func.dfg[inst] {
                let name = &func.dfg.ext_funcs[*func_ref].name;
                callsites.push((inst, name.clone()));
            }
        }
    }
    callsites
}

fn find_func_in_module(module: &cranelift_llvm::Module, func_name: &String) -> Option<Function> {
    let funcs_filterd = module
        .functions
        .iter()
        .filter(|&x| module.strings.get_str(&x.il.name) == func_name)
        .collect::<Vec<_>>();
    if funcs_filterd.len() == 1 {
        return Some(funcs_filterd[0].il.clone());
    }
    None
}

fn create_jit_module() -> Module<SimpleJITBackend> {
    let mut flag_builder = settings::builder();
    flag_builder.set("opt_level", "best").unwrap();

    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
        panic!("host machine is not supported: {}", msg);
    });
    let target_isa = isa_builder.finish(settings::Flags::new(flag_builder));

    let mut jit_builder = SimpleJITBuilder::with_isa(target_isa, default_libcall_names());

    // this is a hack for now: we supply all symbols but instead the JIT should provide us a way to supply a custom lookup function..
    /*
    for (name, addr) in SYMS.iter() {
        jit_builder.symbol(name, *addr as *const u8);
    }
    */

    Module::new(jit_builder)
}

pub fn jit_func(func_name: String) -> Result<*const u8, Error> {
    let mut module = create_jit_module();
    let mut ctx = module.make_context();

    let modules = MODULES.lock().unwrap();
    let mut tmodule: Option<&cranelift_llvm::Module> = None;
    let mut func: Option<Function> = None;
    for module in modules.iter() {
        func = find_func_in_module(&module, &func_name);
        // check if we found the function in the module
        if let Some(_) = func {
            tmodule = Some(module);
            break;
        }
    }
    let func = func.unwrap();
    let tmodule = tmodule.unwrap();

    let func_a = module
        .declare_function(&func_name, Linkage::Local, &func.signature)
        .unwrap();

    ctx.func = func.clone();
    let mapping = import_externs_into_module(&mut module, &tmodule, &mut ctx.func);

    let callsites = find_callsites(&ctx.func);
    for (callsite, name) in callsites {
        if let ir::ExternalName::User { index, .. } = name {
            let func_id = FuncId::from_u32(index);
            let name_str = mapping.get(&func_id).unwrap();
            if should_inline(&name_str) {
                if let Some(mut f) = find_func_in_module(&tmodule, &name_str) {
                    import_externs_into_module(&mut module, &tmodule, &mut f);
                    inlinefn_into(&mut ctx.func, callsite, &f);
                }
            }
        }
    }

    if VERBOSITY > 0 {
        println!("{}", ctx.func.display(module.isa()));
    }
    module.define_function(func_a, &mut ctx).unwrap();

    if VERBOSITY > 0 {
        for (_, gvd) in ctx.func.global_values.iter() {
            match gvd {
                GlobalValueData::Symbol { name, .. } => {
                    println!("\tgv: {}", name);
                }
                _ => println!("unknown"),
            };
        }

        println!("num ext funcs {}", ctx.func.dfg.ext_funcs.len());

        println!("\n{}", ctx.func.display(module.isa()));
    }

    println! {"{} has now {} insts", func_name, ctx.func.dfg.num_insts()};

    module.clear_context(&mut ctx);

    // Perform linking.
    module.finalize_definitions();

    // Get a raw pointer to the generated code.
    let code = module.get_finalized_function(func_a);

    Ok(code)
}

pub fn call_func(func: *const u8, params: &[u64]) -> i64 {
    match params.len() {
        0 => (unsafe { mem::transmute::<_, fn() -> i64>(func) })(),
        1 => (unsafe { mem::transmute::<_, fn(i64) -> i64>(func) })(params[0] as i64),
        n => panic!("{} params not handled", n),
    }
}
