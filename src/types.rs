#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get = 0,
    Post = 1,
    Put = 2,
    Delete = 3,
    Patch = 4,
    Head = 5,
    Options = 6,
}

pub type ErrorCode = u16;
pub type StaticString = &'static str;
pub type WorkerId = u32;

pub type RouteMatch = (u16, Vec<(String, (usize, usize))>);
