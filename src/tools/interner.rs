use hashbrown::HashMap as FastHashMap;
use parking_lot::RwLock;

#[derive(Debug, Default)]
struct InternerInner {
    map: FastHashMap<Box<str>, u32>,
    rev: Vec<Box<str>>,
}

#[derive(Debug, Default)]
pub struct Interner {
    inner: RwLock<InternerInner>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn intern(&self, s: &str) -> u32 {
        if let Some(id) = self
            .inner
            .read()
            .map
            .get(s)
            .copied()
        {
            return id;
        }

        let mut inner = self.inner.write();
        if let Some(&id) = inner.map.get(s) {
            return id;
        }

        let id = inner.rev.len() as u32;
        let boxed = s.to_string().into_boxed_str();
        inner.rev.push(boxed.clone());
        inner.map.insert(boxed, id);
        id
    }

    #[inline]
    pub fn runtime_cleanup(&self) {
        let mut inner = self.inner.write();

        inner.map.clear();
        inner.map.shrink_to_fit();
        inner.rev.clear();
        inner.rev.shrink_to_fit();
    }
}
