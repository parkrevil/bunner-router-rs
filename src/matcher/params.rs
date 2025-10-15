use std::cell::RefCell;

pub type ParamEntry = (String, (usize, usize));

thread_local! {
    static PARAM_BUF: RefCell<Vec<ParamEntry>> = RefCell::new(Vec::with_capacity(4));
}

pub fn with_param_buffer<R>(f: impl FnOnce(&mut Vec<ParamEntry>) -> R) -> R {
    PARAM_BUF.with(|cell| {
        let mut buf = cell.borrow_mut();
        buf.clear();
        f(&mut buf)
    })
}
