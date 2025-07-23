use alloc::vec;
use alloc::vec::Vec;

use crate::print;
use core::arch::asm;

#[derive(Debug, Clone, Copy)]
enum Instruction {
    Const(i32),
    LocalGet(u32),
    I32Add,
    End,
}

#[derive(Default)]
pub struct Frame {
    pub pc: isize,
    pub sp: usize,
    insts: Vec<Instruction>,
    pub arity: usize,
    pub locals: Vec<i32>, // i32固定とする
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    I32,
}

#[derive(Clone)]
pub struct FuncType {
    pub params: Vec<ValueType>,
    pub results: Vec<ValueType>,
}

#[derive(Clone)]
pub struct Func {
    pub locals: Vec<i32>,
    body: Vec<Instruction>,
}

#[derive(Clone)]
pub struct InternalFuncInst {
    pub func_type: FuncType, // 関数のシグネチャ
    pub code: Func,          // 関数のローカル変数の定義・命令列
}

pub enum FuncInst {
    Internal(InternalFuncInst),
}

#[derive(Default)]
pub struct Store {
    pub funcs: Vec<FuncInst>,
}

#[derive(Default)]
pub struct Runtime {
    pub store: Store,
    pub stack: Vec<i32>,
    pub call_stack: Vec<Frame>,
}

impl Runtime {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            ..Default::default()
        }
    }

    pub fn call(&mut self, idx: usize, args: Vec<i32>) -> Option<i32> {
        let Some(func_inst) = self.store.funcs.get(idx) else {
            panic!("not found func")
        };
        for arg in args {
            self.stack.push(arg);
        }
        match func_inst {
            FuncInst::Internal(func) => self.invoke(func.clone()),
        }
    }

    fn invoke(&mut self, func: InternalFuncInst) -> Option<i32> {
        let bottom = self.stack.len() - func.func_type.params.len();
        let mut locals = self.stack.split_off(bottom);

        for _ in func.code.locals.iter() {
            locals.push(0);
        }
        let arity = func.func_type.results.len();
        let frame = Frame {
            pc: -1,
            sp: self.stack.len(),
            insts: func.code.body.clone(),
            arity,
            locals,
        };
        self.call_stack.push(frame);

        self.execute();

        if arity > 0 {
            let Some(val) = self.stack.pop() else {
                panic!("not found return value")
            };
            return Some(val);
        }
        None
    }

    fn execute(&mut self) {
        loop {
            let Some(frame) = self.call_stack.last_mut() else {
                break;
            };
            frame.pc += 1;

            let Some(inst) = frame.insts.get(frame.pc as usize) else {
                break;
            };

            match inst {
                Instruction::I32Add => {
                    let (Some(rhs), Some(lhs)) = (self.stack.pop(), self.stack.pop()) else {
                        panic!("not found any value in the stack")
                    };
                    self.stack.push(lhs + rhs);
                }
                Instruction::Const(val) => {
                    self.stack.push(*val);
                }
                Instruction::LocalGet(idx) => {
                    let Some(val) = frame.locals.get(*idx as usize) else {
                        panic!("not found local variable")
                    };
                    self.stack.push(*val)
                }
                Instruction::End => {
                    let Some(frame) = self.call_stack.pop() else {
                        panic!("not found call frame")
                    };
                    let Frame { sp, arity, .. } = frame;
                    stack_unwind(&mut self.stack, sp, arity);
                }
            }
        }
    }
}

pub fn stack_unwind(stack: &mut Vec<i32>, sp: usize, arity: usize) {
    if arity > 0 {
        let Some(val) = stack.pop() else {
            panic!("not found return value")
        };
        stack.drain(sp..);
        stack.push(val);
    } else {
        stack.drain(sp..);
    }
}

pub fn wasm_entry() {
    let wasm = Store {
        funcs: vec![FuncInst::Internal(InternalFuncInst {
            func_type: FuncType {
                params: vec![ValueType::I32, ValueType::I32],
                results: vec![ValueType::I32],
            },
            code: Func {
                locals: vec![],
                body: vec![
                    Instruction::LocalGet(0),
                    Instruction::LocalGet(1),
                    Instruction::I32Add,
                    Instruction::Const(40),
                    Instruction::I32Add,
                    Instruction::End,
                ],
            },
        })],
    };
    let mut runtime = Runtime::new(wasm);
    loop {
        if let Some(res) = runtime.call(0, vec![10, 20]) {
            print!("{:#}", res);
        };

        unsafe { asm!("hlt") }
    }
}
