// Accepted clients precede bounded bulk work; a started radio action keeps the shared IPC deadline.
pub const RECEIVE_FRAMES: usize = 8;
pub const RECEIVE_MS: u64 = 100;

pub fn run<S, R>(state: &mut S, request: R, dispatch: impl FnOnce(&mut S, R), ready: impl FnOnce(&S) -> bool, background: impl FnOnce(&mut S)) {
    dispatch(state, request);
    if ready(state) { background(state); }
}

pub fn receive_batch<S>(state: &mut S, mut now_ms: impl FnMut() -> u64, mut receive: impl FnMut(&mut S) -> bool) {
    let start = now_ms();
    for _ in 0..RECEIVE_FRAMES {
        let now = now_ms();
        if now < start || now - start >= RECEIVE_MS { break; }
        if !receive(state) { break; }
    }
}
