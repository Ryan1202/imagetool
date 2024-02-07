pub trait Empty {
    fn new_empty() -> Self;
    fn is_empty(&self) -> bool;
}

#[derive(Clone, Debug)]
pub struct Pool<T> {
    mem: Vec<T>,
    recycle: Vec<usize>,
}

pub trait CallBack {
    fn set_index(&mut self, idx: usize) -> Self;
}

impl<T: Empty + CallBack + Clone> Pool<T> {
    pub fn new() -> Self {
        let m: Vec<T> = Vec::new();
        let r: Vec<usize> = Vec::new();
        Pool { mem: m, recycle: r }
    }
    pub fn append(&mut self, data: &mut T) -> usize {
        let idx;
        if self.recycle.is_empty() {
            idx = self.mem.len();
            self.mem.push(data.clone());
            self.mem[idx] = self.mem[idx].set_index(idx);
        } else {
            idx = match self.recycle.pop() {
                Some(i) => i,
                None => {
                    idx = self.mem.len();
                    self.mem.push(data.clone());
                    self.mem[idx] = self.mem[idx].set_index(idx);
                    return idx;
                }
            };
            self.mem[idx] = data.clone();
        }
        idx
    }
    pub fn read(&self, index: usize) -> T {
        self.mem[index].clone()
    }
    pub fn update(&mut self, index: usize, data: &mut T) {
        self.mem[index] = data.clone();
    }
    pub fn delete(&mut self, index: usize) {
        self.recycle.push(index);
    }
}
