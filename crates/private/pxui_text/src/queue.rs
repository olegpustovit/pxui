//! The prepare queue: text a frame will need soon (a virtual list's rows past the viewport, an app's hint) is shaped ahead of
//! need on a `Worker`, never on the UI thread. A worker is one thread the host owns and attaches to any number of texts (one
//! per `Ui`; `Text::attach`); there is no hidden thread or global. Each attached text has its own lane: its requests, its
//! deliveries and its own fork of its fonts on the worker (texts of different `Ui`s do not share font identities, and
//! `install` refuses a layout shaped with other fonts). The UI thread copies requests into the lane's fixed buffers and flushes
//! them after the frame (`flush_queue`), installs at most `INSTALLS_PER_FRAME` deliveries before the next frame builds
//! (`install_queued`: no shaping, no allocation) and hands replaced storage back to the worker to free. Per lane at most
//! `QUEUE_ROWS` requests are in flight and one flush carries at most `QUEUE_BYTES` of text; past that a request waits for a
//! later frame. A font change on a text reforks that lane's fonts once; deliveries shaped with the old fonts are refused and
//! requested again. Text whose preparation fails or panics is delivered as a failure, remembered (`FAILED` keys) and shaped on
//! the UI thread; after a panic the worker drops that lane's fork (its caches may be inconsistent) and the text sends a new one
//! with its next flush. A worker serves at most `MAX_LANES` texts at once. A worker that is dropped, or whose thread ends, closes: its texts shape inline from then on. Dropping the
//! worker waits for at most one shaping call (it is checked between requests); detaching a text does not wait.
//! Only text that reaches the screen before its delivery shapes on the UI thread: the one named exception of the
//! warm-allocation contract (first-time shaping during a fast fling).
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::thread::JoinHandle;

/// Requests in flight per attached text: queued, being shaped, or delivered and not yet installed.
pub const QUEUE_ROWS: usize = 128;
/// Bytes of request text one flush of one attached text carries; also the longest text one request takes.
pub const QUEUE_BYTES: usize = 16 << 10;
/// Delivered layouts one attached text installs per frame.
pub const INSTALLS_PER_FRAME: usize = 32;
/// Keys whose preparation failed, remembered so they are not requested again.
const FAILED: usize = 32;
/// Texts one worker serves at once; attaching another fails until one detaches.
pub const MAX_LANES: usize = 64;

#[derive(Clone, Copy)]
struct Request {
    key: u64,
    style: Style,
    width: f32,
    start: usize,
    len: usize,
}

/// One attached text's share of the mailbox.
struct Lane {
    id: u64,
    requests: Vec<Request>,
    bytes: String,
    ready: Vec<(u64, Option<Prepared>)>,
    retired: Vec<Prepared>,
    /// a fork of the text's fonts for the worker to adopt (attaching, a font change)
    preparer: Option<Text>,
    detached: bool,
    /// a preparation panicked: the worker dropped the lane's fork; the text sends a new one with its next flush
    refork: bool,
}

impl Lane {
    fn busy(&self) -> bool {
        !self.requests.is_empty() || !self.retired.is_empty() || self.preparer.is_some() || self.detached
    }
}

struct Mailbox {
    lanes: Vec<Lane>,
    next: u64,
}

struct Shared {
    mailbox: Mutex<Mailbox>,
    changed: Condvar,
    /// the worker was dropped or its thread ended: attached texts shape inline
    closed: AtomicBool,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Mailbox> {
        self.mailbox.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The prepare queue's worker: one thread that shapes for every text attached to it. Memory: per attached text, a fork of
/// that text's font database (font bytes are shared, not copied) and fixed buffers of `QUEUE_ROWS` requests and
/// `QUEUE_BYTES` of text; the thread's own stack. Dropping it waits for at most one shaping call.
pub struct Worker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    /// Starts the worker's thread; `None` when the platform cannot start one.
    pub fn start() -> Option<Worker> {
        let shared = Arc::new(Shared { mailbox: Mutex::new(Mailbox { lanes: Vec::new(), next: 1 }), changed: Condvar::new(), closed: AtomicBool::new(false) });
        let owner = shared.clone();
        let thread = std::thread::Builder::new().name("pxui-text-prepare".into()).spawn(move || run(owner)).ok()?;
        Some(Worker { shared, thread: Some(thread) })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        {
            let _mailbox = self.shared.lock();
            self.shared.closed.store(true, Ordering::Relaxed);
        }
        self.shared.changed.notify_all();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The worker thread's: closes the worker however the thread ends (a panic outside a preparation included).
struct Closing(Arc<Shared>);

impl Drop for Closing {
    fn drop(&mut self) {
        self.0.closed.store(true, Ordering::Relaxed);
    }
}

/// A lane's fonts and buffers on the worker.
struct Shaper {
    id: u64,
    /// none after a panicking preparation, until the lane's text sends a new fork
    text: Option<Text>,
    requests: Vec<Request>,
    bytes: String,
    retired: Vec<Prepared>,
}

/// Tests: a request for this text panics inside its preparation (caught) ...
#[cfg(test)]
const PANIC_PREPARING: &str = "\u{1}panic preparing";
/// ... and for this one in the worker's loop (its thread ends).
#[cfg(test)]
const PANIC_WORKER: &str = "\u{1}panic in the worker";

fn run(shared: Arc<Shared>) {
    let _closing = Closing(shared.clone());
    let mut shapers: Vec<Shaper> = Vec::new();
    let mut garbage: Vec<Text> = Vec::new();
    let mut gone: Vec<Lane> = Vec::new();
    loop {
        {
            let mut mailbox = shared.lock();
            while !shared.closed.load(Ordering::Relaxed) && !mailbox.lanes.iter().any(Lane::busy) {
                mailbox = shared.changed.wait(mailbox).unwrap_or_else(PoisonError::into_inner);
            }
            if shared.closed.load(Ordering::Relaxed) {
                return;
            }
            let mut at = 0;
            while at < mailbox.lanes.len() {
                let id = mailbox.lanes[at].id;
                let local = shapers.iter().position(|shaper| shaper.id == id);
                if mailbox.lanes[at].detached {
                    gone.push(mailbox.lanes.swap_remove(at));
                    if let Some(local) = local {
                        garbage.extend(shapers.swap_remove(local).text);
                    }
                    continue;
                }
                let lane = &mut mailbox.lanes[at];
                let local = match (lane.preparer.take(), local) {
                    (Some(preparer), Some(local)) => {
                        garbage.extend(shapers[local].text.replace(preparer));
                        Some(local)
                    }
                    (Some(text), None) => {
                        let (requests, bytes, retired) = (Vec::with_capacity(QUEUE_ROWS), String::with_capacity(QUEUE_BYTES), Vec::with_capacity(2 * QUEUE_ROWS));
                        shapers.push(Shaper { id, text: Some(text), requests, bytes, retired });
                        Some(shapers.len() - 1)
                    }
                    (None, local) => local,
                };
                if let Some(local) = local {
                    let shaper = &mut shapers[local];
                    std::mem::swap(&mut shaper.requests, &mut lane.requests);
                    std::mem::swap(&mut shaper.bytes, &mut lane.bytes);
                    std::mem::swap(&mut shaper.retired, &mut lane.retired);
                }
                at += 1;
            }
        }
        garbage.clear();
        gone.clear();
        for shaper in &mut shapers {
            shaper.retired.clear();
            for at in 0..shaper.requests.len() {
                if shared.closed.load(Ordering::Relaxed) {
                    return;
                }
                let request = shaper.requests[at];
                let value = &shaper.bytes[request.start..request.start + request.len];
                #[cfg(test)]
                assert!(value != PANIC_WORKER, "injected worker panic");
                let Some(text) = shaper.text.as_mut() else {
                    // the fork was dropped after a panic: fail until the new one arrives
                    if let Some(lane) = shared.lock().lanes.iter_mut().find(|lane| lane.id == shaper.id && !lane.detached) {
                        lane.ready.push((request.key, None));
                    }
                    continue;
                };
                let prepared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #[cfg(test)]
                    assert!(value != PANIC_PREPARING, "injected preparation panic");
                    text.prepare(value, &request.style, request.width, &[])
                }));
                // a panic may have left the fork's caches inconsistent: drop it, the text reforks
                let panicked = prepared.is_err();
                if panicked {
                    garbage.extend(shaper.text.take());
                }
                let mut mailbox = shared.lock();
                if let Some(lane) = mailbox.lanes.iter_mut().find(|lane| lane.id == shaper.id && !lane.detached) {
                    lane.ready.push((request.key, prepared.unwrap_or(None)));
                    lane.refork |= panicked;
                }
            }
            shaper.requests.clear();
            shaper.bytes.clear();
        }
    }
}

/// An attached text's end of its lane.
pub(crate) struct Queue {
    shared: Arc<Shared>,
    id: u64,
    /// requested since the last flush
    requests: Vec<Request>,
    bytes: String,
    /// every key requested and not yet delivered
    flight: Vec<u64>,
    failed: [u64; FAILED],
    failed_at: usize,
    /// a frame's deliveries and the storage installing them replaced
    delivered: Vec<(u64, Option<Prepared>)>,
    retired: Vec<Prepared>,
    /// the font generation the lane's fork shapes with
    generation: Arc<()>,
    deferred: u64,
    installed: u64,
    /// the worker dropped the lane's fork after a panic: the next flush sends a new one
    refork: bool,
}

impl Drop for Queue {
    /// Detaching does not wait: the worker drops the lane (its fork and undelivered layouts) on its own time; a closed worker's
    /// lane is released here.
    fn drop(&mut self) {
        let mut mailbox = self.shared.lock();
        if let Some(at) = mailbox.lanes.iter().position(|lane| lane.id == self.id) {
            if self.shared.closed.load(Ordering::Relaxed) {
                let lane = mailbox.lanes.swap_remove(at);
                drop(mailbox);
                drop(lane);
                return;
            }
            mailbox.lanes[at].detached = true;
        }
        drop(mailbox);
        self.shared.changed.notify_all();
    }
}

impl Text {
    /// Attaches this text to `worker` (a lane with a fork of its fonts and fixed buffers), detaching it from any worker first;
    /// `None` only detaches. `false`: the worker serves `MAX_LANES` texts already (this text is left detached). A closed worker
    /// attaches nothing (the text shapes inline). Allocates: the fork and the lane's buffers, once per attach.
    pub fn attach(&mut self, worker: Option<&Worker>) -> bool {
        self.queue = None;
        let Some(worker) = worker else { return true };
        if worker.shared.closed.load(Ordering::Relaxed) {
            return true;
        }
        if worker.shared.lock().lanes.iter().filter(|lane| !lane.detached).count() >= MAX_LANES {
            return false;
        }
        let preparer = self.preparer();
        let generation = preparer.font_generation.clone();
        let shared = worker.shared.clone();
        let mut mailbox = shared.lock();
        let id = mailbox.next;
        mailbox.next += 1;
        mailbox.lanes.push(Lane {
            id,
            requests: Vec::with_capacity(QUEUE_ROWS),
            bytes: String::with_capacity(QUEUE_BYTES),
            ready: Vec::with_capacity(QUEUE_ROWS),
            retired: Vec::with_capacity(2 * QUEUE_ROWS),
            preparer: Some(preparer),
            detached: false,
            refork: false,
        });
        drop(mailbox);
        shared.changed.notify_all();
        self.queue = Some(Box::new(Queue {
            shared,
            id,
            requests: Vec::with_capacity(QUEUE_ROWS),
            bytes: String::with_capacity(QUEUE_BYTES),
            flight: Vec::with_capacity(QUEUE_ROWS),
            failed: [0; FAILED],
            failed_at: 0,
            delivered: Vec::with_capacity(INSTALLS_PER_FRAME),
            retired: Vec::with_capacity(2 * INSTALLS_PER_FRAME),
            generation,
            deferred: 0,
            installed: 0,
            refork: false,
        }));
        true
    }

    /// The text is attached to a worker that still runs.
    pub fn queue_running(&self) -> bool {
        self.queue.as_deref().is_some_and(|queue| !queue.shared.closed.load(Ordering::Relaxed))
    }

    /// Ahead of need: attached to a running worker, a layout this cache does not hold is requested from the worker instead of
    /// shaped now, and the caller uses empty metrics this frame (`true`). `false` (lay it out now): no running worker, inline
    /// spans, a cached key, text over `QUEUE_BYTES`, or a key whose preparation failed. A full lane defers without requesting;
    /// a later frame asks again. Allocates: never.
    pub fn defer(&mut self, key: u64, text: &str, style: &Style, width: f32, spans: &[TextSpan]) -> bool {
        if !self.queue_running() {
            return false;
        }
        let Some(queue) = self.queue.as_deref_mut() else { return false };
        if !spans.is_empty() || text.len() > QUEUE_BYTES || self.entries.contains_key(&key) || queue.failed.contains(&key) {
            return false;
        }
        queue.deferred += 1;
        if queue.flight.contains(&key) || queue.flight.len() == QUEUE_ROWS || queue.bytes.len() + text.len() > QUEUE_BYTES {
            return true;
        }
        let start = queue.bytes.len();
        queue.bytes.push_str(text);
        queue.requests.push(Request { key, style: *style, width, start, len: text.len() });
        queue.flight.push(key);
        true
    }

    /// Layouts `defer` kept off this thread, and layouts the worker shaped that were installed, since attaching.
    pub fn queue_counts(&self) -> (u64, u64) {
        self.queue.as_deref().map_or((0, 0), |queue| (queue.deferred, queue.installed))
    }

    /// Every request flushed to the worker has been delivered, or the worker is closed (tests poll it between frames).
    pub fn queue_delivered(&self) -> bool {
        self.queue.as_deref().is_none_or(|queue| {
            let mailbox = queue.shared.lock();
            queue.shared.closed.load(Ordering::Relaxed)
                || mailbox.lanes.iter().find(|lane| lane.id == queue.id)
                    .is_none_or(|lane| lane.requests.is_empty() && lane.ready.len() + queue.requests.len() == queue.flight.len())
        })
    }

    /// Before a frame builds: installs up to `INSTALLS_PER_FRAME` of the worker's deliveries (no shaping, no allocation), so
    /// the frame draws what it asked for one frame earlier. Replaced storage goes back to the worker with the next flush; a
    /// failed (or panicked) preparation is remembered and shapes inline. Allocates: never.
    pub fn install_queued(&mut self) {
        let Some(mut queue) = self.queue.take() else { return };
        {
            let mut mailbox = queue.shared.lock();
            if let Some(lane) = mailbox.lanes.iter_mut().find(|lane| lane.id == queue.id) {
                let count = lane.ready.len().min(INSTALLS_PER_FRAME);
                queue.delivered.extend(lane.ready.drain(..count));
                queue.refork |= std::mem::take(&mut lane.refork);
            }
        }
        let mut delivered = std::mem::take(&mut queue.delivered);
        for (key, prepared) in delivered.drain(..) {
            queue.flight.retain(|&item| item != key);
            let Some(mut prepared) = prepared else {
                queue.failed[queue.failed_at] = key;
                queue.failed_at = (queue.failed_at + 1) % FAILED;
                continue;
            };
            if self.install_record(&mut prepared, false) {
                queue.installed += 1;
            }
            // installed: the storage it replaced; refused (fonts changed): the record
            if queue.retired.len() < queue.retired.capacity() {
                queue.retired.push(prepared);
            }
        }
        queue.delivered = delivered;
        self.queue = Some(queue);
    }

    /// After a frame: flushes its requests and the replaced storage to the worker, and reforks the lane's fonts after a font
    /// change or a panicking preparation. Allocates: only that refork.
    pub(crate) fn flush_queue(&mut self) {
        let Some(mut queue) = self.queue.take() else { return };
        let preparer = (queue.refork || !Arc::ptr_eq(&queue.generation, &self.font_generation)).then(|| self.preparer());
        queue.refork = false;
        if let Some(preparer) = &preparer {
            queue.generation = preparer.font_generation.clone();
        }
        let mut mailbox = queue.shared.lock();
        let mut wake = false;
        if let Some(lane) = mailbox.lanes.iter_mut().find(|lane| lane.id == queue.id) {
            wake = preparer.is_some() || !queue.retired.is_empty() || (!queue.requests.is_empty() && lane.requests.is_empty());
            if lane.retired.len() + queue.retired.len() <= lane.retired.capacity() {
                lane.retired.append(&mut queue.retired);
            }
            if lane.requests.is_empty() {
                std::mem::swap(&mut queue.requests, &mut lane.requests);
                std::mem::swap(&mut queue.bytes, &mut lane.bytes);
            }
            if preparer.is_some() {
                lane.preparer = preparer;
            }
        }
        drop(mailbox);
        // storage the worker had no room for is freed here (freeing allocates nothing)
        queue.retired.clear();
        if wake {
            queue.shared.changed.notify_all();
        }
        self.queue = Some(queue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Between frames until every flushed request has been delivered (short-deadline polling, no sleep).
    fn delivered(text: &Text) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while !text.queue_delivered() {
            assert!(std::time::Instant::now() < deadline, "the worker delivered nothing for a second");
            std::thread::yield_now();
        }
    }

    fn round(text: &mut Text) {
        text.end_frame();
        delivered(text);
        text.install_queued();
    }

    /// The unbounded layout stands for every width at least its natural one (nothing can wrap or be cut), for left-aligned text;
    /// centred or right-aligned text places glyphs by the width, so only the natural width itself; a narrower width, another
    /// style or text, or a bounded entry never.
    #[test]
    fn natural_layout_serves_only_widths_that_cannot_change_it() {
        let mut text = Text::new(&[PRAGMASEVKA]);
        let style = Style { size: 14.0, wrap: Wrap::Word, ellipsis: Ellipsis::End, max_lines: 1, ..Style::default() };
        let key = content_key(&style, 0.0, "1234567 plays", &[]);
        assert!(text.natural_layout(key, "1234567 plays", &style, 500.0).is_none(), "uncached");
        let natural = text.layout(key, "1234567 plays", &style, 0.0);
        assert_eq!(text.natural_layout(key, "1234567 plays", &style, natural.w), Some(natural));
        assert_eq!(text.natural_layout(key, "1234567 plays", &style, 500.0), Some(natural));
        let layouts = text.layout_count();
        let narrow = natural.w - 1.0;
        assert!(text.natural_layout(key, "1234567 plays", &style, narrow).is_none());
        let cut = text.layout(content_key(&style, narrow, "1234567 plays", &[]), "1234567 plays", &style, narrow);
        assert!(cut.w <= narrow && cut.w < natural.w && text.layout_count() == layouts + 1, "a narrower width lays out its own cut");
        assert!(text.natural_layout(key, "other", &style, 500.0).is_none() && text.natural_layout(key, "1234567 plays", &style, 0.0).is_none());
        assert!(text.natural_layout(key, "1234567 plays", &Style { size: 15.0, ..style }, 500.0).is_none());
        let centred = Style { align: Align::Center, ..style };
        let ckey = content_key(&centred, 0.0, "1234567 plays", &[]);
        let cnat = text.layout(ckey, "1234567 plays", &centred, 0.0);
        assert!(text.natural_layout(ckey, "1234567 plays", &centred, 500.0).is_none());
        assert_eq!(text.natural_layout(ckey, "1234567 plays", &centred, cnat.w), Some(cnat));
        // right-to-left plain text is right-aligned by the engine: only its natural width
        let rtl = Style { align: Align::Right, ..style };
        let rkey = content_key(&rtl, 0.0, "1234567 plays", &[]);
        let rnat = text.layout(rkey, "1234567 plays", &rtl, 0.0);
        assert!(text.natural_layout(rkey, "1234567 plays", &rtl, 500.0).is_none());
        assert_eq!(text.natural_layout(rkey, "1234567 plays", &rtl, rnat.w), Some(rnat));
        // hard newlines: the widest line is the natural width, every line fits a width of at least that
        let lines = Style { max_lines: 0, ellipsis: Ellipsis::None, ..style };
        let lkey = content_key(&lines, 0.0, "1234567 plays\n12", &[]);
        let lnat = text.layout(lkey, "1234567 plays\n12", &lines, 0.0);
        assert_eq!(lnat.lines, 2);
        assert_eq!(text.natural_layout(lkey, "1234567 plays\n12", &lines, lnat.w + 1.0), Some(lnat));
        assert!(text.natural_layout(lkey, "1234567 plays\n12", &lines, lnat.w - 1.0).is_none());
        // a middle ellipsis at exactly the natural width cuts nothing
        let mid = Style { ellipsis: Ellipsis::Middle, ..style };
        let mkey = content_key(&mid, 0.0, "1234567 plays", &[]);
        let mnat = text.layout(mkey, "1234567 plays", &mid, 0.0);
        assert_eq!(text.natural_layout(mkey, "1234567 plays", &mid, mnat.w), Some(mnat));
        let exact = text.layout(content_key(&mid, mnat.w, "1234567 plays", &[]), "1234567 plays", &mid, mnat.w);
        assert_eq!((exact.w, exact.lines), (mnat.w, mnat.lines), "the bounded layout at the natural width is not cut");
        let bounded = content_key(&style, 500.0, "1234567 plays", &[]);
        text.layout(bounded, "1234567 plays", &style, 500.0);
        assert!(text.natural_layout(bounded, "1234567 plays", &style, 600.0).is_none(), "only an unbounded entry is natural");
    }

    /// A deferred layout is shaped on the worker, flushed after the frame, installed before the next one (no allocation, no
    /// shaping on this thread) and then laid out from the cache; a font added since reforks the lane and its deliveries
    /// install; text that cannot be prepared is not requested again; a detached text shapes inline.
    #[test]
    fn deferred_text_arrives_from_the_worker_and_survives_a_font_change() {
        let worker = Worker::start().unwrap();
        let mut text = Text::new(&[PRAGMASEVKA]);
        let style = Style { size: 14.0, ..Style::default() };
        assert!(!text.defer(1, "x", &style, 0.0, &[]), "no worker: lay it out now");
        text.attach(Some(&worker));
        let key = content_key(&style, 0.0, "Song title", &[]);
        assert!(text.defer(key, "Song title", &style, 0.0, &[]) && text.defer(key, "Song title", &style, 0.0, &[]));
        text.end_frame();
        delivered(&text);
        mem_sys::local_reset();
        let (allocations, layouts) = (mem_sys::local_alloc_count(), text.layout_count());
        text.install_queued();
        assert_eq!(mem_sys::local_alloc_count() - allocations, 0, "installing allocated");
        assert_eq!(text.queue_counts(), (2, 1));
        assert!(!text.defer(key, "Song title", &style, 0.0, &[]), "cached: lay it out (a hit)");
        assert!(text.layout(key, "Song title", &style, 0.0).w > 0.0);
        assert_eq!(text.layout_count(), layouts, "the delivered layout was shaped again");

        text.add_font(PRAGMASEVKA);
        let next = content_key(&style, 0.0, "Next title", &[]);
        assert!(text.defer(next, "Next title", &style, 0.0, &[]));
        round(&mut text);
        assert_eq!(text.queue_counts().1, 2, "the reforked lane's delivery was refused");
        assert!(text.has_text(next, "Next title"));

        let bad_style = Style { size: f32::NAN, ..style };
        let bad = content_key(&bad_style, 0.0, "bad", &[]);
        assert!(text.defer(bad, "bad", &bad_style, 0.0, &[]));
        round(&mut text);
        assert!(!text.defer(bad, "bad", &bad_style, 0.0, &[]), "a failed preparation is requested again");

        assert!(text.attach(None));
        assert!(!text.queue_running() && !text.defer(content_key(&style, 0.0, "later", &[]), "later", &style, 0.0, &[]));
    }

    /// One worker serves several texts, each delivered its own layouts (lanes keep their requests apart); dropping one text
    /// leaves the others served.
    #[test]
    fn one_worker_serves_several_texts_each_its_own() {
        let worker = Worker::start().unwrap();
        let style = Style { size: 14.0, ..Style::default() };
        let mut texts: Vec<Text> = (0..3).map(|_| Text::new(&[PRAGMASEVKA])).collect();
        for (n, text) in texts.iter_mut().enumerate() {
            assert!(text.attach(Some(&worker)));
            let value = format!("Window {n}");
            assert!(text.defer(content_key(&style, 0.0, &value, &[]), &value, &style, 0.0, &[]));
        }
        drop(texts.remove(1));
        for (n, text) in [0, 2].into_iter().zip(&mut texts) {
            round(text);
            assert_eq!(text.queue_counts(), (1, 1));
            let value = format!("Window {n}");
            assert!(text.has_text(content_key(&style, 0.0, &value, &[]), &value), "{value} was not delivered to its own text");
        }
    }

    /// A panic preparing one text is caught: it is delivered as a failure (not requested again) and the worker keeps going;
    /// a panic that ends the worker's thread closes it, and its texts shape inline from then on.
    #[test]
    fn a_panicking_preparation_fails_and_a_dead_worker_closes() {
        let worker = Worker::start().unwrap();
        let mut text = Text::new(&[PRAGMASEVKA]);
        let style = Style { size: 14.0, ..Style::default() };
        text.attach(Some(&worker));
        let first = content_key(&style, 0.0, PANIC_PREPARING, &[]);
        assert!(text.defer(first, PANIC_PREPARING, &style, 0.0, &[]));
        round(&mut text);
        assert!(!text.defer(first, PANIC_PREPARING, &style, 0.0, &[]), "a panicked preparation is requested again");
        assert!(text.queue.as_deref().is_some_and(|queue| queue.refork), "the panicked lane's fork was not renewed");
        let second = content_key(&style, 0.0, "second", &[]);
        assert!(text.defer(second, "second", &style, 0.0, &[]));
        round(&mut text);
        assert!(text.has_text(second, "second"), "the lane did not shape again on its new fork after a caught panic");

        let third = content_key(&style, 0.0, PANIC_WORKER, &[]);
        assert!(text.defer(third, PANIC_WORKER, &style, 0.0, &[]));
        text.end_frame();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        while text.queue_running() {
            assert!(std::time::Instant::now() < deadline, "a dead worker did not close");
            std::thread::yield_now();
        }
        assert!(!text.defer(content_key(&style, 0.0, "fourth", &[]), "fourth", &style, 0.0, &[]), "a dead worker's text must shape inline");
        drop(worker);
    }

    /// A worker serves at most `MAX_LANES` texts: one more is refused (left detached, shaping inline) until one detaches; the
    /// texts of a dropped worker shape inline and detach without it.
    #[test]
    fn a_worker_serves_at_most_its_lanes() {
        let worker = Worker::start().unwrap();
        let mut texts: Vec<Text> = (0..MAX_LANES).map(|_| Text::new(&[PRAGMASEVKA])).collect();
        for text in &mut texts {
            assert!(text.attach(Some(&worker)));
        }
        let mut extra = Text::new(&[PRAGMASEVKA]);
        assert!(!extra.attach(Some(&worker)) && !extra.queue_running(), "a lane past the cap was attached");
        assert!(texts[0].attach(None));
        assert!(extra.attach(Some(&worker)) && extra.queue_running(), "a freed lane was not reusable");
        drop(worker);
        assert!(!extra.queue_running());
        drop(texts);
        drop(extra);
    }
}
