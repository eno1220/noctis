use alloc::vec;
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

    print!("{:#}", stack.pop().unwrap());
}

pub fn wasm_entry() {
    loop {
        let instructions = vec![
            Instruction::Const(21),
            Instruction::Const(2),
            Instruction::Mul,
            Instruction::End,
        ];

        run(&instructions);

        unsafe { asm!("hlt") }
    }
}
