use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex};

use tracing::{Subscriber, span};
use tycho_vm::VmLogMask;

const VM_TARGET: &str = "tycho_vm";

pub struct VmLogSubscriber {
    vm_log_mask: VmLogMask,
    state: VmLogRows,
}

impl VmLogSubscriber {
    pub fn new(mask: VmLogMask, capacity: usize) -> Self {
        Self {
            vm_log_mask: mask,
            state: VmLogRows {
                inner: Arc::new(Mutex::new(Inner {
                    capacity,
                    rows: VecDeque::with_capacity(capacity.min(256)),
                })),
            },
        }
    }

    pub fn state(&self) -> &VmLogRows {
        &self.state
    }
}

impl Subscriber for VmLogSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        metadata.target() == VM_TARGET
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_non_zero_u64(NonZeroU64::MIN)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        if !self.enabled(event.metadata()) {
            return;
        }

        event.record(&mut LogVisitor {
            inner: &mut self.state.inner.lock().unwrap(),
            mask: self.vm_log_mask,
        });
    }

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

struct LogVisitor<'a> {
    inner: &'a mut Inner,
    mask: VmLogMask,
}

impl tracing::field::Visit for LogVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        use std::fmt::Write;

        const STACK_MASK: VmLogMask = VmLogMask::DUMP_STACK.union(VmLogMask::DUMP_STACK_VERBOSE);

        let mut buffer = self.inner.get_buffer();

        let res = match field.name() {
            "message" if self.mask.contains(VmLogMask::MESSAGE) => {
                write!(&mut buffer, "{value:?}")
            }
            "opcode" if self.mask.contains(VmLogMask::MESSAGE) => {
                write!(&mut buffer, "execute {value:?}")
            }
            "stack" if self.mask.intersects(STACK_MASK) => {
                write!(&mut buffer, "stack: {value:?}")
            }
            "exec_location" if self.mask.contains(VmLogMask::EXEC_LOCATION) => {
                write!(&mut buffer, "code cell hash: {value:?}")
            }
            "gas_remaining" if self.mask.contains(VmLogMask::GAS_REMAINING) => {
                write!(&mut buffer, "gas remaining: {value:?}")
            }
            "c5" if self.mask.contains(VmLogMask::DUMP_C5) => {
                write!(&mut buffer, "c5: {value:?}")
            }
            _ => return,
        };

        if res.is_ok() {
            self.inner.rows.push_back(buffer);
        }
    }
}

#[derive(Clone)]
pub struct VmLogRows {
    inner: Arc<Mutex<Inner>>,
}

impl serde::Serialize for VmLogRows {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl std::fmt::Display for VmLogRows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut inner = self.inner.lock().unwrap();
        for row in std::mem::take(&mut inner.rows) {
            writeln!(f, "{row}")?;
        }
        Ok(())
    }
}

struct Inner {
    capacity: usize,
    rows: VecDeque<String>,
}

impl Inner {
    fn get_buffer(&mut self) -> String {
        const OK_LEN: usize = 128;

        if self.rows.len() >= self.capacity
            && let Some(mut s) = self.rows.pop_front()
            && s.len() <= OK_LEN
        {
            s.clear();
            return s;
        }

        String::new()
    }
}
