use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::signals::StructuralSignal;

#[derive(Clone, Debug)]
enum EventBusMessage {
    Signal(StructuralSignal),
    Shutdown,
}

type Handler = Box<dyn Fn(StructuralSignal) + Send + 'static>;
type Handlers = Arc<Mutex<HashMap<usize, Handler>>>;

/// Event bus with single dispatcher thread pattern.
/// Instead of spawning a thread per subscriber, all handlers are
/// invoked from a single dispatcher thread.
#[derive(Clone)]
pub(crate) struct EventBus {
    inner: Arc<EventBusInner>,
}

struct EventBusInner {
    next_id: AtomicUsize,
    handlers: Handlers,
    signal_tx: Sender<EventBusMessage>,
    shutdown: Arc<AtomicBool>,
    shutdown_complete: Arc<(Mutex<bool>, Condvar)>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub(crate) fn new() -> Self {
        let (signal_tx, signal_rx) = unbounded();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_complete = Arc::new((Mutex::new(false), Condvar::new()));
        let handlers: Handlers = Arc::new(Mutex::new(HashMap::new()));

        // Spawn the single dispatcher thread
        let handlers_clone = Arc::clone(&handlers);
        let shutdown_clone = Arc::clone(&shutdown);
        let shutdown_complete_clone = Arc::clone(&shutdown_complete);

        thread::spawn(move || {
            dispatcher_loop(
                signal_rx,
                handlers_clone,
                shutdown_clone,
                shutdown_complete_clone,
            );
        });

        Self {
            inner: Arc::new(EventBusInner {
                next_id: AtomicUsize::new(0),
                handlers,
                signal_tx,
                shutdown,
                shutdown_complete,
            }),
        }
    }

    pub(crate) fn emit(&self, signal: StructuralSignal) {
        // Non-blocking send - if shutdown, this will fail silently
        let _ = self.inner.signal_tx.send(EventBusMessage::Signal(signal));
    }

    /// Register a handler that will be called on the dispatcher thread.
    pub(crate) fn register<F>(&self, handler: F) -> usize
    where
        F: Fn(StructuralSignal) + Send + 'static,
    {
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        self.inner
            .handlers
            .lock()
            .expect("handlers mutex poisoned")
            .insert(id, Box::new(handler));
        id
    }

    pub(crate) fn unregister(&self, id: usize) -> bool {
        self.inner
            .handlers
            .lock()
            .expect("handlers mutex poisoned")
            .remove(&id)
            .is_some()
    }

    /// Signal the dispatcher to shut down and wait for completion.
    pub(crate) fn shutdown(&self) {
        self.inner.shutdown.store(true, Ordering::SeqCst);

        // Send an explicit shutdown command to wake up the dispatcher if it's waiting
        let _ = self.inner.signal_tx.send(EventBusMessage::Shutdown);

        // Wait for dispatcher to complete
        let (lock, cvar) = &*self.inner.shutdown_complete;
        let mut completed = lock.lock().unwrap();
        while !*completed {
            completed = cvar.wait(completed).unwrap();
        }
    }

    /// Check if shutdown has been requested.
    pub(crate) fn is_shutdown(&self) -> bool {
        self.inner.shutdown.load(Ordering::SeqCst)
    }

    // Legacy API for backward compatibility
    pub(crate) fn subscribe(&self) -> (usize, Receiver<StructuralSignal>) {
        let (tx, rx) = unbounded();
        let id = self.register(move |signal| {
            let _ = tx.send(signal);
        });
        (id, rx)
    }

    pub(crate) fn unsubscribe(&self, id: usize) -> bool {
        self.unregister(id)
    }
}

fn dispatcher_loop(
    rx: Receiver<EventBusMessage>,
    handlers: Handlers,
    shutdown: Arc<AtomicBool>,
    shutdown_complete: Arc<(Mutex<bool>, Condvar)>,
) {
    while let Ok(message) = rx.recv() {
        let signal = match message {
            EventBusMessage::Signal(signal) => signal,
            EventBusMessage::Shutdown => break,
        };

        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Hold the lock and call handlers directly.
        // This is safe because handlers should be fast (they typically just
        // enqueue work or update state).
        {
            let handlers = handlers.lock().expect("handlers mutex poisoned");
            for handler in handlers.values() {
                handler(signal.clone());
            }
        }
    }

    // Signal completion
    let (lock, cvar) = &*shutdown_complete;
    let mut completed = lock.lock().unwrap();
    *completed = true;
    cvar.notify_all();
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::time::Duration;

    use super::*;
    use crate::signals::{ClipboardSignal, StructuralSignal};

    #[test]
    fn broadcasts_to_subscribers() {
        let bus = EventBus::new();
        let (_, rx) = bus.subscribe();

        bus.emit(StructuralSignal::Clipboard(ClipboardSignal::text(
            "hello",
            "test-app".to_string(),
        )));

        // Give dispatcher time to process
        thread::sleep(Duration::from_millis(50));

        let signal = rx.try_recv().expect("expected signal");
        match signal {
            StructuralSignal::Clipboard(signal) => assert_eq!(signal.size_bytes, 5),
            _ => panic!("expected clipboard signal"),
        }
    }

    #[test]
    fn unsubscribes_cleanly() {
        let bus = EventBus::new();
        let (id, rx) = bus.subscribe();
        assert!(bus.unsubscribe(id));

        bus.emit(StructuralSignal::Clipboard(ClipboardSignal::text(
            "test",
            "test-app".to_string(),
        )));

        thread::sleep(Duration::from_millis(50));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn register_handler_receives_signals() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        bus.register(move |_signal| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit(StructuralSignal::Clipboard(ClipboardSignal::text(
            "test",
            "test-app".to_string(),
        )));

        thread::sleep(Duration::from_millis(50));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn shutdown_completes_cleanly() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        bus.register(move |_signal| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit(StructuralSignal::Clipboard(ClipboardSignal::text(
            "test",
            "test-app".to_string(),
        )));

        thread::sleep(Duration::from_millis(50));
        bus.shutdown();

        assert!(bus.is_shutdown());
    }
}
