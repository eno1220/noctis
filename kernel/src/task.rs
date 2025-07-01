use crate::{
    info,
    spin::{SpinGuard, SpinLock},
};

use alloc::{
    boxed::Box,
    sync::Arc,
    vec::Vec,
};
use core::{
    arch::naked_asm,
    ops::AddAssign,
    pin::Pin,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
    u32,
};

const KERNEL_STACK_SIZE: usize = 4096;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PId(u32);

impl PId {
    pub fn new(pid: u32) -> PId {
        PId(pid)
    }

    pub fn to_u32(&self) -> u32 {
        self.0
    }
}

impl AddAssign<usize> for PId {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs as u32;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TaskState {
    Runnable,
    Stopped,
}

#[repr(C)]
#[derive(Default)]
struct TaskContext {
    rbx: u64,
    rbp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rsp: u64,
    rflags: u64,
}

impl TaskContext {
    fn setup_initial_call(&mut self, kstack: &KStack, func: fn()) {
        let mut stack_top = kstack.as_ref().get_ref().as_ptr() as *mut u8;
        unsafe {
            stack_top = stack_top.add(KERNEL_STACK_SIZE);
            stack_top = stack_top.sub(core::mem::size_of::<usize>());
            stack_top.cast::<usize>().write(func as usize);
        }
        self.rsp = stack_top as u64;
        self.rflags = 0x202;
    }
}

type KStack = Pin<Box<[u8; KERNEL_STACK_SIZE]>>;

pub struct Task {
    pid: PId,
    state: TaskState,
    running: bool,
    context: TaskContext,
    kernel_stack: Option<KStack>,
}

impl Task {
    fn new() -> Self {
        static PID: AtomicU32 = AtomicU32::new(0);
        Task {
            pid: PId::new(PID.fetch_add(1, Ordering::Relaxed)),
            state: TaskState::Stopped,
            running: false,
            context: TaskContext::default(),
            kernel_stack: None,
        }
    }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe fn switch_inner(current: *mut TaskContext, next: *mut TaskContext) {
    naked_asm!(
        "pushfq",
        "pop rax",
        "mov [rdi + 0x38], rax",
        "mov [rdi + 0x0], rbx",
        "mov [rdi + 0x8], rbp",
        "mov [rdi + 0x10], r12",
        "mov [rdi + 0x18], r13",
        "mov [rdi + 0x20], r14",
        "mov [rdi + 0x28], r15",
        "mov [rdi + 0x30], rsp",
        "mov rbx, [rsi + 0x0]",
        "mov rbp, [rsi + 0x8]",
        "mov r12, [rsi + 0x10]",
        "mov r13, [rsi + 0x18]",
        "mov r14, [rsi + 0x20]",
        "mov r15, [rsi + 0x28]",
        "mov rsp, [rsi + 0x30]",
        "mov rax, [rsi + 0x38]",
        "push rax",
        "popfq",
        "ret",
    )
}

#[unsafe(naked)]
unsafe fn switch(current: *mut TaskContext, next: *mut TaskContext) {
    naked_asm!("push 2f", "jmp switch_inner", "2:", "ret",)
}

pub struct CpuContextBlock {
    pub ticks: AtomicUsize,
    current_task: SpinLock<Option<Arc<SpinLock<Task>>>>,
}

impl CpuContextBlock {
    pub const fn new() -> Self {
        Self {
            ticks: AtomicUsize::new(0),
            current_task: SpinLock::new(None),
        }
    }
}

static CPU_CONTEXT_BLOCK: CpuContextBlock = CpuContextBlock::new();

pub fn context() -> &'static CpuContextBlock {
    &CPU_CONTEXT_BLOCK
}

//static CPU_CONTEXT_BLOCK: CpuContextBlock = CpuContextBlock::new();
// Taskは無限ループして終了することはないものとして扱う
static TASKS: SpinLock<Vec<Arc<SpinLock<Task>>>> = SpinLock::new(Vec::new());

pub fn current_task() -> Arc<SpinLock<Task>> {
    let context = context();

    context.current_task.lock().as_ref().unwrap().clone()
}

pub fn tick() {
    let context = context();
    context.ticks.fetch_add(1, Ordering::Relaxed);
}

pub fn tasks() -> SpinGuard<'static, Vec<Arc<SpinLock<Task>>>> {
    TASKS.lock()
}

pub fn init() {
    let mut task = Task::new();
    task.state = TaskState::Runnable;
    task.running = true;

    let task_lock = Arc::new(SpinLock::new(task));
    TASKS.lock().push(task_lock.clone());

    let context = context();
    *context.current_task.lock() = Some(Arc::clone(&task_lock));
}

pub fn schedule() {
    let percpu = context();
    percpu.ticks.store(0, Ordering::Relaxed);

    let (prev_task_lock, next_task_lock) = {
        let tasks = tasks();

        let current_task_lock = current_task();
        let current_index = tasks
            .iter()
            .position(|t| Arc::ptr_eq(t, &current_task_lock));

        let next_task_lock = if tasks.is_empty() {
            None
        } else {
            let len = tasks.len();
            let mut i = current_index.map_or(0, |i| (i + 1) % len);
            let start = i;
            loop {
                if let Some(task) = tasks.get(i) {
                    let state = task.lock().state;
                    if state == TaskState::Runnable {
                        break Some(task.clone());
                    }
                }
                i = (i + 1) % len;
                if i == start {
                    break None;
                }
            }
        };

        if let Some(next_task_lock) = next_task_lock {
            (Some(current_task_lock), Some(next_task_lock))
        } else {
            (None, None)
        }
    };

    if let (Some(prev_task_lock), Some(next_task_lock)) = (prev_task_lock, next_task_lock) {
        let mut prev_task_guard = prev_task_lock.lock();
        let mut next_task_guard = next_task_lock.lock();

        *percpu.current_task.lock() = Some(Arc::clone(&next_task_lock));

        prev_task_guard.running = false;
        next_task_guard.running = true;

        unsafe {
            switch(&mut prev_task_guard.context, &mut next_task_guard.context);
        }
    }
}

pub fn spawn(func: fn()) {
    let kstack = Pin::from(Box::new([0u8; KERNEL_STACK_SIZE]));
    let task_lock = Arc::new(SpinLock::new(Task::new()));

    TASKS.lock().push(task_lock.clone());
    {
        let mut task = task_lock.lock();
        task.context.setup_initial_call(&kstack, func);
        task.kernel_stack = Some(kstack);
        task.state = TaskState::Runnable;
        info!(
            "taskid: {:#}, rsp: {:#x}",
            task.pid.to_u32(),
            task.context.rsp
        );
    }
}
