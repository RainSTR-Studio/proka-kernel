extern crate alloc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    pub static ref TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());
}

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
#[allow(unused)]
pub struct Task {
    /// The ID of this task.
    ///
    /// This uses type "u16", which means the task limit is
    /// 65535. (id range is 0~65535)
    id: u16,

    /// The state of the task.
    state: TaskState,

    /// The priority of the kernel (1-8)
    priority: u8,

    /// The entry point of the task.
    entry_point: usize,
}

impl Task {
    /// Create a new task object.
    pub fn new(id: u16, priority: u8, entry_point: extern "C" fn()) -> Self {
        Self {
            id,
            state: TaskState::Ready,
            priority,
            entry_point: entry_point as usize,
        }
    }

    /// Change the status of a task.
    pub fn update_stat(&mut self, new_state: TaskState) {
        self.state = new_state
    }
}

/// The task manager which contains lots of tasks.
pub struct TaskManager {
    /// The field which contains all tasks.
    tasks: Vec<Task>,

    /// The task ID which has been allocated.
    allocated_tid: Vec<u16>,

    /// The next task id
    next_tid: u16,
}

impl TaskManager {
    pub const fn new() -> Self {
        Self {
            tasks: Vec::new(),
            allocated_tid: Vec::new(),
            next_tid: 0,
        }
    }

    pub fn create_task(&mut self, priority: u8, entry_point: extern "C" fn()) {
        // Allocate a task id
        let mut task_id = self.next_tid;

        // Check: is current ID has been allocated
        if self.allocated_tid.contains(&task_id) {
            task_id += 1;
        }

        // Push the task to the tasks container
        self.tasks.push(Task::new(task_id, priority, entry_point));

        // Set the current id is allocated.
        self.allocated_tid.push(task_id);

        // Set up new ID
        self.next_tid = self.next_tid.wrapping_add(1);
    }

    pub fn delete_task(&mut self, task_id: u16) -> Result<(), &'static str> {
        // Check is task ID allocated.
        if !self.allocated_tid.contains(&task_id) {
            return Err("The task ID is unable to discovor.");
        }

        // Remove the task from [`tasks`].
        self.tasks.retain(|task| task.id != task_id);

        // Remove the task from [`allocated_tid`].
        self.allocated_tid.retain(|id| *id != task_id);

        // Find the task to remove from [`tasks`].
        Ok(())
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
        task_manager.create_task(1, example_task);
        assert_eq!(task_manager.tasks.len(), 1);
    }

    #[test_case]
    fn test_delete_task() {
        // Create a task
        let mut task_manager = TaskManager::new();
        task_manager.create_task(1, example_task);


        // And remove it
        task_manager.delete_task(0).unwrap();   // TaskManager allocates ID 0
        assert_eq!(task_manager.tasks.len(), 0);
    }
}
