use alloc::vec;
use alloc::vec::Vec;

use crate::print;
use core::arch::asm;

#[allow(dead_code)]
#[derive(Debug, Clone)]
enum Instruction {
    If(Block),
    End,
    Return,
    Const(i32),
    LocalGet(u32),
    I32Lts,
    I32Add,
    I32Sub,
    I32Mul,
    Call(u32),
}

#[derive(Default)]
pub struct Frame {
    pub pc: isize,
    pub sp: usize,
    insts: Vec<Instruction>,
    pub arity: usize,
    pub labels: Vec<Label>,
    pub locals: Vec<i32>, // i32固定とする
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockType {
    Void,
}

impl BlockType {
    pub fn result_count(&self) -> usize {
        match self {
            Self::Void => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub block_type: BlockType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LabelKind {
    If,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Label {
    kind: LabelKind,
    pub pc: usize,
    pub sp: usize,
    pub arity: usize,
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
        self.stack.clear();
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

    fn push_frame(&mut self, func: &InternalFuncInst) {
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
            labels: vec![],
            locals,
        };

        self.call_stack.push(frame);
    }

    fn invoke(&mut self, func: InternalFuncInst) -> Option<i32> {
        let arity = func.func_type.results.len();

        self.push_frame(&func);
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
                Instruction::I32Sub => {
                    let (Some(rhs), Some(lhs)) = (self.stack.pop(), self.stack.pop()) else {
                        panic!("not found any value in the stack")
                    };
                    self.stack.push(lhs - rhs);
                }
                Instruction::I32Mul => {
                    let (Some(rhs), Some(lhs)) = (self.stack.pop(), self.stack.pop()) else {
                        panic!("not found any value in the stack")
                    };
                    self.stack.push(lhs * rhs);
                }
                Instruction::I32Lts => {
                    let (Some(rhs), Some(lhs)) = (self.stack.pop(), self.stack.pop()) else {
                        panic!("not found any value in the stack")
                    };
                    self.stack.push((lhs < rhs).into());
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
                Instruction::Call(idx) => {
                    let Some(func) = self.store.funcs.get(*idx as usize) else {
                        panic!("not found func")
                    };
                    match func {
                        FuncInst::Internal(func) => self.push_frame(&func.clone()),
                    }
                }
                Instruction::If(block) => {
                    let cond = self.stack.pop().expect("not found value in the stack");
                    let next_pc = get_end_addr(&frame.insts, frame.pc as usize);
                    if cond == 0 {
                        frame.pc = next_pc as isize
                    }

                    let label = Label {
                        kind: LabelKind::If,
                        pc: next_pc,
                        sp: self.stack.len(),
                        arity: block.block_type.result_count(),
                    };
                    frame.labels.push(label);
                }
                Instruction::Return => {
                    let frame = self.call_stack.pop().expect("not found frame");
                    let Frame { sp, arity, .. } = frame;
                    stack_unwind(&mut self.stack, sp, arity);
                }
                Instruction::End => match frame.labels.pop() {
                    Some(label) => {
                        let Label { pc, sp, arity, .. } = label;
                        frame.pc = pc as isize;
                        stack_unwind(&mut self.stack, sp, arity);
                    }
                    None => {
                        let frame = self.call_stack.pop().expect("not found value in the stack");
                        let Frame { sp, arity, .. } = frame;
                        stack_unwind(&mut self.stack, sp, arity);
                    }
                },
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

fn get_end_addr(insts: &[Instruction], pc: usize) -> usize {
    let mut pc = pc;
    let mut depth = 0;
    loop {
        pc += 1;
        let inst = insts.get(pc).expect("not found instructions");
        match inst {
            Instruction::If(_) => depth += 1,
            Instruction::End => {
                if depth == 0 {
                    return pc;
                } else {
                    depth -= 1;
                }
            }
            _ => {}
        }
    }
}

pub fn wasm_entry() {
    // フィボナッチ数列の計算
    let wasm = Store {
        funcs: vec![FuncInst::Internal(InternalFuncInst {
            func_type: FuncType {
                params: vec![ValueType::I32],
                results: vec![ValueType::I32],
            },
            code: Func {
                locals: vec![],
                body: vec![
                    Instruction::LocalGet(0), // n
                    Instruction::Const(2),    // 2
                    Instruction::I32Lts,      // n < 2
                    Instruction::If(Block {
                        block_type: BlockType::Void,
                    }),
                    Instruction::LocalGet(0), // n
                    Instruction::Return,      // return n
                    Instruction::End,
                    Instruction::LocalGet(0), // n
                    Instruction::Const(1),
                    Instruction::I32Sub,      // n - 1
                    Instruction::Call(0),     // fib(n-1)
                    Instruction::LocalGet(0), // n
                    Instruction::Const(2),
                    Instruction::I32Sub,  // n - 2
                    Instruction::Call(0), //fib(n-2)
                    Instruction::I32Add,  // fib(n-1) + fib(n-2)
                    Instruction::Return,
                ],
            },
        })],
    };
    let mut runtime = Runtime::new(wasm);
    loop {
        if let Some(res) = runtime.call(0, vec![8]) {
            print!("{:#}", res);
        };

        unsafe { asm!("hlt") }
    }
}
