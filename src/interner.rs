use hashbrown::HashMap as FastHashMap;
use parking_lot::RwLock;

#[derive(Debug, Default)]
pub struct Interner {
    map: RwLock<FastHashMap<Box<str>, u32>>,
    rev: RwLock<Vec<Box<str>>>,
}

impl Interner {
    pub fn new() -> Self {
        Self {
            map: RwLock::new(FastHashMap::new()),
            rev: RwLock::new(Vec::new()),
        }
    }

    #[inline]
    pub fn intern(&self, s: &str) -> u32 {
        if let Some(id) = self.map.read().get(s).copied() {
            return id;
        }

        // Upgrade to write: allocate once and record both tables
        let mut map = self.map.write();
        if let Some(&id) = map.get(s) {
            return id;
        }
        let mut rev = self.rev.write();
        let id = rev.len() as u32;
        let boxed = s.to_string().into_boxed_str();
        rev.push(boxed.clone());
        map.insert(boxed, id);
        id
    }

    #[inline]
    pub fn runtime_cleanup(&self) {
        let mut rev = self.rev.write();

        rev.clear();
        rev.shrink_to_fit();
    }
}
