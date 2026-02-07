use alloc::vec::Vec;
use bitmap_allocator::{BitAlloc, BitAlloc64K};
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());
}

type EntryPoint = extern "C" fn();

/// Defintion of task state
pub enum TaskState {
    /// The init state, which means the task is ready to
    /// run by CPU.
    Ready,

    /// Sign the process is currently running.
    Running,

    /// If the process has completed running, sign it as
    /// terminated.
    Terminated,
}

/// The object of a task.
pub struct Task {
    /// The ID of this task.
    pub id: u16,

    /// The state of the task.
    pub state: TaskState,

    /// The priority of the kernel (1-8)
    pub priority: u8,

    /// The entry point of the task.
    pub entry_point: usize,

    /// The stack pointer (placeholder for context switching)
    pub stack_top: usize,
}

impl Task {
    /// Create a new task object.
    pub fn new(id: u16, priority: u8, entry_point: EntryPoint) -> Self {
        Self {
            id,
            state: TaskState::Ready,
            priority,
            entry_point: entry_point as usize,
            stack_top: 0, // Should be initialized during stack allocation
        }
    }

    /// Change the status of a task.
    pub fn update_state(&mut self, new_state: TaskState) {
        self.state = new_state
    }
}

/// The task manager which contains lots of tasks.
pub struct TaskManager {
    /// The field which contains all tasks.
    pub tasks: Vec<Task>,

    /// The bitmap allocator for tracking allocated task IDs.
    allocator: BitAlloc64K,
}

impl TaskManager {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            allocator: BitAlloc64K::DEFAULT,
        }
    }

    /// Allocate a unique task ID using the bitmap allocator.
    fn alloc_tid(&mut self) -> Option<u16> {
        self.allocator.alloc().map(|tid| tid as u16)
    }

    pub fn create_task(
        &mut self,
        priority: u8,
        entry_point: EntryPoint,
    ) -> Result<u16, &'static str> {
        let task_id = self.alloc_tid().ok_or("No available Task IDs")?;

        self.tasks.push(Task::new(task_id, priority, entry_point));
        Ok(task_id)
    }

    pub fn delete_task(&mut self, task_id: u16) -> Result<(), &'static str> {
        let len_before = self.tasks.len();
        self.tasks.retain(|task| task.id != task_id);

        if self.tasks.len() == len_before {
            return Err("Task ID not found");
        }

        // Release the ID back to the allocator
        self.allocator.dealloc(task_id as usize);
        Ok(())
    }

    /// Get a mutable reference to a task by ID.
    pub fn get_task_mut(&mut self, task_id: u16) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial_println;

    /// The example function
    extern "C" fn example_task() {
        serial_println!("Testing is this function work...");
    }

    #[test_case]
    fn test_create_task() {
        let mut task_manager = TaskManager::new();
        task_manager.create_task(1, example_task).unwrap();
        assert_eq!(task_manager.tasks.len(), 1);
    }

    #[test_case]
    fn test_delete_task() {
        // Create a task
        let mut task_manager = TaskManager::new();
        let tid = task_manager.create_task(1, example_task).unwrap();

        // And remove it
        task_manager.delete_task(tid).unwrap();
        assert_eq!(task_manager.tasks.len(), 0);
    }
}
