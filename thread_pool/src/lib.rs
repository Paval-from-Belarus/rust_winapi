#![feature(new_uninit)]
extern crate utils;

use std::mem::MaybeUninit;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::atomic::Ordering::Relaxed;
use winapi::shared::minwindef::{DWORD, LPVOID};
use winapi::um::processthreadsapi::{CreateThread, SwitchToThread};
use winapi::um::synchapi::{CONDITION_VARIABLE, EnterCriticalSection, InitializeConditionVariable, InitializeCriticalSection, LeaveCriticalSection, SleepConditionVariableCS, WaitForSingleObject, WakeAllConditionVariable, WakeConditionVariable};
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::{HANDLE, RTL_CRITICAL_SECTION};
use utils::bitflags;


pub type TaskHandler = fn(*const u8);

pub struct Task {
    handler: TaskHandler,
    param: *const u8,
    is_done: Arc<AtomicBool>,
}

pub struct Future {
    is_done: Arc<AtomicBool>,
}

pub struct ThreadPool {
    workers: Vec<Box<Worker>>,
    queue: Box<TaskQueue>,
}

pub struct Worker {
    id: DWORD,
    handle: HANDLE,
    queue: *mut TaskQueue,
}
bitflags!(
    pub ThreadStatus(usize),
    INTERRUPTED = 0b1
);
pub struct TaskQueue {
    lock: RTL_CRITICAL_SECTION,
    empty_queue_condition: CONDITION_VARIABLE,
    full_queue_condition: CONDITION_VARIABLE,
    tasks: Vec<Task>,
    task_count: usize,
    is_interrupted: bool,
}

impl Future {
    //wait until task is not completed
    pub fn get(&self) {
        while !self.is_done.load(Relaxed) {
            unsafe {
                SwitchToThread();
            }
        }
    }
}

impl Task {
    fn new(handler: TaskHandler, param: *const u8, is_done: Arc<AtomicBool>) -> Self {
        Task { handler, param, is_done }
    }
    pub fn invoke(&mut self) {
        let handler = self.handler;
        handler(self.param);
        self.is_done.store(true, Relaxed);
    }
}

impl ThreadPool {
    pub fn new(queue_size: usize, worker_count: usize) -> Self {
        let mut queue = Box::new(TaskQueue::new(queue_size));
        let mut workers = Vec::<Box<Worker>>::with_capacity(worker_count);
        for _ in 0..worker_count {
            workers.push(Worker::new(queue.as_mut()));
        }
        ThreadPool { queue, workers }
    }
    pub fn submit(&mut self, handler: TaskHandler, param: *const u8) -> Future {
        let future = Future { is_done: Arc::new(AtomicBool::new(false)) };
        let task = Task::new(handler, param, Arc::clone(&future.is_done));
        self.queue.put(task).expect("The put thread was interrupted");
        future
    }
    pub fn wait(&mut self) {
        while self.queue.get_size() > 0 {
            unsafe { SwitchToThread() };
        }
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.queue.notify_all();
        for worker in &self.workers {
            unsafe { WaitForSingleObject(worker.handle, INFINITE) };
        }
    }
}

impl TaskQueue {
    pub fn new(size: usize) -> TaskQueue {
        assert_ne!(size, 0);
        TaskQueue {
            tasks: Vec::with_capacity(size),
            task_count: 0,
            is_interrupted: false,
            lock: init_lock(),
            empty_queue_condition: init_condition(),
            full_queue_condition: init_condition(),
        }
    }
    pub fn notify_all(&mut self) {
        unsafe {
            EnterCriticalSection(&mut self.lock);
            self.is_interrupted = true;
            LeaveCriticalSection(&mut self.lock);
            WakeAllConditionVariable(&mut self.full_queue_condition);
            WakeAllConditionVariable(&mut self.empty_queue_condition);
        }
    }
    pub fn get_size(&mut self) -> usize {
        let queue_size = unsafe {
            let size;
            EnterCriticalSection(&mut self.lock);
            size = self.task_count;
            LeaveCriticalSection(&mut self.lock);
            size
        };
        queue_size
    }
    //atomically put value in queue
    pub fn put(&mut self, task: Task) -> Result<(), ThreadStatus> {
        unsafe {
            EnterCriticalSection(&mut self.lock);
            while !self.is_interrupted && self.task_count == self.tasks.capacity() {
                SleepConditionVariableCS(
                    &mut self.full_queue_condition,
                    &mut self.lock,
                    INFINITE);
            }
        }
        if self.is_interrupted {
            unsafe { LeaveCriticalSection(&mut self.lock) };
            return Err(ThreadStatus::from(ThreadStatus::INTERRUPTED));
        }
        self.task_count += 1;
        self.tasks.push(task);
        unsafe {
            LeaveCriticalSection(&mut self.lock);
            WakeConditionVariable(&mut self.empty_queue_condition);
        }
        Ok(())
    }
    pub fn get(&mut self) -> Result<Task, ThreadStatus> {
        unsafe {
            EnterCriticalSection(&mut self.lock);
            while !self.is_interrupted && self.task_count == 0 {
                SleepConditionVariableCS(
                    &mut self.empty_queue_condition,
                    &mut self.lock,
                    INFINITE);
            }
        }
        if self.is_interrupted {
            unsafe { LeaveCriticalSection(&mut self.lock) };
            return Err(ThreadStatus::from(ThreadStatus::INTERRUPTED));
        }
        self.task_count -= 1;
        let task = self.tasks.remove(self.task_count);
        unsafe {
            LeaveCriticalSection(&mut self.lock);
            WakeConditionVariable(&mut self.full_queue_condition);
        }
        Ok(task)
    }
}

impl Worker {
    pub fn new(queue: *mut TaskQueue) -> Box<Self> {
        let mut worker = Box::<Worker>::new_uninit();
        let mut id = 0;
        let handle: HANDLE = unsafe {
            CreateThread(
                ptr::null_mut(),
                0,
                Some(Self::system_proc),
                worker.as_ptr() as _,
                0,
                &mut id,
            ) as _
        };
        worker.write(Worker { id, handle, queue });
        unsafe { worker.assume_init() }
    }
    fn run(&mut self) -> DWORD {
        let queue = unsafe { &mut *self.queue };
        loop {
            let next_task = queue.get();
            if let Err(thread_status) = next_task {
                println!("The thread {} exits with status {}", self.id, thread_status.0);
                break;
            }
            let mut task = next_task.expect("Unreachable way");
            task.invoke();
        }
        0
    }
    extern "system" fn system_proc(param: LPVOID) -> DWORD {
        let worker = unsafe { &mut *(param as *mut Worker) };
        worker.run()
    }
}

pub fn init_lock() -> RTL_CRITICAL_SECTION {
    let mut lock = MaybeUninit::<RTL_CRITICAL_SECTION>::uninit();
    unsafe { InitializeCriticalSection(lock.as_mut_ptr()); }
    unsafe { lock.assume_init() }
}

pub fn init_condition() -> CONDITION_VARIABLE {
    let mut cond = MaybeUninit::<CONDITION_VARIABLE>::uninit();
    unsafe { InitializeConditionVariable(cond.as_mut_ptr()); }
    unsafe { cond.assume_init() }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use super::*;

    #[test]
    fn it_works() {
        let mut pool = ThreadPool::new(5, 3);
        for number in 0..7 {
            let handler = unsafe { mem::transmute::<fn(usize), TaskHandler>(print_number) };
            let param = number as *const u8;
            pool.submit(handler, param);
        }
        pool.wait();
    }
    #[test]
    fn test_futures() {
        let mut pool = ThreadPool::new(3, 10);
        let mut futures = Vec::<Future>::with_capacity(10);
        let handler = unsafe { mem::transmute::<fn(usize), TaskHandler>(print_number) };
        for i in 0..10 {
            let future = pool.submit(handler, i as *const u8);
            futures.push(future);
        }
        for (index, future) in futures.iter().enumerate() {
            future.get();
            println!("Futures {} is released", index);
        }
        pool.submit(handler, 13 as *const u8);
    }
    pub fn print_number(value: usize) {
        println!("The counter is {}", value);
    }
}
