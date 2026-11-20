use crate::decode::{self, Frame};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub const THUMB: u32 = 480;

pub enum Msg {
    Done(usize, Result<Frame, String>),
    Finished,
}

struct Queue {
    jobs: VecDeque<(usize, PathBuf, u32)>,
    stop: bool,
}

pub struct Thumbs {
    pub rx: crossbeam_channel::Receiver<Msg>,
    tx: crossbeam_channel::Sender<Msg>,
    q: Arc<(Mutex<Queue>, Condvar)>,
    pub total: usize,
    pub done: usize,
    pub ok: usize,
    pub fail: usize,
    pub started: Instant,
    pub ms: Option<u128>,
    pub cancel: Arc<AtomicBool>,
    pub workers: usize,
    pub in_flight: Arc<AtomicUsize>,
}

impl Thumbs {
    pub fn empty() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self {
            rx,
            tx,
            q: Arc::new((
                Mutex::new(Queue {
                    jobs: VecDeque::new(),
                    stop: false,
                }),
                Condvar::new(),
            )),
            total: 0,
            done: 0,
            ok: 0,
            fail: 0,
            started: Instant::now(),
            ms: None,
            cancel: Arc::new(AtomicBool::new(false)),
            workers: 0,
            in_flight: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn start(paths: Vec<PathBuf>, workers: usize) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let q = Arc::new((
            Mutex::new(Queue {
                jobs: paths
                    .into_iter()
                    .enumerate()
                    .map(|(i, p)| (i, p, THUMB))
                    .collect(),
                stop: false,
            }),
            Condvar::new(),
        ));
        let cancel = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let total = q.0.lock().unwrap().jobs.len();
        for _ in 0..workers {
            let q = q.clone();
            let tx = tx.clone();
            let cancel = cancel.clone();
            let in_flight = in_flight.clone();
            std::thread::spawn(move || loop {
                let job = {
                    let (lock, cv) = &*q;
                    let mut g = lock.lock().unwrap();
                    loop {
                        if g.stop || cancel.load(Ordering::Relaxed) {
                            return;
                        }
                        if let Some(j) = g.jobs.pop_front() {
                            break Some(j);
                        }
                        if g.jobs.is_empty() {
                            return;
                        }
                        g = cv.wait_timeout(g, std::time::Duration::from_millis(200))
                            .unwrap()
                            .0;
                    }
                };
                let Some((idx, path, max)) = job else { return };
                in_flight.fetch_add(1, Ordering::Relaxed);
                let r = decode::load(&path, max);
                in_flight.fetch_sub(1, Ordering::Relaxed);
                let _ = tx.send(Msg::Done(idx, r));
            });
        }
        let _ = tx.send(Msg::Finished);
        Self {
            rx,
            tx,
            q,
            total,
            done: 0,
            ok: 0,
            fail: 0,
            started: Instant::now(),
            ms: None,
            cancel,
            workers,
            in_flight,
        }
    }

    /// Jump the queue, used for the photo on screen.
    pub fn bump(&self, idx: usize, path: PathBuf) {
        let (lock, cv) = &*self.q;
        let mut g = lock.lock().unwrap();
        if let Some(pos) = g.jobs.iter().position(|(i, _, _)| *i == idx) {
            let j = g.jobs.remove(pos).unwrap();
            g.jobs.push_front(j);
            cv.notify_all();
        } else {
            g.jobs.push_front((idx, path, THUMB));
            cv.notify_all();
        }
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let (lock, cv) = &*self.q;
        let mut g = lock.lock().unwrap();
        g.stop = true;
        g.jobs.clear();
        cv.notify_all();
    }

    pub fn running(&self) -> bool {
        self.done < self.total && !self.cancel.load(Ordering::Relaxed)
    }

    pub fn rate(&self) -> f64 {
        let s = self.started.elapsed().as_secs_f64();
        if s <= 0.0 {
            0.0
        } else {
            self.done as f64 / s
        }
    }

    pub fn seconds(&self) -> f64 {
        match self.ms {
            Some(ms) => ms as f64 / 1000.0,
            None => self.started.elapsed().as_secs_f64(),
        }
    }
}
