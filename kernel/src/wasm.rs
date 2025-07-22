use alloc::vec::Vec;

use crate::print;
use core::arch::asm;

#[derive(Debug, Clone, Copy)]
enum Instruction {
    Const(i32),
    Add,
    Sub,
    Mul,
    Div,
    End,
}

fn run(instructions: &[Instruction]) {
    let mut pc = 0;
    let mut stack: Vec<i32> = Vec::new();

    while pc < instructions.len() {
        match instructions[pc] {
            Instruction::Const(val) => stack.push(val),
            Instruction::Add => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a + b);
            }
            Instruction::Sub => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a - b);
            }
            Instruction::Mul => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a * b);
            }
            Instruction::Div => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                stack.push(a / b);
            }
            Instruction::End => {}
        }
        pc += 1;
    }

    print!("[{:#}]", stack.pop().unwrap());
}

pub fn wasm_entry() {
    let mut idx = 0;
    let patterns: [&[Instruction]; 5] = [
        &[
            Instruction::Const(21),
            Instruction::Const(2),
            Instruction::Mul,
            Instruction::End,
        ],
        &[
            Instruction::Const(10),
            Instruction::Const(5),
            Instruction::Add,
            Instruction::End,
        ],
        &[
            Instruction::Const(100),
            Instruction::Const(4),
            Instruction::Div,
            Instruction::End,
        ],
        &[
            Instruction::Const(7),
            Instruction::Const(3),
            Instruction::Sub,
            Instruction::End,
        ],
        &[
            Instruction::Const(2),
            Instruction::Const(3),
            Instruction::Const(4),
            Instruction::Add,
            Instruction::Mul,
            Instruction::End,
        ],
    ];

    loop {
        run(patterns[idx]);
        idx = (idx + 1) % 5;

        unsafe { asm!("hlt") }
    }
}
