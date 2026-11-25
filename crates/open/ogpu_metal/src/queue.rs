//! Ogpu_metal.Queue: one command queue, its submissions and their GPU durations.
use crate::native;
use crate::device::{fail, Device};
use ogpu_core::error::{Error, ErrorKind};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// What a submitted command buffer keeps alive until it completes; the last drop of a destroyed
/// object finishes its destroy.
#[allow(dead_code)]
pub enum Retained {
    Buffer(crate::buffer::Buffer),
    Texture(crate::texture::Texture),
    Sampler(crate::sampler::Sampler),
    Pipeline(crate::pipeline::Pipeline),
    Accel(crate::acceleration::Acceleration),
}

struct Pending {
    epoch: i64,
    command: native::CommandBuffer,
    retained: Vec<Retained>,
}

struct QueueInner {
    device: Device,
    metal: native::CommandQueue,
    next_epoch: Cell<i64>,
    pending: RefCell<Vec<Pending>>,
    /// Completed command-buffer wrappers recycled on the next begin (after GPU completion only).
    free_commands: RefCell<Vec<native::CommandBuffer>>,
    /// Durations of completed epochs the core has not yet asked for.
    durations: RefCell<Vec<(i64, f64)>>,
    dead: Cell<bool>,
}

#[derive(Clone)]
pub struct Queue(Rc<QueueInner>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub epoch: i64,
}

pub fn create(device: &Device) -> Result<Queue, Error> {
    let metal = native::CommandQueue::create(device.metal())?;
    device.attach_resource();
    Ok(Queue(Rc::new(QueueInner {
        device: device.clone(),
        metal,
        next_epoch: Cell::new(1),
        pending: RefCell::new(Vec::with_capacity(4)),
        free_commands: RefCell::new(Vec::with_capacity(4)),
        durations: RefCell::new(Vec::new()),
        dead: Cell::new(false),
    })))
}

pub(crate) fn adopt_raw(device: &Device, pointer: *mut std::ffi::c_void) -> Result<Queue, Error> {
    let metal = native::CommandQueue::adopt_raw(device.metal(), pointer)?;
    device.attach_resource();
    Ok(Queue(Rc::new(QueueInner {
        device: device.clone(),
        metal,
        next_epoch: Cell::new(1),
        pending: RefCell::new(Vec::with_capacity(4)),
        free_commands: RefCell::new(Vec::with_capacity(4)),
        durations: RefCell::new(Vec::new()),
        dead: Cell::new(false),
    })))
}

impl Queue {
    pub fn metal(&self) -> &native::CommandQueue {
        &self.0.metal
    }

    /// A command buffer wrapper: recycled after GPU completion when possible, otherwise created.
    pub fn obtain_command_buffer(&self) -> Result<native::CommandBuffer, Error> {
        let value = &self.0;
        if let Some(command) = value.free_commands.borrow_mut().pop() {
            match command.renew(&value.metal) {
                Ok(()) => return Ok(command),
                Err(_) => {
                    let _ = command.destroy();
                }
            }
        }
        native::CommandBuffer::create(&value.metal)
    }

    /// The GPU seconds of a completed epoch, handed over once.
    pub fn gpu_duration(&self, epoch: i64) -> Option<f64> {
        let mut durations = self.0.durations.borrow_mut();
        let at = durations.iter().position(|(e, _)| *e == epoch)?;
        Some(durations.swap_remove(at).1)
    }

    /// Admits an externally encoded, uncommitted native command buffer. On any failure the caller
    /// still owns `native` and `retained`; on success `retained` is emptied into the queue.
    pub fn submit_native(&self, native: &native::CommandBuffer, retained: &mut Vec<Retained>) -> Result<Receipt, Error> {
        let op = "Ogpu_metal.Queue.submit_native";
        let value = &self.0;
        if value.dead.get() {
            return Err(fail(op, ErrorKind::StaleHandle, "queue is destroyed"));
        }
        let epoch = value.next_epoch.get();
        match native.commit() {
            Err(e) => Err(e),
            Ok(()) => {
                value.next_epoch.set(epoch + 1);
                // `append` moves entries and keeps `retained`'s capacity for the next encode.
                let held = if retained.is_empty() {
                    Vec::new()
                } else {
                    let mut v = Vec::with_capacity(retained.len());
                    v.append(retained);
                    v
                };
                value.pending.borrow_mut().push(Pending { epoch, command: native.clone(), retained: held });
                Ok(Receipt { epoch })
            }
        }
    }

    pub fn wait_through(&self, epoch: i64) -> Result<(), Error> {
        let op = "Ogpu_metal.Queue.wait_through";
        let value = &self.0;
        let mut pending = value.pending.borrow_mut();
        if pending.first().map(|p| p.epoch > epoch).unwrap_or(true) {
            return Err(fail(op, ErrorKind::InvalidArgument, "epoch was not submitted"));
        }
        let mut first_error: Option<Error> = None;
        // `remove(0)` keeps the Vec capacity; pending is bounded (≤4).
        while pending.first().is_some_and(|p| p.epoch <= epoch) {
            let done = pending.remove(0);
            let outcome = match done.command.wait_until_completed() {
                Err(e) => Err(e),
                Ok(()) => {
                    if let Ok(d) = done.command.diagnostics() {
                        if d.gpu_end_time >= d.gpu_start_time {
                            value.durations.borrow_mut().push((done.epoch, d.gpu_end_time - d.gpu_start_time));
                        }
                    }
                    Ok(())
                }
            };
            drop(done.retained);
            if value.free_commands.borrow().len() < 4 {
                value.free_commands.borrow_mut().push(done.command);
            } else {
                let _ = done.command.destroy();
            }
            if let (None, Err(failure)) = (&first_error, outcome) {
                first_error = Some(failure);
            }
        }
        match first_error {
            Some(failure) => Err(failure),
            None => Ok(()),
        }
    }

    pub fn poll_through(&self, epoch: i64) -> Result<bool, Error> {
        let value = &self.0;
        for pending in value.pending.borrow().iter() {
            if pending.epoch > epoch {
                break;
            }
            match pending.command.status()? {
                native::CommandBufferStatus::Completed | native::CommandBufferStatus::Error => {}
                _ => return Ok(false),
            }
        }
        self.wait_through(epoch)?;
        Ok(true)
    }

    pub fn destroy(&self) -> Result<(), Error> {
        let op = "Ogpu_metal.Queue.destroy";
        let value = &self.0;
        if value.dead.get() {
            Ok(())
        } else if !value.pending.borrow().is_empty() {
            Err(fail(op, ErrorKind::InvalidState, "queue has commands in flight"))
        } else {
            for command in value.free_commands.borrow_mut().drain(..) {
                let _ = command.destroy();
            }
            value.metal.destroy()?;
            value.dead.set(true);
            value.device.detach_resource();
            Ok(())
        }
    }
}
