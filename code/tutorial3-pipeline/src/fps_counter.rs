use web_time::Instant;

const ARRAY_SIZE: usize = 180;

pub struct FpsCounter {
    values: [Instant; ARRAY_SIZE],
    pos: usize,
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            values: [Instant::now(); ARRAY_SIZE],
            pos: 0,
        }
    }

    pub fn next_pos(&self) -> usize {
        (self.pos + 1) % ARRAY_SIZE
    }

    pub fn register_entry(&mut self, time_point: Instant) {
        self.pos = self.next_pos();
        self.values[self.pos] = time_point;
    }

    pub fn framerate(&self) -> u32 {
        let past = &self.values[self.next_pos()];
        let curr = &self.values[self.pos];
        let duration = curr.duration_since(*past);

        (ARRAY_SIZE as f64 / duration.as_secs_f64()) as u32
    }
}
